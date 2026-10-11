# Open Download Manager

An original Windows x64 download manager built with C++17, Win32 and WinHTTP, with Firefox and Chromium browser extensions. This repository contains the source of the ODM app, not the proprietary Internet Download Manager product.

## Features

- Validated segmented HTTP/HTTPS transfers, pause/resume, redirects and safe filename collision handling.
- Live speed limits and connection settings; FTP and SOCKS through Windows curl.
- Separate start, progress and completion windows; notification-area operation.
- Sortable queue columns, including local date and time added. Existing entries without a recorded date display a dash.
- Browser download capture and a selectable video menu.
- Supported video and HLS downloads through optional yt-dlp, Deno and FFmpeg tools.

Site authentication, browser verification, expiring URLs and DRM can prevent downloads. Increasing connections does not guarantee higher speed.

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

## Source layout

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

Original ODM source is licensed under MIT; see `LICENSE`. Dependencies retain their own licenses. The MIT license does not replace those dependency licenses.