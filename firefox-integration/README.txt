FIREFOX DOWNLOAD INTEGRATION — Open Download Manager 2.6.0

UPDATE / INSTALL
1. Open Firefox about:debugging > This Firefox.
2. Reload Open Download Manager. If missing, Load Temporary Add-on and choose
   Firefox\extension\manifest.json inside the IDM Code folder.
3. Refresh existing website tabs so they receive the new video button.
4. Open the extension popup and ensure Automatically capture file downloads is on.
   Capture is on by default and enabled on installation/update.
5. Check app connection should report version 2.6.0.

DOWNLOAD FILES
Click a website's file download button. The app opens its Save and Start window.
Choose a folder and filename and click Start download. Cancel stops the request.
Automatic capture supports arbitrary file formats through response MIME types,
attachment headers and explicit downloads; it is not limited to known extensions.
Ordinary HTML pages and passive page images/scripts remain in Firefox.
Right-click links to send them manually, or paste URLs into the extension popup.

DOWNLOAD PLAYING VIDEOS
A Download video button appears when a page's video is playing. Click it to open
Save and Start in the app. Direct HTTP videos, YouTube and detected HLS/DASH
sources are supported. Protected/DRM streams, browser-only blobs with no detected
HTTP source, signed-in videos and some custom players can be unavailable. The
extension does not remove DRM or copy website sign-in cookies. A page refresh
is required after installing/reloading the extension.

APP CONTROLS
Settings contains simultaneous files, connections per file and per-file speed.
Clear list removes inactive entries without deleting files; active jobs stay.
Right-click entries for open, pause, retry, cancel, copy link and remove actions.
Closing the app window hides it in the Windows notification tray. Transfers keep
running. Click the tray icon or launch the EXE again to reopen. Tray > Exit quits.
Scheduling has been removed; old scheduled jobs become paused on loading.
A completion prompt offers Open file and Open folder after a download finishes.

CONNECTION AND LIMITATIONS
The native connection is registered locally. Run Install.cmd after moving the
app folder. Uninstall.ps1 removes the connection; remove the add-on separately.
Unsigned temporary extensions last until Firefox restarts. Standard Firefox
requires Mozilla signing for permanent installation. No browser security
preferences were changed.

The app checks access before accepting browser transfers. If the site returns
403, verification HTML or another error, Firefox keeps the transfer. Starting a
job cancels its duplicate browser response. Later server errors or expired links
can still fail. Testfile.org was observed returning a Cloudflare 403; the app does
not bypass its verification. Use the browser's ordinary verification flow.
Private-window transfers, blob/data links and protected browser pages are not
automatically captured. Cookies, passwords and verification tokens are not copied.
No external server receives extension data. URLs pass locally to the app, and
normal downloads connect to their source website.

VIDEO CONTROL
Click Download latest to download the latest detected video.
The separate arrow opens an upward scrollable list including the latest video.
Click X to hide the control until page reload.
Start opens a separate progress window; completion offers Open file/Open folder.
The main app stays in the tray during browser captures.
