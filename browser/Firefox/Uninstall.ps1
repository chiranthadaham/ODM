$ErrorActionPreference = 'Stop'
$key='HKCU:\Software\Mozilla\NativeMessagingHosts\com.opendownloadmanager'
$manifest=Join-Path $env:LOCALAPPDATA 'Programs/Open Download Manager/Firefox/native-host.json'
if (Test-Path -LiteralPath $key) {
    $registered=(Get-Item -LiteralPath $key).GetValue('')
    if ($registered -eq $manifest) { Remove-Item -LiteralPath $key; Write-Host 'Firefox connection removed.' }
    else { Write-Host 'A different installation is registered; it was left in place.' }
}
Write-Host 'Remove the add-on separately from Firefox Add-ons if it is installed.'
