# PunktF - A cross-platform multi-target dotfiles manager

[![MIT License](https://img.shields.io/crates/l/punktf)](https://choosealicense.com/licenses/mit/) [![Continuous integration](https://github.com/Shemnei/punktf/workflows/Continuous%20Integration/badge.svg)](https://github.com/Shemnei/punktf/actions) [![Crates.io](https://img.shields.io/crates/v/punktf)](https://crates.io/crates/punktf)

## DISCLAIMER

This crate is sill under development and not all features are currently implemented.
Layouts and formats can and will change while in development.

The following features are already implemented:

- [x] Basic deployment process
- [x] Basic templating support
- [x] Reading from a profile file
- [x] Depolying `dotfiles`
- [x] Directories can be used as a `dotfile`
- [x] Profiles can have another profile as a base
- [x] Pre/Post deployment hooks
- [x] Basic support for merge operations

Before you try this tool, please make sure to save/backup your existing setup.
While in deployment there are likely bugs which can and will mess up your
existing setup.

## Yet another dotfile manager?!

Well yes, but hear me out. This project was driven by the personal need of having to manage several dotfiles for different machines/targets. You want the same experience everywhere: On your work Windows machine along with an Ubuntu WSL instance, your Debian server and your private Arch installation. This tool fixes that problem while beeing cross-platform and blazingly fast.

Features:
- [ ] Merge mutliple layers of dotfiles
- [ ] Create profiles for different targets
- [ ] Use instructions to compile your dotfiles/templates conditionally
- [ ] Use hadlebar-like instructions to insert variables and more
- [ ] Define pre- and post-hooks to customize the behaviour with custom commands
- [ ] Handles file permissions and line endings (CRLF vs LF)

## Commands

```shell
# deploy (dry-run)
punktf deploy windows --dry-run

# deploy (custom source folder)
punktf --source /home/demo deploy windows

# deploy (custom home folder)
PUNKTF_SOURCE=/home/demo punktf deploy windows
```

## PunktF Source

PunktF searches for the source path in the following order:

1) CLI argument given with `-s/--source`
2) Environment variable `PUNKTF_SOURCE`
3) Current working directory of the shell

```
+ profiles\
	+ windows.pfp
+ dotfiles\
	+ init.vim.win
```

## PunktF Target

Determines where `punktf` will deploy files too.
It can be set with:

1) Variable `target` in profile file
2) Environment variable `PUNKTF_TARGET`

## PunktF profile (either json or yaml)

```json5
{
	// OPT: Other profile which will be used as base for this one
	// Default: None
	"extends": "base_profile_name",

	// OPT: Variables for all `dotfiles`
	// Default: None
	"variables: [
		{
			"key": "RUSTC_PATH",
			"value": "/usr/bin/rustc",
		}
		//, ...
	],

	// OPT: Target path of config dir; used when no specific deploy_location was given
	// Default: `PUNKTF_TARGET`
	"target": "/home/demo/.config",

	// OPT: Hooks which are executed once before the deployment.
	// Default: None
	"pre_hooks": ["echo \"Foo\""],

	// OPT: Hooks which are executed once after the deployment.
	// Default: None
	"post_hooks": ["echo \"Bar\""],

	// `dotfiles` to be deployed
	"dotfiles": [
		{
			// Relative path in `dotfiles/`
			"path": "init.vim.win",

			// OPT: Alternative deploy target (PATH: used instead of `root` + `file`, ALIAS: `root` + (alias instead of `file`))
			// Default: None
			"target": {
				"kind": "alias",
				"value": "init.vim",
			},

			// OPT: Custom variables for the specific file (same as above)
			// Default: None
			"variables": [
				...
			],

			// OPT: Merge operation/kind (like: Ask, Keep, Overwrite)
			// Default: Overwrite
			"merge": "Overwrite",

			// OPT: Wether this file is a template or not (skips template actions (replace, ..) if not)
			// Default: true
			"template": false,

			// OPT: Higher priority `dotfile` is allowed to overwrite lower priority one
			// Default: None
			"priority": 2,
		}
		//, ...
	]
}
```

## Template Format

### Comments

Comments can be inserted with `{{!-- ... --}}`. They will be ignored by the template
parser and will not be transferred to the output.

Example:

```handlebars
{{!-- Inserts the current os name and prints it when executed --}}
print("{{OS}}")
```

### Escaped

If `{{` or `}}` need to used outside of a template block, put them inside an
escaped block. Everything within it will get copied over without modification.

Example:

```handlebars
{{{ This is escaped ... I can use {{ without worry. I can even use }} and is still fine }}}
```

### Variables

Prefix to determine where variables are looked for (can be combined: e.g. {{#$RUSTC_PATH}}):

- None: First profile.variables then profile.file.variables
- `$`: Only (system) ENVIRONMENT
- `#`: Only profile.variables
- `&`: Only profile.dotfile.variables


Example:

```handlebars
rustc = {{RUSTC_PATH}}
```

### Conditionals

Supported are only if expressions with the following structure:

- Check if value of `VAR` is (not) equal to `LITERAL`: `{{VAR}} (==|!=) "LITERAL"`
- Check if a value for `VAR` exists: `{{VAR}}`


Example:

```handlebars
{{@if {{OS}}}}
	{{@if {{OS}} == "windows"}}
		print("running on windows")
	{{@elif {{OS}} == "linux"}}
		print("running on linux")
	{{@else}}
		print("NOT running on windows/linux")
	{{@fi}}
{{@fi}}
```
