use crate::model::*;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};
pub fn normalize(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(String::new());
    }
    if value.len() != 64 || !value.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Enter a 64-character SHA-256 checksum, or leave it blank.".into());
    }
    Ok(value.to_ascii_lowercase())
}
pub fn hash(path: &Path, job: Option<&Job>) -> Result<String, String> {
    let mut file =
        std::fs::File::open(path).map_err(|e| format!("Cannot read file for verification: {e}"))?;
    let before = file.metadata().map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut buf = vec![0; 1024 * 1024];
    loop {
        if job.is_some_and(|j| j.stopped()) {
            return Err("Checksum verification paused; partial file preserved.".into());
        }
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        digest.update(&buf[..n]);
    }
    let after = file.metadata().map_err(|e| e.to_string())?;
    if before.len() != after.len() || before.modified().ok() != after.modified().ok() {
        return Err(
            "The file changed during verification. Try again when it is no longer being modified."
                .into(),
        );
    }
    Ok(format!("{:x}", digest.finalize()))
}
pub fn verify_download(engine: &Engine, j: &Job, path: &Path) -> Result<(), String> {
    let expected = normalize(&j.snapshot().expected_sha256)?;
    if expected.is_empty() && !engine.hash_downloads.load(Ordering::Relaxed) {
        return Ok(());
    }
    j.set_error("Verifying SHA-256…".into());
    let actual = hash(path, Some(j))?;
    let mut d = j.details.lock().unwrap();
    d.actual_sha256 = actual.clone();
    if !expected.is_empty() && actual != expected {
        d.integrity_status = "Mismatch — do not use this file".into();
        return Err(format!("SHA-256 mismatch. The file was not marked complete. Expected {expected}; received {actual}. Partial data is preserved; restart from the source to download again."));
    }
    d.integrity_status = if expected.is_empty() {
        "SHA-256 calculated (no reference checksum supplied)"
    } else {
        "SHA-256 verified"
    }
    .into();
    Ok(())
}
pub fn destination(base: &Path, name: &str, enabled: bool) -> PathBuf {
    if !enabled {
        return base.join(name);
    }
    let folder = match file_type(Path::new(name)) {
        "Video" => "Videos",
        "Audio" => "Audio",
        "Document" => "Documents",
        "Archive" => "Archives",
        "Image" => "Images",
        "Application" => "Applications",
        _ => "Other",
    };
    // Do not nest category folders when a previously chosen folder is reused.
    if base
        .file_name()
        .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case(folder))
    {
        base.join(name)
    } else {
        base.join(folder).join(name)
    }
}
