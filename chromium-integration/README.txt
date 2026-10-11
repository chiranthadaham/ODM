OPEN DOWNLOAD MANAGER — CHROMIUM EXTENSION 2.6.0

1. Run Install.cmd once to register the app connection for this user.
2. Open chrome://extensions (Chrome), edge://extensions (Edge),
   brave://extensions (Brave), or vivaldi://extensions (Vivaldi).
3. Enable Developer mode, choose Load unpacked, select Chromium\extension.
4. Pin Open Download Manager and click Check app connection.
5. Refresh existing tabs. Automatic capture defaults to on.

The same folder works with Chromium-based browsers supporting Manifest V3
and Windows native messaging. Browser/enterprise policies can prohibit unpacked
extensions. This package is an unpacked development extension, not a Web Store install.

Download filename determination is held while the separate app prompt is open.
Firefox's blocking webRequest API is not available to normal Manifest V3 extensions.
If app access fails, the browser download continues. Browser-only, private and
security-blocked requests are left alone. Some browser-managed downloads can complete
before extensions receive a usable event. File types have no fixed allowlist.

On pages with videos, Download latest sends the most recently detected video.
The arrow opens a scrollable list upward, including the latest video. The X hides
the control until you refresh the page. Protected/DRM streams are not supported.

Extension ID: jancefngmifbnaldahilleoinncjppjh
