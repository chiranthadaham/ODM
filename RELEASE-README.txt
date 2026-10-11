OPEN DOWNLOAD MANAGER — RUST EDITION 0.2.0
Windows 10 / 11, 64-bit

Run OpenDownloadManager.exe. The application is native Rust, with the same
Windows interface and download workflow as the previous ODM application.

Click + to add a URL. Choose its save location and click Start download.
A separate progress window shows the transfer. When it completes, use Open
or Open folder. Closing the main window leaves ODM in the system tray;
right-click its tray icon and choose Exit to quit.

Use Settings for simultaneous downloads, connections per download, speed
limits, proxy settings, and server credentials. Click a table header to sort.
The trash icon removes selected queue entries; Clear all removes inactive
entries. These actions keep downloaded files on disk.

BROWSER INTEGRATION
Run Firefox\Install.cmd or Chromium\Install.cmd and follow its instructions.
The scripts register this Rust executable for the current Windows user.
This switches the browser's ODM connection to this edition. To switch back,
run the installer in the original ODM folder.

Firefox: about:debugging -> This Firefox -> Load Temporary Add-on, then
choose Firefox\extension\manifest.json. Temporary add-ons must be loaded
again after restarting Firefox unless separately signed and installed.

Chromium / Chrome / Edge / Brave / Vivaldi: open the extensions page,
enable Developer mode, choose Load unpacked, then Chromium\extension.

The bundled video tools extract into a verified per-user cache on first use.
FTP and SOCKS transfers use the curl supplied with modern Windows.

Source code is in the sibling Source folder. Tests and build instructions
are included there. Third-party licenses are in licenses; keep that folder
with the application when redistributing it.

The previous ODM installation is preserved. Rust stores its queue and
encrypted credentials under %LOCALAPPDATA%\OpenDownloadManagerRust, and
imports the previous saved queue on first use when no Rust queue exists.
