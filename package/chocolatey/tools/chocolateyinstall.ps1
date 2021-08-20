
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

    checksum64    = '6dcbb94b4993f746636ebc4050f866ae91b115146daea13818724c5ca6f373c5'
    checksumType64= 'sha256'

    validExitCodes= @(0)
}
Install-ChocolateyZipPackage @packageArgs

Install-BinFile -Name "punktf" -Path "$bin"
