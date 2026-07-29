// https://github.com/garrynewman/bootil/blob/beb4cec8ad29533965491b767b177dc549e62d23/src/3rdParty/globber.cpp
// https://github.com/Facepunch/gmad/blob/master/include/AddonWhiteList.h

use std::{sync::LazyLock, time::Duration};

const ADDON_WHITELIST_OFFLINE: &[&str] = &[
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
	"models/*.phy",
	"models/*.ani",
	"models/*.vvd",
	"models/*.vtx",
	"!models/*.sw.vtx",
	"!models/*.360.vtx",
	"!models/*.xbox.vtx",
	"gamemodes/*/*.txt",
	"!gamemodes/*/*/*.txt",
	"gamemodes/*/*.fgd",
	"!gamemodes/*/*/*.fgd",
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
	"gamemodes/*/content/models/*.phy",
	"gamemodes/*/content/models/*.ani",
	"gamemodes/*/content/models/*.vvd",
	"gamemodes/*/content/models/*.vtx",
	"!gamemodes/*/content/models/*.sw.vtx",
	"!gamemodes/*/content/models/*.360.vtx",
	"!gamemodes/*/content/models/*.xbox.vtx",
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
	"shaders/*.vcs",
];

const ALWAYS_IGNORED: &[&str] = &[
	"models/*.sw.vtx",
	"models/*.360.vtx",
	"models/*.xbox.vtx",
	"gamemodes/*/content/models/*.sw.vtx",
	"gamemodes/*/content/models/*.360.vtx",
	"gamemodes/*/content/models/*.xbox.vtx",
];

static ADDON_WHITELIST: LazyLock<&'static [&'static str]> = LazyLock::new(download_addon_whitelist);

fn download_addon_whitelist() -> &'static [&'static str] {
	if std::env::var_os("ADDON_WHITELIST_OFFLINE").is_some() {
		return ADDON_WHITELIST_OFFLINE;
	}

	sysreq::RequestBuilder::new("https://raw.githubusercontent.com/Facepunch/gmad/master/include/AddonWhiteList.h")
		.timeout(Some(Duration::from_secs(2)))
		.send()
		.map_err(std::io::Error::other)
		.and_then(|response| String::from_utf8(response.body).map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err)))
		.and_then(|response| {
			let mut wildcard = Vec::new();

			let captures = regex::Regex::new(r#"static +const +char\* +Wildcard\s*\[\s*\]\s*=\s*\{\s*([\s\S]*?)\s*NULL,?\s*};"#)
				.unwrap()
				.captures(response.leak())
				.and_then(|captures| captures.get(1))
				.ok_or_else(|| std::io::Error::other("Failed to parse addon whitelist"))?;

			let line_regex = regex::Regex::new(r#""(.+?)","#).unwrap();

			for line in captures.as_str().lines() {
				let line = line.trim();
				if line.is_empty() {
					continue;
				} else if line == "NULL" {
					break;
				} else if let Some(capture) = line_regex.captures(line) {
					let glob = capture.get(1).unwrap().as_str();
					wildcard.push(&*glob.to_string().leak());
				}
			}

			if wildcard.is_empty() {
				return Err(std::io::Error::other("Failed to parse addon whitelist (empty)"));
			}

			if !wildcard.contains(&"lua/*.lua") {
				// This should definitely be in there, so if it isn't, something has gone wrong. Probably.
				return Err(std::io::Error::other("Failed to parse addon whitelist (missing lua/*.lua)"));
			}

			println!("Downloaded up to date addon whitelist: {wildcard:#?}");

			Ok(&*wildcard.leak())
		})
		.map_err(|err| {
			eprintln!("Failed to download addon whitelist: {:#?}", err);
			err
		})
		.unwrap_or(ADDON_WHITELIST_OFFLINE)
}

const WILD_BYTE: u8 = b'*';
const QUESTION_BYTE: u8 = b'?';
const EXCLAMATION_BYTE: u8 = b'!';

fn globber(wild: &str, str: &str) -> bool {
	let wild = wild.as_bytes();
	let str = str.as_bytes();
	let mut widx = 0;
	let mut sidx = 0;
	let mut star_idx = None;
	let mut saved_sidx = 0;

	while widx < wild.len() && sidx < str.len() && wild[widx] != WILD_BYTE {
		if wild[widx] != str[sidx] && wild[widx] != QUESTION_BYTE {
			return false;
		}
		widx += 1;
		sidx += 1;
	}

	while sidx < str.len() {
		if widx < wild.len() && wild[widx] == WILD_BYTE {
			widx += 1;
			if widx >= wild.len() {
				return true;
			}
			star_idx = Some(widx);
			saved_sidx = sidx + 1;
		} else if widx < wild.len() && (wild[widx] == str[sidx] || wild[widx] == QUESTION_BYTE) {
			widx += 1;
			sidx += 1;
		} else if let Some(star_pos) = star_idx {
			widx = star_pos;
			sidx = saved_sidx;
			saved_sidx += 1;
		} else {
			return false;
		}
	}

	while widx < wild.len() && wild[widx] == WILD_BYTE {
		widx += 1;
	}

	widx >= wild.len()
}

/// Check if a path is allowed in a GMA file
pub fn check(str: &str) -> bool {
	let mut valid = false;

	for glob in ADDON_WHITELIST.iter() {
		if glob.as_bytes().first() == Some(&EXCLAMATION_BYTE) {
			if globber(&glob[1..], str) {
				valid = false;
			}
		} else if !valid && globber(glob, str) {
			valid = true;
		}
	}

	valid
}

/// Check if a path is ignored by a list of custom globs
pub fn is_ignored(str: &str, ignore: &[String]) -> bool {
	for glob in ALWAYS_IGNORED {
		if globber(glob, str) {
			return true;
		}
	}

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
fn test_whitelist() {
	let good: &'static [&'static str] = &[
		"lua/test.lua",
		"lua/lol/test.lua",
		"lua/lua/testing.lua",
		"gamemodes/test/something.txt",
		"gamemodes/test/content/sound/lol.wav",
		"materials/lol.jpeg",
		"gamemodes/the_gamemode_name/backgrounds/file_name.jpg",
		"gamemodes/my_base_defence/backgrounds/1.jpg",
	];

	let bad: &'static [&'static str] = &[
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
		assert!(check(good), "{}", good);
	}

	for good in ADDON_WHITELIST.iter() {
		if good.as_bytes().first() == Some(&EXCLAMATION_BYTE) {
			continue;
		}
		assert!(check(&good.replace('*', "test")));
	}

	for good in ADDON_WHITELIST.iter() {
		if good.as_bytes().first() == Some(&EXCLAMATION_BYTE) {
			continue;
		}
		assert!(check(&good.replace('*', "a")));
	}

	for bad in bad {
		assert!(!check(bad));
	}
}

#[test]
fn test_ignore() {
	assert!(is_ignored("lol.txt", &["lol.txt".to_string()]));
	assert!(is_ignored("lua/hello.lua", &["lua/*.lua".to_string()]));
	assert!(is_ignored("lua/hello.lua", &["lua/*".to_string()]));
	assert!(is_ignored(".gitattributes", &[".git*".to_string()]));
	assert!(!is_ignored("lol.txt", &[]));
	assert!(is_ignored("models/player.sw.vtx", &[]));
	assert!(!is_ignored("models/player.vtx", &[]));
}

#[test]
fn test_exclusions() {
	assert!(check("models/player.vtx"));
	assert!(check("models/weapons/gun.vtx"));

	assert!(!check("models/player.sw.vtx"));
	assert!(!check("models/player.360.vtx"));
	assert!(!check("models/player.xbox.vtx"));
	assert!(!check("models/weapons/gun.sw.vtx"));

	assert!(check("gamemodes/test/content/models/player.vtx"));
	assert!(!check("gamemodes/test/content/models/player.sw.vtx"));
	assert!(!check("gamemodes/test/content/models/player.360.vtx"));
	assert!(!check("gamemodes/test/content/models/player.xbox.vtx"));

	assert!(check("gamemodes/sandbox/info.txt"));
	assert!(check("gamemodes/sandbox/sandbox.fgd"));
	assert!(!check("gamemodes/sandbox/nested/info.txt"));
	assert!(!check("gamemodes/sandbox/entities/weapons/info.txt"));
}
