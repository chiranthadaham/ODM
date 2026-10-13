$ErrorActionPreference='Stop'
$tokens=$null;$errors=$null
$ast=[System.Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot '../updater/update.ps1'),[ref]$tokens,[ref]$errors)
if($errors.Count){throw ($errors | Out-String)}
$fn=$ast.Find({param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq 'Select-Update'},$true)
Invoke-Expression $fn.Extent.Text
$release=@{tag_name='v0.3.0';draft=$false;prerelease=$false;assets=@(@{name='ODM-Setup-0.3.0.exe';state='uploaded';size=100;digest=('sha256:'+('a'*64));browser_download_url='https://github.com/chiranthadaham/ODM/releases/download/v0.3.0/ODM-Setup-0.3.0.exe'})}
if((Select-Update $release ([version]'0.2.9')).Version -ne '0.3.0'){throw 'New version not selected'}
if($null -ne (Select-Update $release ([version]'0.3.0'))){throw 'Same version selected'}
if($null -ne (Select-Update $release ([version]'0.4.0'))){throw 'Downgrade selected'}
$release.assets[0].digest=''
try {Select-Update $release ([version]'0.2.9');throw 'TEST FAILED: missing digest accepted'} catch {if($_ -like '*TEST FAILED*'){throw}}
$release.assets[0].digest='sha256:'+('a'*64)
$release.assets[0].browser_download_url='https://evil.example/setup.exe'
try {Select-Update $release ([version]'0.2.9');throw 'TEST FAILED: foreign URL accepted'} catch {if($_ -like '*TEST FAILED*'){throw}}
$release.prerelease=$true
try {Select-Update $release ([version]'0.2.9');throw 'TEST FAILED: prerelease accepted'} catch {if($_ -like '*TEST FAILED*'){throw}}
'PASS: updater syntax, newer/same/older versions, missing digest, foreign URL, and prerelease guards.'
