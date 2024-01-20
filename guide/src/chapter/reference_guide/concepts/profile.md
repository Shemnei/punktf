# Profile

Profiles can be either a `json` or `yaml` file.

## Paths

All paths that are given via the profile can have system/environment variables embedded.
They start with a `$` and then the name (e.g. `$HOME`).
Optionally they can also be enclosed with braces (e.g. `${HOME}`).
This syntax is both valid/used for Unix and Windows systems (instead of Windows `%...%` syntax).

Additionally, paths can start with a `~` which corresponds to the user home directory:

- `Unix`: `/home/test` (`$HOME`)
- `Windows`: `C:\Users\test`

## Layout

### Yaml

```yaml
# Optional: Other profiles which will be used as base for this one. The order in which they are specified matters, the higher up the higher the priority for overwrites of values.
# Default: None
extends:
  - base_profile_name

# Optional: Variables for all `dotfiles`
# Default: None
# DON'T add '-' infront of the variable names (e.g. - OS: "linux")
variables:
  RUSTC_PATH: "/usr/bin/rustc"
  OS: "linux"


# Optional: Content transformer. These will take the content of a dotfile, process it and return a new version of it.
transformers:
  - LineTerminator: CRLF

# Optional: Target path of config dir; used when no specific deploy_location was given
# Default: `$PUNKTF_TARGET`
target: "/home/demo/.config"

# Optional: Hooks which are executed once before the deployment.
# Default: None
pre_hooks:
  - echo "Foo"

# Optional: Hooks which are executed once after the deployment.
# Default: None
post_hooks:
  - echo "Bar"

# `dotfiles` to be deployed
dotfiles:
    # Relative path in `dotfiles/`
  - path: init.vim.win

	# Optional: Alternative name for the dotfile. This name will be used instead of [`Dotfile::path`] when
	# deploying. If this is set and the dotfile is a folder, it will be deployed under the given
	# name and not in the root source directory.
	# Default: None
	rename: init.vim

	# Optional: Alternative deploy target path. This will be used instead of [`Profile::target`] when
	# deploying.
	# Default: None
	overwrite_target: "/home/demo/.config/nvim"

	# Optional: Custom variables for the specific file (same as above)
	# Default: None
	variables: []

	# Optional: Content transformer. These will take the content of a dotfile, process it and return a new version of it.
	transformers:
	- LineTerminator: CRLF

	# Optional: Merge operation/kind (like: Ask, Keep, Overwrite)
	# Default: Overwrite
	merge: Overwrite

	# Optional: Whether this file is a template or not (skips template actions (replace, ...) if not)
	# Default: true
	template: false

	# Optional: Higher priority `dotfile` is allowed to overwrite lower priority one
	# Default: None
	priority: 2

# Symlinks to be created
links:
	# Absolute path to target of the link
  - source_path: "$HOME/configurations"
	# Absolute path to the source of the link
	target_path: "~/.config"
	# Optional: Will replace existing symlink at target (overwrite). But only if the file at the target is a symlink.
	# Default: true
	replace: false
```

### Json

```json5
{
	"extends": [
        "base_profile_name"
    ],
	"variables": {
		"RUSTC_PATH": "/usr/bin/rustc",
		"OS": "linux",
		//, ...
	},
    "transformers": [
        { "LineTerminator": "CRLF" }
    ],
	"target": "/home/demo/.config",
	"pre_hooks": ["echo \"Foo\""],
	"post_hooks": ["echo \"Bar\""],
	"dotfiles": [
		{
			"path": "init.vim.linux",
			"rename": "init.vim",
			"overwrite_target": "/home/demo/.config/nvim"
			"variables": {
				//...
			},
            "transformers": [
                 { "LineTerminator": "CRLF" }
            ],
			"merge": "Overwrite",
			"template": false,
			"priority": 2,
		}
		//, ...
	]
}
```
