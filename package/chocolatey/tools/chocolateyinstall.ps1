
$ErrorActionPreference = 'Stop';

$packageName = 'punktf'
$toolsDir   = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"
$url64       = 'https://github.com/Shemnei/punktf/releases/download/v1.0.0-alpha/punktf-x86_64-pc-windows-msvc.zip'
$bin         = $toolsDir + '\punktf-x86_64-pc-windows-msvc\punktf.exe'

$packageArgs = @{
    packageName   = $packageName
    url64bit      = $url64

    softwareName  = 'punktf*'

    unzipLocation = $toolsDir

    checksum64    = 'e4cf1d9ed43217df69b7e13659435b86614ac62ea6049fc079785e993b8c5b0f'
    checksumType64= 'sha256'

    validExitCodes= @(0)
}
Install-ChocolateyZipPackage @packageArgs

Install-BinFile -Name "punktf" -Path "$bin"
