# Test suite

The application is Rust. Python scripts are independent test drivers and local servers; they are not shipped in the binary folder. They generate download fixtures in temporary directories and remove them after testing.

Pass the path to a compiled Windows executable as the first argument, for example:

```powershell
python tests/test_segments.py ../Bin/OpenDownloadManager.exe
```

| Test | Coverage |
| --- | --- |
| `cargo test --locked` | URL validation, ranges, quoting, duration formatting, live rate changes |
| `test_downloads.py` | Full transfers, pause/resume, ignored ranges, filename collisions |
| `test_segments.py` | Parallel integrity, segmented resume, changed validators, C++ metadata migration |
| `test_http_errors.py` | HTTP failures, HTML responses, incomplete bodies |
| `test_optimizations.py` | Reused probe response, 429 fallback, bounded mirror redirects |
| `test_recovery.py` | Automatic recovery from a dropped connection |
| `test_largefile.py` | 64 MiB assembly, stale assembly file, Unicode paths, existing destination |
| `test_transports.py` | HTTP Basic authentication, HTTP proxy, authenticated SOCKS5, FTP |
| `test_ui.py` | Real Windows dialogs, browser start/cancel, live pause/resume, tray, sorting, removal |
| `test_video.py` | Fresh-cache extraction, local HLS, audio/video merge and FFprobe validation |
| `test_online.py` | Optional real HTTPS text and PDF downloads |

The video test requires FFmpeg and FFprobe in the existing verified runtime cache or a folder selected with `ODM_TEST_TOOLS`. Other local-server tests use only Python's standard library. The UI test requires an interactive Windows desktop and should run on its own because the application has a single-instance window.

Enable `test_online.py` with `$env:ODM_ONLINE_TESTS='1'`; it makes internet requests. The other regression tests operate locally. Real YouTube validation for this release is recorded in `../VALIDATION.md`.

## 100-publisher internet audit

The archived [HTML report](reports/100-site-audit.html) and [JSON evidence](reports/100-site-audit.json) record the 110-case audit and follow-up retests. These are actual app transfers, not a certification of every browser download button.

To repeat the curated cohort (Windows curl is required):

```powershell
python tests/audit_sites100.py ../Bin/OpenDownloadManager.exe ../Testing/site-audit
```

This downloads several gigabytes, uses five concurrent cases, and retains app files and per-case evidence in the chosen testing folder. Reference copies are removed after comparison. Downloaded executables are never run. The explicit input list is `sites100.json`; website responses may change. The initial HTML report created by this driver is a fresh first-pass report; the archived release report additionally includes the recorded exception retests.

## Performance and detection

```powershell
python tests/benchmark_downloads.py path/to/previous.exe ../Bin/OpenDownloadManager.exe benchmark.json
python tests/test_timeout_fallback.py ../Bin/OpenDownloadManager.exe
node tests/test_detection.cjs detection.json
```

The benchmark simulates connection/request latency and verifies each full file. The timeout test intentionally takes about 30 seconds. Node.js is a test dependency only. The archived speed and detection evidence is in `reports/`.
