# 0.2.9 Firefox handoff validation

13 October 2026.

Confirmed original failure: Firefox helper integrity 8192 (Medium), ODM integrity 12288 (High), WM_COPYDATA blocked with Windows error 5 in 0 ms. Both tokens reported elevated; elevation-only checks were misleading.

Fix: local per-user, per-session named-pipe connection. Explicit current-user DACL, Medium integrity label, noninherited handles, remote-client rejection, and executable-path verification at both endpoints. Browser helpers no longer send HWND messages to the GUI. Only the worker inside ODM communicates with the UI. Browser requests remain bounded to 64 KiB, restricted to HTTP(S), and confirmed by the user. Preflight validation, cancellation and transfer validation remain. No UAC, Firefox sandbox or Windows message-filter settings were changed.

Passed real cross-integrity cases using a High-integrity app and a Medium-integrity helper:
- connection/ping;
- access-check prompt cancellation and reply;
- save/start confirmation, reply and byte-exact local HTTP download;
- invalid browser URL rejection;
- foreign executable-path rejection;
- Medium helper cold-starting the app, which remains running after the helper exits.

Also passed the native UI regression, 10 HTTP optimization tests, both browser detection suites, explicit download-link tests and updater checks. All 9 Rust unit tests passed. Temporary test queues are isolated. The live Firefox session still requires a final user retry after installation; the automated cross-integrity test reproduces its measured integrity combination.

Run `tests/test_ipc_integrity.py ../Bin/OpenDownloadManager.exe` from an elevated Windows terminal with ODM closed. The elevation is for the regression fixture, not an application requirement. Medium/High testing is Windows-specific.
