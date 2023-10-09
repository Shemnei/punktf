# `punktf` - A multi-target dotfiles manager
[![MIT License](https://img.shields.io/crates/l/punktf)](https://choosealicense.com/licenses/mit/) [![Continuous Integration](https://github.com/Shemnei/punktf/workflows/CI/badge.svg)](https://github.com/Shemnei/punktf/actions) [![rust docs](https://docs.rs/punktf-lib/badge.svg)](https://docs.rs/punktf-lib/latest/punktf_lib/) [![Crates.io](https://img.shields.io/crates/v/punktf)](https://crates.io/crates/punktf) [![Homebrew](https://img.shields.io/badge/homebrew-available-blue?style=flat)](https://github.com/michidk/homebrew-tools/blob/main/Formula/punktf.rb) [![AUR](https://img.shields.io/aur/version/punktf)](https://aur.archlinux.org/packages/punktf) [![Chocolatey](https://img.shields.io/chocolatey/v/punktf?include_prereleases)](https://community.chocolatey.org/packages/punktf)

## Yet another dotfile manager?!

Well, yes, but hear me out: This project was driven by the personal need of having to manage several dotfiles for different machines/targets. You want the same experience everywhere: On your Windows workstation along with an Ubuntu WSL instance, your Debian server and your private Arch installation. This tool fixes that problem while being cross-platform and blazingly fast. You won't need multiple sets of dotfile configurations ever again!

Features:

- Compile and deploy your dotfiles with one command across different platforms
- Use handlebar-like instructions to insert variables and compile sections conditionally
- Define pre- and post-hooks to customize the behavior with your own commands
- Create multiple profiles for different targets
- Works on Windows and Linux

## Installation

[![Packaging status](https://repology.org/badge/vertical-allrepos/punktf.svg)](https://repology.org/project/punktf/versions)

### [Homebrew](https://brew.sh/)

Install [punktf using Homebrew](https://github.com/michidk/homebrew-tools/blob/main/Formula/punktf.rb) on Linux:

```sh
brew install michidk/tools/punktf
```

### [AUR](https://aur.archlinux.org/)

Install [punktf from AUR](https://aur.archlinux.org/packages/punktf) on Arch Linux.

To install it use your favorite AUR capable package manager (e.g. [yay](https://github.com/Jguer/yay), [pikaur](https://github.com/actionless/pikaur)):

**NOTE:** As this builds `punktf` from source an up-to-date rust installation is needed.

```sh
yay punktf
```

or

```sh
pikaur -S punktf
```

### [Scoop](https://scoop.sh/)

Install [punktf using Scoop](https://github.com/Shemnei/scoop-bucket/blob/main/bucket/punktf.json) on Windows:

```sh
scoop bucket add shemnei https://github.com/Shemnei/scoop-bucket

scoop install punktf
```

### [Chocolatey](https://chocolatey.org/)

Install [punktf using Chocolatey](https://community.chocolatey.org/packages/punktf) on Windows:

```sh
choco install punktf
```

### [Cargo & Crates.io](https://crates.io/)

Install [punktf using cargo and crates.io](https://crates.io/crates/punktf) on Windows and Linux:

```sh
cargo install punktf
```

## Building from source

To install `punktf` from source the following is needed:

- An up-to-date rust installation
- An installed nightly toolchain

```bash
# Clone
git clone https://github.com/Shemnei/punktf
cd punktf

# Build (cargo)
cargo build --release
```

## Usage

### Commands

To deploy a profile, use the `deploy` subcommand:

```sh
# deploy 'windows' profile
punktf deploy --profile windows

# deploy (custom source folder)
punktf deploy --source /home/demo/mydotfiles --profile windows
```

Adding the `-h`/`--help` flag to a given subcommand, will print usage instructions.

### Source Folder

The `punktf` source folder is the folder containing the dotfiles and `punktf` profiles. We recommend setting the `PUNKTF_SOURCE` environment variable so that the dotfiles can be compiled using `punktf deploy <profile>`.

`punktf` searches for the source folder in the following order:

1. Paths specified with `-s`/`--source`
2. Paths specified by an environment variable `PUNKTF_SOURCE`
3. The current working directory of the shell

The source folder should contain two sub-folders:

- `profiles\`: Contains the `punktf` profile definitions (`.yaml` or `.json`)
- `dotfiles\`: Contains folders and the actual dotfiles

Example `punktf` source folder structure:

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

1. Variable `target` in the `punktf` profile file
2. Environment variable `PUNKTF_TARGET`

### Profiles

Profiles define which dotfiles should be used. They can be a `.json` or `.yaml` file.

Example `punktf` profile:

```yaml
variables:
  OS: "windows"

target: "C:\\Users\\Demo"

dotfiles:
  - path: "base"
  - path: "windows/alacritty.yml"
    target:
      path: "C:\\Users\\Demo\\AppData\\Local\\alacritty.yml"
    merge: Ask

links:
  - source_path: "C:\\Users\\Demo\\Dotfiles\\test.txt"
    target_path: "C:\\Users\\Demo\\test.txt"
```

All properties are explained [in the wiki](https://shemnei.github.io/punktf/chapter/reference_guide/concepts/profile.html).

## Templates

Please refer to the [wiki](https://shemnei.github.io/punktf/chapter/reference_guide/concepts/dotfile/template.html) for the templating syntax.

## Dotfile Repositories using punktf

- [michidk/dotfiles](https://github.com/michidk/dotfiles)

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms
or conditions.
