# Browser store submission — ODM 2.7.0

These are upload candidates, not published or approved extensions. The Windows app is version 0.2.0. Extension version numbers are independent.

## Publisher steps required

1. Create your Chrome Web Store and AMO publisher accounts and complete their identity/contact requirements. Supply a public support contact and project/repository URL. No fictitious publisher details are included.
2. Publish the privacy notice from `extension/consent.html` at a permanent public HTTPS URL, with working publisher/support information. The upload folder contains a standalone notice. Use that URL in both store dashboards. Review the notice against the final release and your own data practices.
3. Upload the Chrome ZIP as a listing draft, obtain its assigned 32-character extension ID, and replace `chrome_extension_id` in `browser-stores.json`. The upload ZIP omits the unpacked development key. For testing the store identity unpacked, replace the source manifest's public `key` with the public key supplied by the Chrome listing. Do not include private signing keys. Rebuild the installer and verify native messaging against the actual listing ID. The current ID supports the included unpacked development build only.
4. Add the actual published Chrome and Firefox listing URLs to `browser-stores.json` and rebuild the installer. Until then, Browser plug-in buttons open clearly disclosed official store searches, not a nonexistent ODM listing.
5. Upload `ODM-Chrome-2.7.0.zip` and `ODM-Firefox-2.7.0.zip` to their respective dashboards, with accurate screenshots, listing descriptions, privacy disclosures, support details, and review instructions. AMO must sign the Firefox release before normal Firefox can install it permanently. Do not rename the unsigned ZIP to XPI and claim it is signed.
6. Submit and complete store review. Neither lint success nor this document guarantees approval or certifies legal compliance. Windows setup and the app are currently unsigned; sign with your own publisher certificate for a public release.

## Suggested listing

**Name:** Open Download Manager

**Summary:** Send file downloads and selected media links to the installed Open Download Manager app.

**Description:** Capture file downloads in the Open Download Manager desktop app. Choose where to save each download, pause and resume supported transfers, and select detected media using an on-page download button. Requires the separately installed Windows ODM application. Automatic capture is enabled only after you accept the privacy notice and can be switched off. If the app cannot accept a download, the browser retains it. Some protected, authenticated, expiring, or browser-only downloads cannot be handed off. Download only content you are authorized to save. The Chrome edition does not support YouTube downloads.

**Single purpose:** Hand off user-selected file and media downloads to the local ODM desktop download manager.

## Permissions and data disclosures

Host access on HTTP/HTTPS is necessary to detect downloadable file responses and playable media on arbitrary sites. `webRequest` reads response metadata and source origins; Firefox `webRequestBlocking` handles a validated file response before the browser saves it. `downloads` pauses/cancels a browser download only after the native app accepts it and the user chooses Start. `nativeMessaging` communicates with the installed local app. `contextMenus` provides an explicit link/page command. `storage` stores capture and consent preferences. No unnecessary `activeTab`, cookie, history, telemetry, or management permission is requested.

Disclose browsing activity and website content: selected URLs (including possible query tokens), source origins, filenames, and file response metadata. Local request/detection caches are bounded and cleared on consent revocation. The app retains download queue records locally until the user clears them. No developer server, sale, advertising, or analytics collection. Technical browser user-agent data is no longer forwarded by these extensions. Private-window transmission is excluded. Downloads themselves contact their source websites.

Firefox requires version 140 or newer and declares `browsingActivity` and `websiteContent` in its built-in data consent manifest, plus a local consent screen. Chrome uses the same explicit local opt-in screen. Both package all executable code locally. The Chrome package excludes YouTube/googlevideo capture to comply with Chrome's prohibition on facilitating YouTube downloads.

## Reviewer instructions

Install `ODM-Setup-0.2.0.exe` on Windows x64. It installs per user into `%LOCALAPPDATA%\Programs\Open Download Manager`, registers the browser native messaging host, creates a Start menu shortcut and a startup tray entry. No administrator password is needed. Install the extension, accept or decline its privacy screen, then use Check app connection. Test a direct HTTP/HTTPS file link with capture on, capture off, and the app unavailable. In the native prompt choose a location and Start; confirm the downloaded file contents. Context-menu link handoff and detected unprotected media are additional entry points. No account or paid subscription is required by ODM.

Uninstall ODM from Windows Installed apps. Its startup and native messaging registrations are removed only if they still point to this installation. User downloads and saved queue data are retained. Disable startup in Windows Startup apps if desired. Uninstall the extension separately.

Setup opens the Browser plug-in chooser after installation while the main app stays in the tray. Chrome and Firefox buttons launch the selected browser, not the system's default browser. Public listings are required before direct installation links can work. Regular Chrome/Firefox installations require browser confirmation; the installer does not force-install extensions, edit browser profiles, or configure enterprise management policies.

## Licensing

ODM source and extension code are MIT licensed; packaged extensions include LICENSE. Installer retains all dependency notices in `licenses`. Do not claim affiliation with IDM, Google, Mozilla, Chrome, Firefox, or download source websites. Do not reuse proprietary IDM code or assets. This packaging work does not establish the provenance of third-party content a user chooses to download.

## Policy references (checked 12 October 2026)

- https://developer.chrome.com/docs/webstore/program-policies/policies
- https://developer.chrome.com/docs/webstore/troubleshooting
- https://developer.chrome.com/docs/webstore/user_data
- https://extensionworkshop.com/documentation/publish/add-on-policies/
- https://extensionworkshop.com/documentation/develop/firefox-builtin-data-consent/
