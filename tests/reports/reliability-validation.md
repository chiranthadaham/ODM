# Integrity, recovery and organization validation

Date: 12 October 2026. Native Rust Windows build, version 0.2.0.

## Passed

- Correct SHA-256 completes and produces byte-identical output.
- Incorrect SHA-256 prevents final publication and preserves sequential or segmented partial data.
- A changed server validator restarts a partial file instead of appending incompatible bytes.
- Previously complete segments without a stable validator are rejected and downloaded again.
- A truncated primary queue recovers from its valid backup; saved transfers remain paused.
- A recovered sequential download resumes at its saved byte offset and matches the original file.
- Completed-file checksum calculation, successful comparison, and mismatch reporting.
- Optional category-folder suggestions in the save prompt and creation of the destination directory.
- Organization and automatic hashing settings persist with the queue.
- Queue backup remains valid after subsequent saves.
- Existing native UI download, browser handoff, theme switching, sorting and popup regression checks.
- Seven Rust unit tests, including checksum input validation and category paths.
- Dark-mode visual inspection of settings and the checksum-enabled save prompt.
- Rust release build and Inno Setup compilation.

Tests use a local HTTP server and an isolated data directory. They simulate saved interrupted transfers and damaged queue files; they do not simulate a physical power interruption. No public website download or store submission was needed for these features. Existing user downloads were not moved or removed.

## Usage

Paste a publisher's SHA-256 in **Save and Start**, or right-click a completed item and choose **Verify SHA-256…**. Automatic calculation and file-type folders are optional settings. A hash calculated without an expected value is labelled as calculated, not verified. Existing downloads retain their current locations.
