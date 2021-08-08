
$ErrorActionPreference = 'Stop';

$packageName = 'punktf'
$destDir     = Join-Path $(Get-ToolsLocation) $packageName
$url64       = 'https://github.com/Shemnei/punktf/releases/download/v1.0.0-alpha/punktf-x86_64-pc-windows-msvc.zip'


$packageArgs = @{
    packageName   = $packageName
    installerType = 'exe'
    url64bit      = $url64

    softwareName  = 'punktf*'

    checksum64    = 'e4cf1d9ed43217df69b7e13659435b86614ac62ea6049fc079785e993b8c5b0f'
    checksumType64= 'sha256'

    validExitCodes= @(0)
}

Install-ChocolateyPackage @packageArgs
