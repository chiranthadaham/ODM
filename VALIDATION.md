# Windows Rust release validation

Validated on Windows x64 on 2026-10-11. The GNU release build completed successfully using Rust 1.99.0 and Zig 0.15.2. The shipped executable is the native Rust application.

## Automated results

All 41 checks passed. Integration scripts ran against the packaged executable in `Bin`.

| Suite | Passed | Coverage |
| --- | ---: | --- |
| Rust unit tests | 6 | ETA formatting, URL validation, ranges, process quoting, live pacing, filename reservations |
| Downloads | 6 | Integrity, pause/resume, ignored ranges, missing validators, collision protection, protocol validation |
| Segments | 7 | Parallel downloads, partial resume, changed remote files, fallback, connection limits, legacy metadata |
| HTTP errors | 4 | Error handling and response validation |
| Optimizations | 5 | Probe reuse, rate-limit fallback, relative mirrors, redirect bounds and safety |
| Recovery | 1 | Interrupted connection resumed with byte-range integrity |
| Transports | 7 | Basic/Digest, HTTP proxy, proxy authentication, authenticated SOCKS5, FTP, credential origin isolation |
| Native UI | 1 | Download dialogs, progress/completion, settings, browser messaging, tray, sorting, rapid pause/resume, selected removal, Clear all |
| HLS video | 1 | Embedded tools extraction, download, remux, audio/video verification |
| Large file | 1 | 64 MiB assembly, stale assembly recovery, Unicode paths, collision safety, SHA256 integrity |
| Online HTTPS | 2 | Rust license text and Mozilla PDF downloaded and validated |

The UI suite contains multiple assertions within its single end-to-end test. Main-window and New-download screenshots were also visually reviewed.

## Real YouTube download

The release candidate downloaded `https://www.youtube.com/watch?v=3PbdkXLnK74` successfully: 187,902,591 bytes, 974.734 seconds, with both audio and video verified using FFprobe. Later release changes did not alter the video engine.

## Scope

These results do not certify every website or network environment. DRM, account access, expired links, and website verification remain subject to the site's restrictions and the browser fallback. NTLM and FTPS are supported through the underlying Windows/curl transports but were not individually exercised here. The alternative MSVC build script was not tested on this machine.

Downloaded fixtures and build caches are excluded from the release folders. Python is a testing dependency only.
