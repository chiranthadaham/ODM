<div align="center">
  <img src="assets/logo.svg" width="96" alt="Open Download Manager logo">
  <h1>Open Download Manager</h1>
  <p>A native Windows download manager, rebuilt in Rust.</p>
  <p>Parallel downloads · Browser integration · Video downloads</p>
  <img src="https://img.shields.io/badge/version-0.2.8-007AFF" alt="Version 0.2.8">
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

1. Run `ODM-Setup-0.2.8.exe` from the binary package. It installs into `%LOCALAPPDATA%\Programs\Open Download Manager`, adds a Start menu shortcut, and starts in the tray at Windows sign-in. Open ODM from the Start menu or tray.
2. Click **+**, paste a download link, and choose a destination.
3. Start the download. Use the toolbar or right-click menu to manage it.

Closing the main window keeps ODM in the system tray. Use the tray menu to reopen or exit it. Video tools are embedded and extracted into a verified per-user cache when first needed; Rust is not required to run the application.

### Connect your browser

Click **Browser plug-in** in ODM and choose **Chrome** or **Firefox**. Until the listings are published, the buttons open official store searches. The installer registers the native host at the fixed installed location; moving a portable EXE does not change the browser connection.

Open this browser chooser using the Browser plug-in button. Each button launches its specific browser. Once a listing URL is configured, it opens that listing for installation; users must confirm the install in the browser. Extensions are not silently installed or forced through enterprise policies.

Upload candidates are in `Bin/Store-Uploads`. See [store submission instructions](browser/STORE-SUBMISSION.md) for publisher details, privacy hosting, store IDs, signing and review. Firefox requires version 140+. The Chrome store edition excludes YouTube downloads. Both ask for privacy consent before capture; automatic capture defaults on after acceptance. The desktop app retains its existing video support.

Disable startup in Windows Startup apps. Uninstall ODM using Windows Installed apps; downloaded files and local queue records are retained. Exit ODM from the tray before upgrading.

Browser downloads open separate confirmation and progress windows while the main application stays in the tray. Website access restrictions, login requirements, DRM, and browser verification can affect capture or downloads; browser fallback remains available.

## Build the installer and extension packages

After building the Rust app, install Inno Setup 6.7.3 and run:

```powershell
.\build-installer.ps1 -ISCC "C:\Path\To\Inno Setup 6\ISCC.exe"
.\package-extensions.ps1
```

Set the actual store URLs and Chrome listing ID in `browser-stores.json` before a store release. The supplied Chrome ID matches the unpacked test extension; the store-assigned ID must be verified before publishing an installer for that listing. Setup is currently unsigned. Keep all third-party notices.

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

The UI follow-up passed 28 relevant regression checks. The later speed/failure update passed 37 native integration checks and both browser detection suites; all four failed original URLs passed their follow-up downloads. A [100-publisher download audit](tests/reports/100-site-audit.html) records 110 format cases and the exception retests, including original failures and their follow-up results.

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

A fresh install starts with an empty queue. Reinstalling preserves this edition's existing queue and settings.

## Contributing

Bug reports and pull requests are welcome. For download issues, include the app version, browser version, error message, and a publicly accessible example URL when possible. Remove cookies, passwords, and private links from reports.

## License and acknowledgements

The Rust application is available under the [MIT License](LICENSE). Bundled helpers and dependencies retain their own licenses and notices in [licenses/](licenses/).

ODM uses Windows APIs and Rust libraries for the desktop application, with **yt-dlp**, **Deno**, and **FFmpeg** for supported video workflows. Browser extensions remain JavaScript components.

## Updates

Click **Check for updates** in the app to check this project's GitHub Releases. A newer stable Windows installer is downloaded and checked against GitHub's SHA-256 digest before installation. ODM pauses transfers and retains the queue and settings. See [updater instructions](updater/README.md) for publishing requirements. Update checks contact GitHub only when requested; GitHub receives standard connection information such as your IP address.


## Appearance

ODM follows the Windows app theme by default. Click **Theme: System** to cycle through **Light**, **Dark**, and **System**. The choice is saved locally and applies to the main window and app download dialogs.


## Integrity, recovery, and file folders

- **SHA-256:** paste an optional publisher checksum in the Save and Start dialog. A mismatch prevents completion and retains partial data. Right-click a completed download and choose **Verify SHA-256…** to calculate its checksum or compare it with a reference. A calculated hash without a trusted reference is not proof that a file is authentic.
- **Automatic hashing:** Settings → **Calculate SHA-256 for every download**. This is optional because reading large files adds disk work. A supplied expected checksum is always verified regardless of this setting.
- **Recovery:** queue writes are flushed and replaced atomically, with a previous valid backup. Damaged or missing queue files can recover from a valid pending write or backup. Recovered transfers remain paused. Partial file bytes and metadata are checkpointed. Resume rechecks server validators; changed files or servers without a safe resume validator restart rather than combine unrelated bytes. If every queue copy is unreadable, ODM preserves them and reports the problem instead of overwriting them with an empty queue.
- **Folders:** Settings → **Use file-type subfolders for new downloads**. Save prompts suggest Videos, Audio, Documents, Archives, Images, Applications, or Other beneath the selected base folder. You can override the suggested path. Existing files are not moved; organization is off by default.

Run `tests/test_reliability.py` on Windows with the compiled executable path after closing ODM. Tests use an isolated queue and local HTTP fixtures. They cover checksum success and rejection, server changes, stale segments, queue recovery, byte-range resume, folder suggestions, and persisted settings.

## 0.2.8 startup improvements

Browser-identified downloads avoid a duplicate access preflight. Firefox no longer waits for body bytes, a known file size, or resume support before handoff. Explicit HTTP(S) download links are captured regardless of extension. Single-connection downloads reuse the first normal GET response. See [startup validation](tests/reports/startup-0.2.8.md) for coverage and limits. Server latency, browser-generated files and protected media can still affect downloads.
