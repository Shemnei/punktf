# Command

The main command is `punktf`.

```
USAGE:
    punktf [FLAGS] [OPTIONS] <SUBCOMMAND>

FLAGS:
    -h, --help
            Prints help information

    -v, --verbose
            Runs with specified level of verbosity which affects the log level.

            The level can be set by repeating the flag `n` times (e.g. `-vv` for 2). Levels: 0 -
            `Info`; 1 - `Debug`; 2 - `Trace`.

    -V, --version
            Prints version information


OPTIONS:
    -s, --source <source>
            The source directory where the profiles and dotfiles are located [env:
            PUNKTF_SOURCE=] [default:]
```

# Subcommands

Available subcommands:
- `deploy`: Compiles and deploys the dotfiles
- `help`: Prints usage instructions

## Deploy

Used to deploy punktf profiles.

```
USAGE:
    punktf deploy [FLAGS] [OPTIONS] <PROFILE>

ARGS:
    <PROFILE>
            Name of the profile to deploy.

            The name should be the file name of the profile without an extension (e.g.
            `profiles/arch.json` should be given as `arch`). [env: PUNKTF_PROFILE=]

FLAGS:
    -d, --dry-run
            Deploys the profile but without actually coping/creating the files.

            This can be used to test and get an overview over the changes which would be applied
            when run without this flag.

    -h, --help
            Print help information

    -V, --version
            Print version information

OPTIONS:
    -t, --target <TARGET>
            Alternative deployment target path.

            This path will take precendence over all other ways to define a deployment path.
```

Example commands:

```sh
# deploy 'windows' profile
punktf deploy windows

# deploy (custom source folder)
punktf --source /home/demo/mydotfiles deploy windows
```
