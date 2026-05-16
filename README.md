# Empire at War Mod Repacker

## Description
A CLI tool that automates creating a local copy of
a *Star Wars: Empire at War* mod with eligible contents repacked into MEG files.

Many mods contain a very large amount of files in flat directory hierarchies,
which has been shown to degrade performance when running on Linux under Proton.
This can be alleviated by packing mod files into MEG file bundles, like the base game already does.
This issue and its solution was first solved by https://github.com/Alexeyov/Empire-at-War-Linux-Mod-Tool.

## Why?
It's fun to sometimes liberally apply the principle of "[not invented here](https://en.wikipedia.org/wiki/Not_invented_here)" and discover the square wheel.
On a more serious note, EAW-LMT requires Wine to run, which can be a hassle to setup e.g. on Steam Deck.
This tool also aims to provide a more automated experience OOTB, with autodiscovery for common paths
and other QoL.

## Usage
**NOTE:** you can run `eaw-mod-repack --help` to get (mostly) the below documentation.

Typically one would run the tool providing either the Steam workshop ID or the source directory of the mod to repack, e.g.:
```bash
# repack Thrawn's Revenge
$ eaw-mod-repack --steam-mod-id 1125571106
# repack some local mod
$ eaw-mod-repack --source-dir path/to/some/mod
```
Optionally specify the Steam library root if you installed EaW in a non-default Steam library folder:
```bash
# repack Thrawn's Revenge in a custom Steam library
$ eaw-mod-repack --steam-library-root path/to/my/SteamLibrary --steam-mod-id 1125571106 
```

Using this for submods is probably redundant since submods tend to contain relatively few files.
Trying to pack submods may be possibly detrimental, since the tool has no knowledge of your mod load order and therefore cannot determine which files should be exluded from packing.
This could cause submod overrides to not take effect.

## Acknowledgements
The author would like to thank Alexey Skywalker for developing the original Empire At War Linux ModTool
and documenting the underlying issues around running Empire at War on Linux.
Empire At War Linux ModTool can be found at https://github.com/Alexeyov/Empire-at-War-Linux-Mod-Tool.

## Copyright
```
Copyright 2026 the eaw-mod-repack contributors.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```