# Examples

## Structure

For each example, a separate folder should be created in the `examples` directory.
To automatically test the deployment of the profile, create a script named `run.sh` in the
newly created example folder (e.g. `examples/01_test/run.sh`).

The example folder should start with a number which determines the execution order and also the level of `advancedness` of the example (e.g. `10_extending_profiles`).

## Examples

- `01_single_file`: Deploys a single file dotfile
- `02_single_dir`: Deploys a single directory dotfile
- `03_single_template`: Deploys a single template dotfile
- `10_extending_profiles`: Deploys a profile which extends a base profile
- `80_simple_complete`: Deploys a really simple "real world" example
- `85_multi_os`: Holds profiles and dotfiles for a windows and linux host system with shared dotfiles.
