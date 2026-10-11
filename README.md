<div align="center">
  <img src="app-icon.svg" alt="Open Download Manager logo" width="112" height="112">
  <h1>Open Download Manager</h1>
  <p><strong>Your downloads. One simple place.</strong></p>
  <p>A native Windows download manager with segmented transfers,<br>video downloads, and Firefox &amp; Chromium integration.</p>
  <p>
    <img src="https://img.shields.io/badge/platform-Windows_x64-2563eb?style=flat-square" alt="Windows x64">
    <img src="https://img.shields.io/badge/language-C%2B%2B17-475569?style=flat-square" alt="C++17">
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-15803d?style=flat-square" alt="MIT license"></a>
  </p>
  <p>
    <a href="#features">Features</a> &nbsp; · &nbsp;
    <a href="#getting-started">Getting started</a> &nbsp; · &nbsp;
    <a href="#build-on-windows">Build</a> &nbsp; · &nbsp;
    <a href="#browser-extensions">Extensions</a> &nbsp; · &nbsp;
    <a href="#tests">Tests</a>
  </p>
</div>

---

## Features

| | What you get |
| :--- | :--- |
| **Segmented downloads** | Validated parallel HTTP/HTTPS transfers with automatic fallback when a server cannot safely support ranges. |
| **Pause & resume** | Persistent download queues and validated partial-file recovery. |
| **Browser integration** | Firefox and Chromium extensions capture downloads and offer a selectable video menu. |
| **Video support** | Supported video and HLS downloads through optional yt-dlp, Deno and FFmpeg tools. |
| **Live controls** | Change speed limits and connection settings while downloads are running. |
| **A focused interface** | Separate start, progress and completion windows, sortable columns, and a notification-area icon. |
| **Organized downloads** | File types, status, progress, speed, and local date and time added in one queue. |
| **Connection options** | Proxy and authentication settings; FTP and SOCKS transfers through Windows curl. |

## Getting started

1. **Launch** `OpenDownloadManager.exe` after building the app.
2. **Add a link** with the **+** button, or capture a download using a browser extension.
3. **Choose a location** and start the download in its separate window.
4. **Keep browsing** while ODM runs in the notification area, then open the completed file from the completion prompt.

The trash button removes a selected inactive entry. **Clear all** removes inactive entries from the queue. Downloaded files stay on disk.

> **Compatibility:** Site authentication, browser verification, expiring links and DRM can prevent downloads. More connections do not guarantee higher speed. Existing queue entries without a recorded added time display a dash.

## Build on Windows

Use PowerShell, Zig **0.15.2** (the tested compiler) and Windows 10/11 x64. Put `zig.exe` on PATH or pass its full path:

```powershell
.\build.ps1 -Compiler zig -Output "$PWD\OpenDownloadManager.exe"
```

This builds the native app without bundled video tools. No Visual Studio installation is required. The application uses Windows system libraries and the included `json.hpp` dependency.

### Build with video tools

Python 3 and network access are needed to fetch upstream helper releases:

```powershell
python .\provision-video.py
pwsh -File .\bundle-video.ps1
.\build.ps1 -Compiler zig -Output "$PWD\OpenDownloadManager.exe"
```

Run the cabinet bundling script with PowerShell 7 (`pwsh`); it uses Windows `makecab.exe`. The provisioning script downloads current releases, records versions/hashes under `video-runtime`, and collects notices. It does not pin the exact helper versions used by an older release, so rebuilding can change video behavior and does not guarantee a byte-identical EXE.

Generated tools, cabinets and executables are excluded from Git. Keep the generated licenses with any portable binary distribution. See `THIRD_PARTY.md` for dependency and source information.

## Browser extensions

After building `OpenDownloadManager.exe` at the repository root:

1. Run `firefox-integration/Install.cmd` and/or `chromium-integration/Install.cmd` to register this user's native host. These scripts generate machine-specific `native-host.json` files.
2. Firefox: open `about:debugging`, choose **This Firefox → Load Temporary Add-on**, and select `firefox-extension/manifest.json`. Temporary add-ons must be reloaded after browser restarts; permanent installation requires Mozilla signing.
3. Chromium: open `chrome://extensions` (or your browser's equivalent), enable Developer mode, choose **Load unpacked**, and select `chromium-extension`.
4. Refresh website tabs after loading or updating the extension.

The unpacked Chromium extension ID is `jancefngmifbnaldahilleoinncjppjh`. Re-run the registration scripts if the application folder moves. The extensions validate access before transferring a download to the app; browser fallback remains available when app access fails.

## Tests

The following controlled transfer tests use local HTTP servers and temporary folders:

```powershell
python .\tests\test_downloads.py .\OpenDownloadManager.exe
python .\tests\test_segments.py .\OpenDownloadManager.exe
python .\tests\test_http_errors.py .\OpenDownloadManager.exe
python .\tests\test_optimizations.py .\OpenDownloadManager.exe
```

These tests cover native downloads, resume, segmented transfers, HTTP errors and fallback behavior. They do not certify every website or browser button.

## Project structure

| Files | Purpose |
| --- | --- |
| `main.cpp` | Startup, queue persistence and transfer coordination |
| `segmented.h`, `network.h`, `network_options.h` | Transfers, proxies and authentication |
| `ui.h`, `workflow.h` | Queue, settings, start/progress/completion windows |
| `video.h`, `runtime.h` | Video helpers and verified runtime extraction |
| `browser_bridge.h`, `download_redirect.h` | Browser integration and redirect handling |
| `firefox-extension`, `chromium-extension` | Browser add-ons |
| `*-integration` | Native-host setup scripts |
| `tests/` | Separate download test scripts and test helpers |
| `app.*`, `bundle.rc` | Windows application resources |

## License

Original ODM source is licensed under the [MIT License](LICENSE). Third-party components retain their own licenses; see [third-party notices](THIRD_PARTY.md) and the `third-party-licenses` folder.

---

<p align="center"><sub>Built with C++17 · Win32 · WinHTTP</sub></p>