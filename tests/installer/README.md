# Installer/browser integration checks

Run in a disposable Windows user account: setup tests intentionally uninstall/reinstall ODM. Install the Bin setup first and exit ODM from the tray. Node.js, Python, Chrome, Firefox, and geckodriver are needed for browser tests. Install Playwright with npm install in this directory. Set ODM_GECKODRIVER to its executable path when it is not on PATH. ODM_CHROME optionally overrides Chrome's path.

- python test_setup.py --allow-install-test
- python test_browser.py

Browser tests use temporary profiles, a temporary app queue, a local HTTP server, explicit consent, and automated Start confirmations. They compare complete file bytes for ZIP, PDF and unknown-format handoffs. They never download internet content. Firefox's documented MOZ_REMOTE_ALLOW_SYSTEM_ACCESS is enabled only in the disposable test process so WebDriver can click the extension's consent screen. This is not set on the user's normal browser.

The Windows app should not run elevated above the browser's native host. Exit an elevated instance before testing; the Firefox test deliberately checks native-host cold launch.
