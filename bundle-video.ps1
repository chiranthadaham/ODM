param([string]$RuntimeFolder = "$PSScriptRoot\video-runtime", [string]$SourceFolder = "$PSScriptRoot")
$ErrorActionPreference='Stop'
$runtimeRoot=(Resolve-Path -LiteralPath $RuntimeFolder).Path
$sourceRoot=(Resolve-Path -LiteralPath $SourceFolder).Path
$bundleRoot=Join-Path $runtimeRoot 'bundle'
New-Item -ItemType Directory -Path $bundleRoot -Force | Out-Null
$files=@(Get-ChildItem -LiteralPath (Join-Path $runtimeRoot 'tools') -File | Sort-Object Name)
$lines=@('.OPTION EXPLICIT','.Set CabinetNameTemplate=video-tools.cab',('.Set DiskDirectoryTemplate="'+$bundleRoot+'"'),'.Set MaxDiskSize=0','.Set MaxCabinetSize=0','.Set FolderSizeThreshold=2000000000','.Set Cabinet=on','.Set Compress=on','.Set CompressionType=LZX','.Set CompressionMemory=21')
foreach ($file in $files) { $lines+=('"'+$file.FullName+'" "'+$file.Name+'"') }
$ddf=Join-Path $bundleRoot 'video-tools.ddf'
[IO.File]::WriteAllLines($ddf,$lines,[Text.Encoding]::ASCII)
& "$env:WINDIR\System32\makecab.exe" /F $ddf | Out-File -LiteralPath (Join-Path $bundleRoot 'build.log')
if ($LASTEXITCODE -ne 0) { throw 'Video runtime compression failed' }
Copy-Item -LiteralPath (Join-Path $bundleRoot 'video-tools.cab') -Destination (Join-Path $sourceRoot 'video-tools.cab')
$hashLines=@($files | ForEach-Object { (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLower()+' '+$_.Name })
$hashAlgorithm=[Security.Cryptography.SHA256]::Create()
$id=[Convert]::ToHexString($hashAlgorithm.ComputeHash([Text.Encoding]::UTF8.GetBytes(($hashLines -join "`n")))).ToLower().Substring(0,20)
[IO.File]::WriteAllLines((Join-Path $sourceRoot 'video-tools.index'),(@($id)+$hashLines),[Text.Encoding]::ASCII)
Get-Item -LiteralPath (Join-Path $sourceRoot 'video-tools.cab') | Select-Object Name,Length
