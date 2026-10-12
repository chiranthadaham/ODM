# Windows updates

Click **Check for updates** in ODM. Checks run only when requested and use the public GitHub API for `chiranthadaham/ODM`. Windows PowerShell performs the separate update task so ODM stays responsive and can close before installation.

Publish a stable GitHub Release tagged `vMAJOR.MINOR.PATCH` with `ODM-Setup-MAJOR.MINOR.PATCH.exe` attached. The Cargo version and installer version must match that release. GitHub must supply the asset's SHA-256 `digest`; the updater refuses assets without it. Git tags alone, drafts, prereleases, equal versions and older versions are not installed.

The user confirms a newer version before downloading. A separate progress window remains visible while downloading. The asset size and SHA-256 are checked against GitHub metadata before execution. This verifies integrity against the trusted GitHub repository; it does not replace Windows code signing. Protect the publishing account and enable two-factor authentication.

Only after verification does the updater request ODM's normal Exit action, which pauses workers and saves the queue. It waits up to 60 seconds for shutdown, then runs the installer with `/SILENT /SUPPRESSMSGBOXES /NORESTART`. The installer starts ODM in the tray when finished. Existing data is retained. Network, metadata, verification or shutdown failures stop the update. No forced termination is used. Managed PCs may block PowerShell execution or installer launch.

Run `powershell -NoProfile -File tests/test_updater.ps1` for selection and validation tests. A full upgrade test requires a published newer release; do not publish a fake version just to trigger updates.
