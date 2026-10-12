param([string]$Zig=$env:ODM_ZIG,[string]$Rustup='rustup')
$ErrorActionPreference='Stop'
if (-not $Zig -or -not (Test-Path -LiteralPath $Zig)) { throw 'Pass -Zig with the full path to Zig 0.15.2 or set ODM_ZIG.' }
$toolchain='stable-x86_64-pc-windows-gnu'
$sysroot=(& $Rustup run $toolchain rustc --print sysroot).Trim()
if ($LASTEXITCODE -ne 0) { throw 'Install the GNU toolchain first: rustup toolchain install stable-x86_64-pc-windows-gnu' }
$lld=Join-Path $sysroot 'lib/rustlib/x86_64-pc-windows-gnu/bin/rust-lld.exe'
$tools=Join-Path $PSScriptRoot 'target/build-tools'
New-Item -ItemType Directory -Force $tools | Out-Null
$wrapper=Join-Path $tools 'dlltool.cmd'
"@echo off`r`n`"$Zig`" dlltool %*`r`n" | Set-Content -LiteralPath $wrapper -Encoding Ascii
$previousFlags=$env:CARGO_ENCODED_RUSTFLAGS
$previousZig=$env:ODM_ZIG
$previousCache=$env:ZIG_GLOBAL_CACHE_DIR
try {
    $env:ODM_ZIG=$Zig
    $env:ZIG_GLOBAL_CACHE_DIR=Join-Path $PSScriptRoot 'target/zig-global-cache'
    $env:CARGO_ENCODED_RUSTFLAGS=@('-C','link-self-contained=yes','-C','linker-flavor=ld.lld','-C',"linker=$lld",'-C',"dlltool=$wrapper") -join [char]31
    Push-Location $PSScriptRoot
    try { & $Rustup run $toolchain cargo build --release --locked; if ($LASTEXITCODE -ne 0) { throw 'Rust build failed.' } } finally { Pop-Location }
    $bin=Join-Path (Split-Path -Parent $PSScriptRoot) 'Bin'
    New-Item -ItemType Directory -Force $bin | Out-Null
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'target/release/OpenDownloadManager.exe') -Destination $bin -Force
    Copy-Item -Path (Join-Path $PSScriptRoot 'browser/*') -Destination $bin -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'licenses') -Destination $bin -Recurse -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'RELEASE-README.txt') -Destination (Join-Path $bin 'README.txt') -Force
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'browser-stores.json') -Destination $bin -Force
    Write-Host "Built $bin\OpenDownloadManager.exe"
} finally { $env:CARGO_ENCODED_RUSTFLAGS=$previousFlags; $env:ODM_ZIG=$previousZig; $env:ZIG_GLOBAL_CACHE_DIR=$previousCache }
