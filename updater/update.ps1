param([Parameter(Mandatory=$true)][string]$CurrentVersion)
$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Windows.Forms
function Notice([string]$text) { [void][System.Windows.Forms.MessageBox]::Show($text,'ODM updates') }
function Select-Update($release,[version]$current) {
    if($release.draft -or $release.prerelease){ throw 'No stable release is available.' }
    $tag=[string]$release.tag_name
    if($tag -notmatch '^v?(\d+\.\d+\.\d+)$'){throw 'The release tag must use vMAJOR.MINOR.PATCH.'}
    $version=$Matches[1]
    if([version]$version -le $current){return $null}
    $assets=@($release.assets | Where-Object { $_.name -ceq "ODM-Setup-$version.exe" -and $_.state -eq 'uploaded' })
    if($assets.Count -ne 1){throw 'This release does not contain the expected Windows installer.'}
    $asset=$assets[0]
    if([string]$asset.digest -notmatch '^sha256:([a-fA-F0-9]{64})$'){throw 'GitHub did not provide an SHA-256 digest. Update cancelled.'}
    $digest=$Matches[1]
    $uri=[uri]$asset.browser_download_url
    if($uri.Scheme -ne 'https' -or $uri.Host -ne 'github.com' -or $uri.UserInfo -or -not $uri.AbsolutePath.StartsWith('/chiranthadaham/ODM/releases/download/')){throw 'Unexpected installer download address.'}
    if($asset.size -le 0 -or $asset.size -gt 1GB){throw 'Unexpected installer size.'}
    return @{Version=$version;Url=$uri.AbsoluteUri;Digest=$digest;Size=[long]$asset.size}
}
$mutex=New-Object System.Threading.Mutex($false,'Local\ODMUpdate')
$locked=$false
$folder=$null
try {
    $locked=$mutex.WaitOne(0)
    if(-not $locked){Notice 'An update check is already running.';exit}
    [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12
    try {
        $release=Invoke-RestMethod -Uri 'https://api.github.com/repos/chiranthadaham/ODM/releases/latest' -Headers @{'User-Agent'='OpenDownloadManager-Updater';Accept='application/vnd.github+json'} -TimeoutSec 30
    } catch {
        if($_.Exception.Response -and [int]$_.Exception.Response.StatusCode -eq 404){Notice 'No published stable release is available yet.';exit}
        throw
    }
    $update=Select-Update $release ([version]$CurrentVersion)
    if(-not $update){Notice "ODM $CurrentVersion is up to date.";exit}
    $answer=[System.Windows.Forms.MessageBox]::Show("Install ODM $($update.Version)? The installer will download first, then active downloads will pause while ODM updates. Your queue, settings, and files will be kept.",'ODM update available','YesNo','Information')
    if($answer -ne 'Yes'){exit}
    $folder=Join-Path $env:LOCALAPPDATA ('OpenDownloadManagerRust\updates\'+[guid]::NewGuid())
    [void](New-Item -ItemType Directory -Path $folder -Force)
    $installer=Join-Path $folder "ODM-Setup-$($update.Version).exe"
    $progress=New-Object System.Windows.Forms.Form
    $progress.Text='Downloading ODM update';$progress.Width=380;$progress.Height=110
    $progress.ControlBox=$false;$progress.StartPosition='CenterScreen'
    $bar=New-Object System.Windows.Forms.ProgressBar
    $bar.Dock='Fill';$bar.Style='Marquee';$progress.Controls.Add($bar);$progress.Show()
    $client=New-Object Net.WebClient
    try {
        $task=$client.DownloadFileTaskAsync([uri]$update.Url,$installer)
        $deadline=[datetime]::UtcNow.AddMinutes(30)
        while(-not $task.IsCompleted){
            [System.Windows.Forms.Application]::DoEvents();Start-Sleep -Milliseconds 100
            if([datetime]::UtcNow -gt $deadline){$client.CancelAsync();throw 'Update download timed out.'}
        }
        $task.GetAwaiter().GetResult()
    } finally {$client.Dispose();$progress.Close();$progress.Dispose()}
    if((Get-Item -LiteralPath $installer).Length -ne $update.Size -or (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash -ine $update.Digest){throw 'Installer verification failed. Nothing was installed.'}
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ODMUpdateWindow {
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls,string title);
 [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint msg,UIntPtr w,IntPtr l);
}
'@
    $window=[ODMUpdateWindow]::FindWindow('OpenDownloadManagerRust',$null)
    if($window -ne [IntPtr]::Zero){
        if(-not [ODMUpdateWindow]::PostMessage($window,0x111,[UIntPtr]119,[IntPtr]::Zero)){throw 'Could not ask ODM to close. Exit it from the tray and retry.'}
    }
    $deadline=[datetime]::UtcNow.AddSeconds(60)
    $running=$null
    while([System.Threading.Mutex]::TryOpenExisting('Local\OpenDownloadManagerRust',[ref]$running)){
        $running.Dispose()
        if([datetime]::UtcNow -gt $deadline){throw 'ODM is still busy saving downloads. Update cancelled; close it from the tray and retry.'}
        Start-Sleep -Milliseconds 200
    }
    $process=Start-Process -FilePath $installer -ArgumentList '/SILENT','/SUPPRESSMSGBOXES','/NORESTART' -PassThru -Wait
    if($process.ExitCode -ne 0){throw "The installer returned error $($process.ExitCode). Run the downloaded installer manually to retry."}
} catch {Notice ("Update could not complete. "+$_.Exception.Message)}
finally {
    if($folder -and (Test-Path -LiteralPath $folder)){
        # Only files created by this invocation are removed.
        Remove-Item -LiteralPath (Join-Path $folder "ODM-Setup-$($update.Version).exe") -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $folder -ErrorAction SilentlyContinue
    }
    if($locked){$mutex.ReleaseMutex()};$mutex.Dispose()
}
