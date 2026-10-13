$ErrorActionPreference='Stop'
$output=Join-Path (Split-Path -Parent $PSScriptRoot) 'Bin/Store-Uploads'
New-Item -ItemType Directory -Force $output|Out-Null
foreach($product in @('Firefox','Chromium')){
 $extension=Join-Path $PSScriptRoot "browser/$product/extension"
 $label=if($product -eq 'Chromium'){'Chrome'}else{'Firefox'}
 $version=(Get-Content -LiteralPath (Join-Path $extension 'manifest.json') -Raw|ConvertFrom-Json).version
 if($product -eq 'Chromium'){
  $local=Join-Path (Split-Path -Parent $PSScriptRoot) 'Bin/Chromium'
  New-Item -ItemType Directory -Force $local|Out-Null
  # Preserve the development key for the local edition's native-host identity.
  Compress-Archive -Path (Join-Path $extension '*') -DestinationPath (Join-Path $local "ODM-Chrome-$version-local.zip") -Force
 }
 Compress-Archive -Path (Join-Path $extension '*') -DestinationPath (Join-Path $output "ODM-$label-$version.zip") -Force
 if($product -eq 'Chromium'){
  # The unpacked test key is not the public key assigned by a new store listing.
  Add-Type -AssemblyName System.IO.Compression
  $archive=[IO.Compression.ZipFile]::Open((Join-Path $output "ODM-$label-$version.zip"),[IO.Compression.ZipArchiveMode]::Update)
  try{
   $archive.GetEntry('manifest.json').Delete()
   $manifest=Get-Content -LiteralPath (Join-Path $extension 'manifest.json') -Raw|ConvertFrom-Json
   $manifest.PSObject.Properties.Remove('key')
   $excluded=@('*://youtube.com/*','*://*.youtube.com/*','*://youtu.be/*','*://*.youtube-nocookie.com/*','*://*.googlevideo.com/*')
   foreach($script in $manifest.content_scripts){$script|Add-Member -NotePropertyName exclude_matches -NotePropertyValue $excluded -Force}
   $entry=$archive.CreateEntry('manifest.json')
   $writer=[IO.StreamWriter]::new($entry.Open(),[Text.UTF8Encoding]::new($false))
   try{$writer.Write(($manifest|ConvertTo-Json -Depth 10))}finally{$writer.Dispose()}
   $archive.GetEntry('edition.js').Delete()
   $entry=$archive.CreateEntry('edition.js')
   $writer=[IO.StreamWriter]::new($entry.Open(),[Text.UTF8Encoding]::new($false))
   try{$writer.Write('"use strict";'+"`n"+'// Chrome Web Store edition: YouTube handoff is disabled.'+"`n"+'var ODM_YOUTUBE_ENABLED = false;'+"`n")}finally{$writer.Dispose()}
  }finally{$archive.Dispose()}
 }
}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'browser/STORE-SUBMISSION.md') -Destination $output -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'browser/PRIVACY.html') -Destination (Join-Path $output 'privacy-policy.html') -Force

