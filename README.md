## Commands

```shell
# update
punktf update

# deploy (dry-run)
punktf deploy windows --dry-run

# deploy filtered???
punktf deploy windows --filter "file LIKE '%.win'"

# deploy single
punktf deploy windows --single 'init.vim.win'
```

## Layout

```
+ profiles\
	+ windows.pfp
+ files\
	+ init.vim.win
```

## PFP Format (PunktF profile)

```json5
{
	// Either read from ENVIRONMENT (std::env::var) or from here
	"env": [
		{
			"key": "RUSTC_PATH",
			"value": "/usr/bin/rustc",
		}
		//, ...
	],
	// Target path of config dir; used when no specific deploy_location was given
	"target": "/home/demo/.config",
	// Files to be deployed
	"files": [
		{
			// Relative path in `files/`
			"file": "init.vim.win",
			// OPT: Alternative deploy location (PATH: used instead of `root` + `file`, ALIAS: `root` + alias instead of `file`)
			"deploy_location": {
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
			// OPT: Higher priority overwrites files from lower files
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
{{@if {{}} == "windows"}}
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
- Source (profile/file) via argument or ENVIRONMENT
- have other templates as base for others
