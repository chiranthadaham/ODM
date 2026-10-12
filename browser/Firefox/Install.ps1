$ErrorActionPreference='Stop'
$manifest=Join-Path $env:LOCALAPPDATA 'Programs/Open Download Manager/Firefox/native-host.json'
if(-not(Test-Path -LiteralPath $manifest)){throw 'Install ODM using ODM-Setup-0.2.0.exe first.'}
$hostInfo=Get-Content -LiteralPath $manifest -Raw|ConvertFrom-Json
if(-not(Test-Path -LiteralPath $hostInfo.path)){throw 'Repair the ODM installation.'}
$key='HKCU:\Software\Mozilla\NativeMessagingHosts\com.opendownloadmanager'
New-Item -Path $key -Force|Out-Null
Set-Item -LiteralPath $key -Value $manifest
Write-Host 'Connected to the installed ODM app. Use Browser plug-in in ODM to open the official store.'
