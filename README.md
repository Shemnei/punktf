# PunktF - A cross-platform multi-target dotfiles manager 

## Commands

```shell
# deploy (dry-run)
punktf deploy windows --dry-run

# deploy (custom home folder)
punktf --home /home/demo deploy windows

# deploy (custom home folder)
PUNKTF_HOME=/home/demo punktf deploy windows
```

## PunktF Home

PunktF searches for the home path in the following order:

1) CLI argument given with `-h/--home`
2) Environment variable `PUNKTF_HOME`
3) Current working directory of the shell

```
+ profiles\
	+ windows.pfp
+ item\
	+ init.vim.win
```

## PFP Format (PunktF profile)

```json5
{
	// OPT: Either read from ENVIRONMENT (std::env::var) or from here
	"env": [
		{
			"key": "RUSTC_PATH",
			"value": "/usr/bin/rustc",
		}
		//, ...
	],

	// Target path of config dir; used when no specific deploy_location was given
	"target": "/home/demo/.config",

	// OPT: Hook which is executed once before the deployment.
	"pre_hook": "echo \"Foo\"",

	// OPT: Hook which is executed once after the deployment.
	"pre_hook": "echo \"Bar\"",

	// Items to be deployed
	"items": [
		{
			// Relative path in `items/`
			"path": "init.vim.win",

			// OPT: Alternative deploy target (PATH: used instead of `root` + `file`, ALIAS: `root` + (alias instead of `file`))
			"target": {
				"kind": "alias",
				"value": "init.vim",
			},

			// OPT: Custom env for the specific file (same as above)
			"env": [
				...
			],

			// OPT: Merge operation/kind (like: overwrite_all, ask, keep, overwrite_deployed)
			"merge": "overwrite",

			// OPT: Wether this file is a template or not (skips template actions (replace, ..) if not)
			"template": false,

			// OPT: Higher priority item is allowed to overwrite lower priority ones
			"priority": 2,
		}
		//, ...
	]
}
```

## Template Format

### Replacement

Prefix (can be combined: e.g. {{#$RUSTC_PATH}}):

- None: First profile.env then profile.file.env
- `$`: Only ENVIRONMENT
- `#`: Only env
- `&`: Only file.env

```python
rustc = {{RUSTC_PATH}}
```

### Conditionals (TODO: think about structure)

```python
{{@if {{OS}} == "windows"}}
	print("running on windows")
{{@else}}
	print("NOT running on windows")
{{@if}}
```

## Future

- File permissions/mode
- Content transformer: take file as input change it and return it (e.g. replace CRLF => LF)
- Remember last deployed files and only overwrite them if they are the same as they were
	- sqlite or json with previous deployments
- Generate profile from directory structure
- Have templates as base for others
