<h1 align="center">⚙️ fastgmad</h1>
<p align="center"><a href="https://github.com/WilliamVenner/fastgmad/releases">Download</a></p>
<br/>

An extremely fast reimplementation of gmad.exe and gmpublish.exe.

###### Prefer to use a GUI? Check out [gmpublisher](https://github.com/WilliamVenner/gmpublisher)

## Features

* Up to **x100 faster** than gmad.exe
* Create, extract and publish GMAs **all in one tool**
* Drop-in replacement for gmad.exe and gmpublish.exe - **usage is identical**
* Upload addon icons in PNG, JPG or **even GIF format**
* `-stdin` and `-stdout` support for piping data between tools

## Requirements

Windows, macOS or Linux

# Usage
<!--BEGINUSAGE><!-->
```
https://wiki.facepunch.com/gmod/Workshop_Addon_Creation
https://wiki.facepunch.com/gmod/Workshop_Addon_Updating

Drag & Drop
-----------
Drag & drop a .gma onto fastgmad to extract it
Drag & drop a folder onto fastgmad to convert it to .gma

Creating GMAs
-------------
fastgmad create -folder path/to/folder -out path/to/gma.gma
fastgmad create -folder path/to/folder -out path/to/gma.gma
fastgmad create -folder path/to/folder
fastgmad create -folder path/to/folder -stdout

Extracting GMAs
---------------
fastgmad extract -file path/to/gma.gma -out path/to/folder
fastgmad extract -file path/to/gma.gma
fastgmad extract -stdin -out path/to/folder

Publishing GMAs
---------------
>> Adding an icon is OPTIONAL for publishing a new Workshop addon. A default icon will be provided for you if you don't add one.
Accepted Icon Formats: JPG, PNG, GIF
Icon Max Size: 1 MB
Recommended Icon Dimensions: 512x512

fastgmad publish -addon path/to/gma.gma -icon path/to/icon
fastgmad update -id 1337 -addon path/to/gma.gma
fastgmad update -id 1337 -addon path/to/gma.gma -icon path/to/icon
fastgmad update -id 1337 -addon path/to/gma.gma -changes "fixed something"
fastgmad update -id 1337 -addon path/to/gma.gma -changes "fixed something" -icon path/to/icon

Additional flags
----------------
-max-io-threads <integer> - The maximum number of threads to use for reading and writing files. Defaults to the number of logical cores on the system.
-max-io-memory-usage <integer> - The maximum amount of memory to use for reading and writing files in parallel. Defaults to 2 GiB.
-warninvalid - Warns rather than errors if the GMA contains invalid files. Off by default.
-noprogress - Turns off progress bars.

Notes
-----
- CRC checking and computation is not a feature. Implementing this would slow down the program for no benefit and it is virtually unused and redundant in Garry's Mod.
```
<!--ENDUSAGE><!-->

<br/>
<p align="center"><img src="https://i.imgur.com/Un4akZe.gif"/></p>