param([string]$Compiler = 'zig', [string]$Output = "$PSScriptRoot\OpenDownloadManager-Cpp.exe")
$ErrorActionPreference = 'Stop'
& $Compiler rc /i $PSScriptRoot /fo "$PSScriptRoot\app.res" "$PSScriptRoot\app.rc"
if ($LASTEXITCODE -ne 0) { throw 'Windows resource compilation failed' }
$resources=@("$PSScriptRoot\app.res")
if ((Test-Path -LiteralPath "$PSScriptRoot\video-tools.cab") -and (Test-Path -LiteralPath "$PSScriptRoot\video-tools.index")) {
    & $Compiler rc /i $PSScriptRoot /fo "$PSScriptRoot\bundle.res" "$PSScriptRoot\bundle.rc"
    if ($LASTEXITCODE -ne 0) { throw 'Video resource compilation failed' }
    $resources+="$PSScriptRoot\bundle.res"
}
& $Compiler c++ "$PSScriptRoot\main.cpp" @resources -std=c++17 -O2 -target x86_64-windows-gnu -municode '-Wl,--subsystem,windows' -lwinhttp -lshlwapi -lcomctl32 -lcomdlg32 -lshell32 -lole32 -luuid -lgdi32 -lgdiplus -luxtheme -lsetupapi -lbcrypt -lcrypt32 -o $Output
if ($LASTEXITCODE -ne 0) { throw 'Compilation failed' }
