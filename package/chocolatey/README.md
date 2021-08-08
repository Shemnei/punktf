Chocolatey Punktf package
===

## Release a new version

1. Change url and checksum in `tools/chocolateyinstall.ps1`
2. Adjust version number in `punktf.nuspec`
3. Run `choco pack` in the current directory
4. Run `choco push .\{file}.nupkg --source https://push.chocolatey.org/`
