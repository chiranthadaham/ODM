$ErrorActionPreference='Stop'
$output=Join-Path (Split-Path -Parent $PSScriptRoot) 'Bin/Store-Uploads'
New-Item -ItemType Directory -Force $output|Out-Null
foreach($product in @('Firefox','Chromium')){
 $extension=Join-Path $PSScriptRoot "browser/$product/extension"
 $label=if($product -eq 'Chromium'){'Chrome'}else{'Firefox'}
 Compress-Archive -Path (Join-Path $extension '*') -DestinationPath (Join-Path $output "ODM-$label-0.2.8.zip") -Force
 if($product -eq 'Chromium'){
  # The unpacked test key is not the public key assigned by a new store listing.
  Add-Type -AssemblyName System.IO.Compression
  $archive=[IO.Compression.ZipFile]::Open((Join-Path $output "ODM-$label-0.2.8.zip"),[IO.Compression.ZipArchiveMode]::Update)
  try{
   $archive.GetEntry('manifest.json').Delete()
   $manifest=Get-Content -LiteralPath (Join-Path $extension 'manifest.json') -Raw|ConvertFrom-Json
   $manifest.PSObject.Properties.Remove('key')
   $entry=$archive.CreateEntry('manifest.json')
   $writer=[IO.StreamWriter]::new($entry.Open(),[Text.UTF8Encoding]::new($false))
   try{$writer.Write(($manifest|ConvertTo-Json -Depth 10))}finally{$writer.Dispose()}
  }finally{$archive.Dispose()}
 }
}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'browser/STORE-SUBMISSION.md') -Destination $output -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'browser/PRIVACY.html') -Destination (Join-Path $output 'privacy-policy.html') -Force

