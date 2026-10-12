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

## UI and download follow-up — 2026-10-12

Removed Organize, normalized table and footer padding, added responsive columns, and applied consistent row selection and filename truncation. Visually reviewed the rendered native UI.

The final build passed 28 relevant regression checks: 9 optimization/fallback tests, the end-to-end UI test, 6 download tests, 7 segmentation tests, 4 HTTP-error tests and 1 recovery test. These include bounded normal-GET fallback when a range probe is rejected, numeric Retry-After handling, and persistent-rate-limit bounds. The original 41-check baseline above was not rerun in full for this UI follow-up.

The [100-site report](tests/reports/100-site-audit.html) contains 110 requests across 100 curated publishers, plus exception retests. Shared hosting and differences between the initial candidate and final-build retests are documented. Access restrictions, variable installer responses, reference failures and the intermittent CrystalIDEA timeout remain visible and are not clean passes.

## Failure and performance follow-up — 2026-10-12

All four failed original URLs (OpenVPN, CrystalIDEA, AnyDesk and ESET) passed full-file independent SHA256 comparisons on the optimized transfer engine. The report preserves earlier restrictions and timeouts. Its latest per-case results contain 106 exact matches, three signed but different-reference installers, and one app-complete case with a refused reference. Other previously passing cases were not rerun in full after optimization.

The optimized engine passed 37 native integration checks (downloads 6, segments 7, HTTP errors 4, optimizations 9, recovery 1, transports 7, large-file assembly 1, UI 1, fresh timeout fallback 1). Both extension detection suites passed, covering cached settings, capture changes, update preference retention, private browsing and initialization races. The final immediate UI wake change was additionally checked with the end-to-end UI suite.

The repeatable 32 MiB HTTP/1.1 simulated-latency benchmark reduced requests from 129 to 17, with full hash integrity. Median times and all six runs are in [speed-benchmark.json](tests/reports/speed-benchmark.json). These are local controlled results, not promises about internet bandwidth.

Connection pooling is separated by origin, user agent and network credentials. Windows HTTP/2 is requested with normal protocol fallback. Existing segment metadata remains resumable. Alternate transport is attempted only after a fresh request timeout when no partial/segment files exist; saved partial downloads are preserved. Browser confirmation and fallback remain intact.

Installer and store-extension checks are recorded in [installer validation](tests/reports/installer-validation.md). Source for repeatable setup and live browser integration checks is in tests/installer. Store review, publisher signing, final listing IDs and public policy/support links remain pending.
