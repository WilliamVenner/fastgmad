// https://github.com/garrynewman/bootil/blob/beb4cec8ad29533965491b767b177dc549e62d23/src/3rdParty/globber.cpp
// https://github.com/Facepunch/gmad/blob/master/include/AddonWhiteList.h

const ADDON_WHITELIST: &[&str] = &[
	"lua/*.lua",
	"scenes/*.vcd",
	"particles/*.pcf",
	"resource/fonts/*.ttf",
	"scripts/vehicles/*.txt",
	"resource/localization/*/*.properties",
	"maps/*.bsp",
	"maps/*.lmp",
	"maps/*.nav",
	"maps/*.ain",
	"maps/thumb/*.png",
	"sound/*.wav",
	"sound/*.mp3",
	"sound/*.ogg",
	"materials/*.vmt",
	"materials/*.vtf",
	"materials/*.png",
	"materials/*.jpg",
	"materials/*.jpeg",
	"materials/colorcorrection/*.raw",
	"models/*.mdl",
	"models/*.vtx",
	"models/*.phy",
	"models/*.ani",
	"models/*.vvd",
	"gamemodes/*/*.txt",
	"gamemodes/*/*.fgd",
	"gamemodes/*/logo.png",
	"gamemodes/*/icon24.png",
	"gamemodes/*/gamemode/*.lua",
	"gamemodes/*/entities/effects/*.lua",
	"gamemodes/*/entities/weapons/*.lua",
	"gamemodes/*/entities/entities/*.lua",
	"gamemodes/*/backgrounds/*.png",
	"gamemodes/*/backgrounds/*.jpg",
	"gamemodes/*/backgrounds/*.jpeg",
	"gamemodes/*/content/models/*.mdl",
	"gamemodes/*/content/models/*.vtx",
	"gamemodes/*/content/models/*.phy",
	"gamemodes/*/content/models/*.ani",
	"gamemodes/*/content/models/*.vvd",
	"gamemodes/*/content/materials/*.vmt",
	"gamemodes/*/content/materials/*.vtf",
	"gamemodes/*/content/materials/*.png",
	"gamemodes/*/content/materials/*.jpg",
	"gamemodes/*/content/materials/*.jpeg",
	"gamemodes/*/content/materials/colorcorrection/*.raw",
	"gamemodes/*/content/scenes/*.vcd",
	"gamemodes/*/content/particles/*.pcf",
	"gamemodes/*/content/resource/fonts/*.ttf",
	"gamemodes/*/content/scripts/vehicles/*.txt",
	"gamemodes/*/content/resource/localization/*/*.properties",
	"gamemodes/*/content/maps/*.bsp",
	"gamemodes/*/content/maps/*.nav",
	"gamemodes/*/content/maps/*.ain",
	"gamemodes/*/content/maps/thumb/*.png",
	"gamemodes/*/content/sound/*.wav",
	"gamemodes/*/content/sound/*.mp3",
	"gamemodes/*/content/sound/*.ogg",
	"data_static/*.txt",
	"data_static/*.dat",
	"data_static/*.json",
	"data_static/*.xml",
	"data_static/*.csv",
	"data_static/*.dem",
	"data_static/*.vcd",
	"data_static/*.vtf",
	"data_static/*.vmt",
	"data_static/*.png",
	"data_static/*.jpg",
	"data_static/*.jpeg",
	"data_static/*.mp3",
	"data_static/*.wav",
	"data_static/*.ogg",
];

const WILD_BYTE: u8 = b'*';
const QUESTION_BYTE: u8 = b'?';

fn globber(wild: &str, str: &str) -> bool {
	unsafe {
		let mut cp: *const u8 = core::ptr::null();
		let mut mp: *const u8 = core::ptr::null();

		let (mut wild, wild_max) = (wild.as_ptr(), wild.as_ptr().add(wild.len()));
		let (mut str, str_max) = (str.as_ptr(), str.as_ptr().add(str.len()));

		while wild < wild_max && str < str_max && *wild != WILD_BYTE {
			if *wild != *str && *wild != QUESTION_BYTE {
				return false;
			}
			wild = wild.add(1);
			str = str.add(1);
		}

		while str < str_max {
			if *wild == WILD_BYTE {
				wild = wild.add(1);
				if wild >= wild_max {
					return true;
				}
				mp = wild;
				cp = str.add(1);
			} else if *wild == *str || *wild == QUESTION_BYTE {
				wild = wild.add(1);
				str = str.add(1);
			} else {
				wild = mp;
				str = cp;
				cp = cp.add(1);
			}
		}

		while wild < wild_max && *wild == WILD_BYTE {
			wild = wild.add(1);
		}

		wild >= wild_max
	}
}

/// Check if a path is allowed in a GMA file
pub fn check(str: &str) -> bool {
	for glob in ADDON_WHITELIST {
		if globber(glob, str) {
			return true;
		}
	}

	false
}

/// Check if a path is ignored by a list of custom globs
pub fn is_ignored(str: &str, ignore: &[String]) -> bool {
	if ignore.is_empty() {
		return false;
	}

	for glob in ignore {
		if globber(glob, str) {
			return true;
		}
	}

	false
}

#[test]
pub fn test_whitelist() {
	let good: &[&str] = &[
		"lua/test.lua",
		"lua/lol/test.lua",
		"lua/lua/testing.lua",
		"gamemodes/test/something.txt",
		"gamemodes/test/content/sound/lol.wav",
		"materials/lol.jpeg",
		"gamemodes/the_gamemode_name/backgrounds/file_name.jpg",
		"gamemodes/my_base_defence/backgrounds/1.jpg",
	];

	let bad: &[&str] = &[
		"test.lua",
		"lua/test.exe",
		"lua/lol/test.exe",
		"gamemodes/test",
		"gamemodes/test/something",
		"gamemodes/test/something/something.exe",
		"gamemodes/test/content/sound/lol.vvv",
		"materials/lol.vvv",
	];

	for good in good {
		assert!(check(&*good), "{}", good);
	}

	for good in ADDON_WHITELIST {
		assert!(check(&good.replace('*', "test")));
	}

	for good in ADDON_WHITELIST {
		assert!(check(&good.replace('*', "a")));
	}

	for bad in bad {
		assert!(!check(&*bad));
	}
}

#[test]
pub fn test_ignore() {
	assert!(is_ignored(&"lol.txt".to_string(), &["lol.txt".to_string()]));
	assert!(is_ignored(&"lua/hello.lua".to_string(), &["lua/*.lua".to_string()]));
	assert!(is_ignored(&"lua/hello.lua".to_string(), &["lua/*".to_string()]));
	assert!(is_ignored(&".gitattributes".to_string(), &[".git*".to_string()]));
	assert!(!is_ignored(&"lol.txt".to_string(), &[]));
}
