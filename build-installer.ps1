param([Parameter(Mandatory=$true)][string]$ISCC)
$ErrorActionPreference='Stop'
$bin=Join-Path (Split-Path -Parent $PSScriptRoot) 'Bin'
if(-not(Test-Path -LiteralPath (Join-Path $bin 'OpenDownloadManager.exe'))){throw 'Build the Rust app first.'}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'browser-stores.json') -Destination $bin -Force
$config=Get-Content -LiteralPath (Join-Path $bin 'browser-stores.json') -Raw|ConvertFrom-Json
if($config.chrome_extension_id -notmatch '^[a-p]{32}$'){throw 'Set the actual Chrome listing ID in browser-stores.json.'}
& $ISCC ('/DChromeId='+$config.chrome_extension_id) (Join-Path $PSScriptRoot 'installer/ODM.iss')
if($LASTEXITCODE -ne 0){throw 'Installer compilation failed.'}
