$ErrorActionPreference = 'Stop'
$app = Join-Path (Split-Path -Parent $PSScriptRoot) 'OpenDownloadManager.exe'
if (-not (Test-Path -LiteralPath $app)) { throw 'Keep the Firefox folder beside OpenDownloadManager.exe.' }
$manifest = Join-Path $PSScriptRoot 'native-host.json'
@{ name='com.opendownloadmanager'; description='Open Download Manager Firefox bridge'; path=$app; type='stdio'; allowed_extensions=@('opendownloadmanager@local') } | ConvertTo-Json | Set-Content -LiteralPath $manifest -Encoding UTF8
$key='HKCU:\Software\Mozilla\NativeMessagingHosts\com.opendownloadmanager'
New-Item -Path $key -Force | Out-Null
Set-Item -LiteralPath $key -Value $manifest
Write-Host 'Firefox connection registered for this user. No administrator access is needed.'
Write-Host 'Next: open about:debugging in Firefox, choose This Firefox, then Load Temporary Add-on.'
Write-Host 'Select Firefox\extension\manifest.json. See Firefox\README.txt for details.'
