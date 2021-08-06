
$ErrorActionPreference = 'Stop';

$packageName = 'punktf'
$destDir     = Join-Path $(Get-ToolsLocation) $packageName
$url64       = 'https://github.com/neovim/neovim/releases/download/release/punktf.exe'


$packageArgs = @{
    packageName   = $packageName
    installerType = 'exe'
    url64bit      = $url64

    softwareName  = 'punktf*'

    checksum64    = ''
    checksumType64= 'sha256'

    validExitCodes= @(0)
}

Install-ChocolateyPackage @packageArgs
