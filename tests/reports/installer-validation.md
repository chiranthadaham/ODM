# Installer and store extension validation — 12 October 2026

- Rust release build succeeded after Browser plug-in UI changes.
- Inno Setup 6.7.3 compiler downloaded from its official release and Authenticode verified as Pyrsys B.V.
- Setup installs at the default per-user path, creates Start menu entry, registers tray startup and both browser native hosts. Installed executable SHA-256 equals the release executable.
- Main window hidden on startup; Browser plug-in popup exposes Chrome and Firefox buttons.
- Uninstall removes its own executable/startup/native-host registrations. Reinstall restores them. User data not deleted.
- Mozilla web-ext 10.7.0: zero errors, notices and warnings.
- Automated Firefox/Chrome background tests: opt-in required, opt-out respected, revocation stops capture, private handoff excluded, Chrome YouTube/googlevideo blocked, 40 metadata captures per browser with one preference read.

Store approval and publication are pending. Public privacy/support URLs, publisher identity, actual Chrome listing ID and final store URLs must be supplied. Firefox package is unsigned until AMO signs it. Setup/app have no publisher signature. See browser/STORE-SUBMISSION.md.

Live Chrome 154 tests captured ZIP, PDF and unknown-format files with identical contents and no native-browser duplicate; consent, revocation, installed native-host ping and YouTube blocking passed. Live Firefox 157 tests accepted the consent screen, connected to the installed native host, and completed manual ZIP and automatic PDF transfers with identical contents. The Firefox test began with ODM closed and verified that the app survived native-message disconnect. Fixed the discovered cold-launch lifetime issue using CREATE_BREAKAWAY_FROM_JOB and null standard streams.

Windows prevents lower-integrity processes messaging an elevated app. Run ODM normally; if an elevated instance blocks handoff, exit it and retry from the browser. This boundary is preserved.

Browser chooser update: setup automatically opens the chooser, each button targets its specific browser through Windows App Paths, and --browser-plugins also opens the chooser in an existing app instance. Final install/uninstall/reinstall and UI/download regression checks passed. Normal sign-in startup remains tray-only. Store listings and browser confirmation remain required; no silent extension installation or enterprise policies are configured.
