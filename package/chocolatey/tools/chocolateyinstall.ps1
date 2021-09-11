
$ErrorActionPreference = 'Stop';

$packageName = 'punktf'
$toolsDir   = "$(Split-Path -parent $MyInvocation.MyCommand.Definition)"
$url64       = 'https://github.com/Shemnei/punktf/releases/download/v1.0.1/punktf-x86_64-pc-windows-msvc.zip'
$bin         = $toolsDir + '\punktf-x86_64-pc-windows-msvc\punktf.exe'

$packageArgs = @{
    packageName   = $packageName
    url64bit      = $url64

    softwareName  = 'punktf*'

    unzipLocation = $toolsDir

    checksum64    = 'e67fe62cb03ae62c8b5cddff0d602700aa02e555d3f00b254794d5d13f59aba3'
    checksumType64= 'sha256'

    validExitCodes= @(0)
}
Install-ChocolateyZipPackage @packageArgs
