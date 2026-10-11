$ErrorActionPreference='Stop'
Push-Location $PSScriptRoot
try {
    & cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Rust build failed.' }
    $bin=Join-Path (Split-Path -Parent $PSScriptRoot) 'Bin'
    New-Item -ItemType Directory -Path $bin -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'target/release/OpenDownloadManager.exe') -Destination $bin -Force
    Copy-Item -Path (Join-Path $PSScriptRoot 'browser/*') -Destination $bin -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'licenses') -Destination $bin -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'RELEASE-README.txt') -Destination (Join-Path $bin 'README.txt') -Force
    Write-Host "Built $bin\OpenDownloadManager.exe"
} finally { Pop-Location }
