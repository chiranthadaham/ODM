$ErrorActionPreference='Stop'
$app=Join-Path (Split-Path -Parent $PSScriptRoot) 'OpenDownloadManager.exe'
if(-not(Test-Path -LiteralPath $app)){throw 'Keep Chromium beside OpenDownloadManager.exe.'}
$manifest=Join-Path $PSScriptRoot 'native-host.json'
@{name='com.opendownloadmanager';description='Open Download Manager Chromium bridge';path=$app;type='stdio';allowed_origins=@('chrome-extension://jancefngmifbnaldahilleoinncjppjh/')}|ConvertTo-Json|Set-Content -LiteralPath $manifest -Encoding UTF8
foreach($product in @('Google\Chrome','Chromium','Microsoft\Edge','BraveSoftware\Brave-Browser','Vivaldi')){
 $key='HKCU:\Software\'+$product+'\NativeMessagingHosts\com.opendownloadmanager'
 New-Item -Path $key -Force|Out-Null;Set-Item -LiteralPath $key -Value $manifest
}
Write-Host 'Chrome, Edge, Brave, Vivaldi and Chromium app connections registered.'
Write-Host 'Open your browser extensions page, enable Developer mode, choose Load unpacked, and select Chromium\extension.'
