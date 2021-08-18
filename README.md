# punktf - A cross-platform multi-target dotfiles manager
[![MIT License](https://img.shields.io/crates/l/punktf)](https://choosealicense.com/licenses/mit/) [![GitHub Issues](https://img.shields.io/github/issues/Shemnei/punktf)](https://github.com/Shemnei/punktf/issues?q=is%3Aissue+is%3Aopen+sort%3Aupdated-desc) [![Continuous integration](https://github.com/Shemnei/punktf/workflows/Continuous%20Integration/badge.svg)](https://github.com/Shemnei/punktf/actions) [![Crates.io](https://img.shields.io/crates/v/punktf)](https://crates.io/crates/punktf) [![Homebrew](https://img.shields.io/badge/dynamic/json.svg?url=https://raw.githubusercontent.com/michidk/homebrew-tools/main/Info/punktf.json&query=$.versions.stable&label=homebrew)](https://github.com/michidk/homebrew-tools/blob/main/Formula/punktf.rb) [![AUR](https://img.shields.io/aur/version/punktf)](https://aur.archlinux.org/packages/punktf) [![Chocolatey](https://img.shields.io/chocolatey/v/punktf?include_prereleases)](https://community.chocolatey.org/packages/punktf)

## Yet another dotfile manager?!

Well, yes, but hear me out: This project was driven by the personal need of having to manage several dotfiles for different machines/targets. You want the same experience everywhere: On your Windows workstation along with an Ubuntu WSL instance, your Debian server and your private Arch installation. This tool fixes that problem while being cross-platform and blazingly fast. You won't need multiple sets of dotfile configurations ever again!

Features:

- Compile and deploy your dotfiles with one command across different platforms
- Use handlebar-like instructions to insert variables and compile sections conditionally
- Define pre- and post-hooks to customize the behavior with your own commands
- Create multiple profiles for different targets
- Works on Windows and Linux

## Installation

### Homebrew

Install [punktf using Homebrew](https://github.com/michidk/homebrew-tools/blob/main/Formula/punktf.rb) on Linux:

```sh
brew tap michidk/tools
brew install punktf
```

### AUR

Install [punktf from AUR](https://aur.archlinux.org/packages/punktf) on Arch Linux.
To install it use your favorite AUR capable package manager (e.g. [yay](https://github.com/Jguer/yay), [pikaur](https://github.com/actionless/pikaur)):

```sh
yay punktf # using yay
```

or

```sh
pikaur -S punktf #
```

### Chocolatey

Install [punktf using Chocolatey](https://community.chocolatey.org/packages/punktf) on Windows:

```sh
choco install punktf --pre
```

## Usage

### Commands

To deploy a profile, use the `deploy` subcommand:

```sh
# deploy 'windows' profile
punktf deploy windows

# deploy (custom source folder)
punktf --source /home/demo/mydotfiles deploy windows
```

Adding the `-h`/`--help` flag to a given subcommand, will print usage instructions.

### Source Folder

The punktf source folder, is the folder containing the dotfiles and punktf profiles. We recommend setting the `PUNKTF_SOURCE` environment variable, so that the dotfiles can be compiled using `punktf deploy <profile>`.

punktf searches for the source folder in the following order:

1. CLI argument given with `-s`/`--source`
2. Environment variable `PUNKTF_SOURCE`
3. Current working directory of the shell

The source folder should contain two sub-folders:

* `profiles\`: Contains the punktf profile definitions (`.yaml` or `.json`)
* `dotfiles\`: Contains folders and the actual dotfiles

Example punktf source folder structure:

```ls
+ profiles
	+ windows.yaml
	+ base.yaml
	+ arch.json
+ dotfiles
	+ .gitconfig
	+ init.vim.win
	+ base
		+ demo.txt
	+ linux
		+ .bashrc
	+ windows
		+ alacritty.yml
```

### Target

Determines where `punktf` will deploy files too.
It can be set with:

1. Variable `target` in the punktf profile file
2. Environment variable `PUNKTF_TARGET`

### Profiles

Profiles define which dotfiles should be used. They can be a `.json` or `.yaml` file.

Example punktf profile:

```yaml
variables:
  OS: "windows"

target: "C:\\Users\\Demo"

dotfiles:
  - path: "base"
  - path: "windows/alacritty.yml"
    target:
    	Path: "C:\\Users\\Demo\\AppData\\Local\\alacritty.yml"
    merge: Ask
```

All properties are explained [in the wiki](https://github.com/Shemnei/punktf/wiki/Profiles).

## Templates

Please refer to the [wiki](https://github.com/Shemnei/punktf/wiki/Templating) for the templating syntax.

## Dotfile Repositories using punktf

- [michidk/dotfiles](https://gitlab.com/michidk/dotfiles)
