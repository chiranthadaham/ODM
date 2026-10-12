# 0.2.8 download startup fixes

13 October 2026.

- Firefox captures real HTTP(S) browser download events without waiting for body bytes, known size, or resume support.
- Browser-identified downloads skip the redundant native access preflight. The actual transfer still checks HTTP failures and validates data/resume metadata. Unidentified page links and failed-browser-request recovery retain preflight validation.
- Explicit HTTP(S) links with the download attribute go directly to the app regardless of filename or extension. Blob links still require a unique matching HTTP source; generated-only blobs remain browser-managed.
- Single-connection HTTP transfers use the first normal GET response, avoiding range negotiation. Multiple connections retain existing validated segmentation.
- Consent, private-window exclusions, Chrome store restrictions, user confirmation, cancellation, and browser fallback on handoff failure remain.

Validation: both browser detection suites (40 metadata events each, unknown extension/size/nonresumable cases, preflight selection, consent/private/store restrictions); both explicit-link tests; 10 HTTP optimization tests, 7 segmented transfer tests, 4 HTTP-error tests; updater checks. JavaScript syntax passed. Native app and installer rebuilt.

The event-handler fixtures have no real browser/native/network latency. They do not establish millisecond internet startup or higher bulk throughput. DNS, TLS, server response, user confirmation, authentication, POST-only requests, generated blobs and protected media can still prevent or delay a handoff. Historical test reports retain their original version numbers.
