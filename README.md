<div align="center">
  <img src="assets/logo.svg" width="96" alt="Open Download Manager logo">
  <h1>Open Download Manager</h1>
  <p>A native Windows download manager, rebuilt in Rust.</p>
  <p>Parallel downloads · Browser integration · Video downloads</p>
  <img src="https://img.shields.io/badge/version-3.0.0-007AFF" alt="Version 3.0.0">
  <img src="https://img.shields.io/badge/language-Rust-orange" alt="Rust">
  <img src="https://img.shields.io/badge/platform-Windows%20x64-0078D4" alt="Windows x64">
  <img src="https://img.shields.io/badge/license-MIT-green" alt="MIT license">
</div>

<br>

**Open Download Manager (ODM)** brings segmented downloads, a persistent queue, and browser capture together in a clean desktop interface. This edition rewrites the Windows application in Rust while retaining the familiar controls and download workflows.

The executable is compiled from Rust. The Windows UI, transfer engine, queue storage, settings, system tray, and browser messaging bridge are implemented in Rust. Browser extensions use JavaScript; video extraction and merging use bundled yt-dlp, Deno, and FFmpeg tools.

## Features

| | What you can do |
| --- | --- |
| **Parallel downloads** | Download HTTP/HTTPS files in segments, adjust connection counts, and use sequential fallback when ranges are unavailable. |
| **Pause and resume** | Continue validated partial downloads and restore the queue after restarting the app. |
| **Browser integration** | Capture supported downloads in Firefox and Chromium browsers, with a separate save/start prompt. |
| **Video downloads** | Download supported YouTube, HLS, and DASH media using the bundled video tools. |
| **Live controls** | Change connection counts, simultaneous download limits, and per-download speed limits. |
| **Organized queue** | Sort by file, status, progress, size, speed, connections, type, or date added. Import/export URLs and renew download links. |
| **Native interface** | Rounded controls, icon toolbar, right-click actions, independent progress/completion windows, and system tray behavior. |
| **Network options** | HTTP/HTTPS, FTP/FTPS, HTTP and SOCKS proxies, and server/proxy authentication through Windows networking or Windows curl. |
| **File protection** | Validate resumed ranges and assemble segmented downloads without overwriting an existing destination. |

## Getting started

**Requirements:** Windows 10 or Windows 11, 64-bit.

1. Open `OpenDownloadManager.exe` from the binary package.
2. Click **+**, paste a download link, and choose a destination.
3. Start the download. Use the toolbar or right-click menu to manage it.

Closing the main window keeps ODM in the system tray. Use the tray menu to reopen or exit it. Video tools are embedded and extracted into a verified per-user cache when first needed; Rust is not required to run the application.

### Connect your browser

The binary package includes `Firefox` and `Chromium` folders. Run `Install.cmd` in the appropriate folder, then follow its instructions to load the extension and register the app.

Browser downloads open separate confirmation and progress windows while the main application stays in the tray. Website access restrictions, login requirements, DRM, and browser verification can affect capture or downloads; browser fallback remains available.

## Build from source

Run the following commands from the repository root, which should contain `Cargo.toml` and this README.

### Retrieve bundled assets

The video-tools archive is tracked with Git LFS. After cloning:

```powershell
git lfs install
git lfs pull
```

### Option 1: Windows SDK / MSVC

Install Rust for Windows and Visual Studio Build Tools with **Desktop development with C++** and the Windows SDK. These provide the Windows linker, libraries, and resource compiler; the application itself is written in Rust.

From Developer PowerShell:

```powershell
.\build.ps1
```

### Option 2: GNU toolchain / Zig

Install Rust's GNU toolchain and Zig 0.15.2:

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu
.\build-gnu.ps1 -Zig C:\tools\zig.exe
```

Both scripts build with `Cargo.lock` and create a **Bin** folder beside the source directory, containing the executable, browser integrations, and licenses. The GNU/Zig build was used for the validated Windows release.

## Project structure

```text
Source/
├── src/                 Rust application and native Windows UI
├── browser/             Firefox and Chromium extensions
├── assets/              Logo, icon, manifest, and video-tools archive
├── tests/               Integration tests
├── licenses/            Third-party notices
├── Cargo.toml
├── Cargo.lock
├── build.ps1
├── build-gnu.ps1
└── VALIDATION.md

Bin/                     Created beside Source when building
├── OpenDownloadManager.exe
├── Firefox/
├── Chromium/
└── licenses/
```

Upload the **contents of Source** to the repository root so the logo, links, and build commands work as shown. Publish the compiled binary package as a GitHub Release asset. Keep the large video-tools archive under Git LFS; do not upload it as a regular Git object.

## Testing

The initial Rust release passed **41 automated checks**, covering downloads, segmented resume, error recovery, authentication, proxies, UI workflows, video processing, and large-file assembly. A real YouTube download was also verified with audio and video tracks.

See [VALIDATION.md](VALIDATION.md) for results and testing limits, and [tests/README.md](tests/README.md) for integration-test instructions.

Core tests:

```powershell
cargo test --locked
```

Example integration tests, after building:

```powershell
python tests/test_downloads.py ../Bin/OpenDownloadManager.exe
python tests/test_segments.py ../Bin/OpenDownloadManager.exe
python tests/test_ui.py ../Bin/OpenDownloadManager.exe
python tests/test_transports.py ../Bin/OpenDownloadManager.exe
python tests/test_video.py ../Bin/OpenDownloadManager.exe
```

Python is used only for testing. The local HLS test needs FFmpeg/FFprobe from the verified runtime cache, or a tools folder supplied through `ODM_TEST_TOOLS`. Test downloads and build caches are kept out of the binary package.

## Saved data and migration

Queue data and Windows-protected credentials are stored in:

```text
%LOCALAPPDATA%\OpenDownloadManagerRust
```

On first run, ODM can migrate the previous C++ edition's saved queue and connection settings without modifying its saved data. Previously active downloads remain paused until resumed. Avoid running both editions against the same partial download at the same time.

## Contributing

Bug reports and pull requests are welcome. For download issues, include the app version, browser version, error message, and a publicly accessible example URL when possible. Remove cookies, passwords, and private links from reports.

## License and acknowledgements

The Rust application is available under the [MIT License](LICENSE). Bundled helpers and dependencies retain their own licenses and notices in [licenses/](licenses/).

ODM uses Windows APIs and Rust libraries for the desktop application, with **yt-dlp**, **Deno**, and **FFmpeg** for supported video workflows. Browser extensions remain JavaScript components.