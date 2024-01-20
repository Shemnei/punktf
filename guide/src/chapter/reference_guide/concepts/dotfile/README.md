# Dotfile

A dotfile, in the sense `punktf` uses it, can be of two different kinds:

- A normal file
- A directory containing files

The options for both are the same (look at [[Profiles|Profiles]] for more details), but they differ in some ways:

1) If a directory get's deployed all files contained within it will also get deployed.
2) The contents of a directory are put in the root of the deployment target directory

	For example:

	- `profile.target`: `/home/demo`
	- `dotfile.path`: `config_linux` (is directory)

	Then all children of `config_linux` will be deployed under `/home/demo` e.g. `/home/demo/.bashrc`

	This behaviour can be influenced in two different ways:

	1) Set `dotfile.overwrite_target`: This will be used instead of `profile.target`. All files will still land in the root.
	2) Set `dotfile.rename`: With this option a name for the directory can be set

		For example:

		- profile.target: /home/demo
		- dotfile.path: config_linux (is directory)
		- dotfile.rename: .config

		Then all children of `config_linux` will be deployed under `/home/demo/.config` e.g. `/home/demo/.config/.bashrc`
