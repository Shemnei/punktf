
$ErrorActionPreference = 'Stop';

$packageName = 'punktf'
$destDir     = Join-Path $(Get-ToolsLocation) $packageName
$url64       = 'https://github.com/Shemnei/punktf/releases/download/v1.0.0-alpha/punktf-x86_64-pc-windows-gnu.zip'


$packageArgs = @{
    packageName   = $packageName
    installerType = 'exe'
    url64bit      = $url64

    softwareName  = 'punktf*'

    checksum64    = '74e7c124eba7402c7cedc6cc71c2f7bbbf5e81b957fe2a05d8d4deced82d1115'
    checksumType64= 'sha256'

    validExitCodes= @(0)
}

Install-ChocolateyPackage @packageArgs
