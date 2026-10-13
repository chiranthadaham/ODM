# Local Chromium YouTube restoration

13 October 2026. Desktop app 0.2.9; local Chromium extension 0.2.10.

Restored content-script injection on YouTube domains and video-page URLs for blob-backed YouTube players. Local background handoff now accepts YouTube/youtu.be/googlevideo links. Consent, private-window restrictions and explicit native save/start confirmation remain. No protected-media bypass was introduced.

The local ZIP preserves the development key and native-host ID jancefngmifbnaldahilleoinncjppjh. Store packaging creates a separate ZIP with its development key removed, YouTube domains excluded and edition.js set to false. Do not submit the local edition as the Chrome Web Store candidate.

Passed: watch/Shorts/short-link video source detection; native-message handoff; unchanged extension ID; repeated edition-script loading; store edition blocking and exclusions; both browser detection suites; JavaScript syntax checks and explicit download-link tests. These are deterministic extension tests, not a live Chrome/YouTube download. The desktop YouTube transfer implementation was unchanged.

Reload the unpacked extension in chrome://extensions, then refresh the YouTube tab. Use Bin/Chromium/extension or extract Bin/Chromium/ODM-Chrome-0.2.10-local.zip.
