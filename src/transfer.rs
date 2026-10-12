use crate::{http, model::*, win};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
#[derive(Serialize, Deserialize, PartialEq)]
struct Metadata {
    url: String,
    validator: String,
    total: u64,
    count: usize,
}
fn legacy_meta(folder: &Path) -> Option<Metadata> {
    let text = std::fs::read_to_string(folder.join("metadata.txt")).ok()?;
    let mut lines = text.lines();
    let url = serde_json::from_str::<String>(lines.next()?).ok()?;
    let validator = serde_json::from_str::<String>(lines.next()?).ok()?;
    let mut numbers = lines.next()?.split_whitespace();
    Some(Metadata {
        url,
        validator,
        total: numbers.next()?.parse().ok()?,
        count: numbers.next()?.parse().ok()?,
    })
}
fn save_meta(path: &Path, meta: &Metadata) -> Result<(), String> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec(meta).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    std::fs::OpenOptions::new()
        .write(true)
        .open(&tmp)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())?;
    win::move_replace(&tmp, path)
}
fn clean_segments(folder: &Path) {
    for i in 0..128 {
        let _ = std::fs::remove_file(folder.join(format!("{i}.part")));
    }
    for name in [
        "metadata.json",
        "metadata.txt",
        "metadata.tmp",
        "assembled.part",
    ] {
        let _ = std::fs::remove_file(folder.join(name));
    }
    let _ = std::fs::remove_dir(folder);
}
fn sequential(
    engine: &Engine,
    j: &Arc<Job>,
    mut response: Option<http::Response>,
    url: &str,
) -> Result<(), String> {
    let d = j.snapshot();
    let part = http::partial(&d.path);
    let existing = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
    let mut append = false;
    let mut offset = 0;
    let saved_validator = if !d.etag.is_empty() && !d.etag.starts_with("W/") {
        &d.etag
    } else {
        &d.modified
    };
    if response.is_none() && existing > 0 && !saved_validator.is_empty() {
        let r = http::request(
            engine,
            j,
            url,
            Some(&format!("{existing}-")),
            Some(saved_validator),
        )?;
        if r.status == 206
            && http::range(&r.header(win::WINHTTP_QUERY_CONTENT_RANGE))
                .is_some_and(|(start, _, _)| start == existing)
            && (http::validator(&r).is_empty() || http::validator(&r) == *saved_validator)
        {
            append = true;
            offset = existing;
            response = Some(r);
        } else if r.status == 200 {
            response = Some(r)
        }
    }
    if existing > 0 && !append {
        j.set_error("The server cannot safely resume this partial file (it changed or range requests are unavailable). Restarting from the beginning.".into());
    }
    let response = match response {
        Some(r) => r,
        None => http::request(engine, j, url, None, None)?,
    };
    if response.status >= 400 {
        return Err(http::status_error(response.status));
    }
    if http::follow_html(&response, url)?.is_some() {
        return Err("Unexpected download mirror".into());
    }
    let length = response
        .header(win::WINHTTP_QUERY_CONTENT_LENGTH)
        .parse::<u64>()
        .ok();
    let total = if append {
        http::range(&response.header(win::WINHTTP_QUERY_CONTENT_RANGE)).map(|(_, _, n)| n)
    } else {
        length
    };
    j.done.store(offset, Ordering::Relaxed);
    j.total.store(total.unwrap_or(0), Ordering::Relaxed);
    j.lanes.store(1, Ordering::Relaxed);
    j.details.lock().unwrap().etag = http::validator(&response);
    std::fs::write(part.with_extension("part.validator"), &j.snapshot().etag)
        .map_err(|e| format!("Cannot save resume information: {e}"))?;
    http::download_response(engine, j, response, &part, append, length, None)?;
    if j.stopped() {
        return Ok(());
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&part)
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    http::finalize(engine, j, &part)?;
    let _ = std::fs::remove_file(part.with_extension("part.validator"));
    Ok(())
}
fn http_transfer(engine: &Arc<Engine>, j: &Arc<Job>) -> Result<(), String> {
    let original = j.snapshot();
    if let Some(parent) = original.path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?
    }
    let mut address = original.url;
    let folder = PathBuf::from(format!("{}.segments", original.path.display()));
    let partial = http::partial(&original.path);
    if partial.exists() && !folder.exists() {
        if j.snapshot().etag.is_empty() {
            if let Ok(v) = std::fs::read_to_string(partial.with_extension("part.validator")) {
                j.details.lock().unwrap().etag = v
            }
        }
        return sequential(engine, j, None, &address);
    }
    let mut probe = None;
    for depth in 0..5 {
        let mut response = http::request(
            engine,
            j,
            &address,
            if depth == 0 && engine.connections.load(Ordering::Relaxed) > 1 { Some("0-0") } else { None },
            None,
        )?;
        // Some publishers reject a tiny range probe while accepting a normal GET.
        // Retry once without Range; an actual access refusal still fails normally.
        if depth == 0 && matches!(response.status, 403 | 405 | 416 | 429) {
            if response.status == 429 {
                let retry = response.header(win::WINHTTP_QUERY_RETRY_AFTER);
                let seconds = if retry.is_empty() {
                    2
                } else {
                    match retry.parse::<u64>() {
                        Ok(n) if n <= 30 => n,
                        _ => return Err(http::status_error(response.status)),
                    }
                };
                for _ in 0..seconds * 10 {
                    if j.stopped() {
                        return Ok(());
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            drop(response);
            response = http::request(engine, j, &address, None, None)?;
        }
        if response.status >= 400 {
            return Err(http::status_error(response.status));
        }
        if let Some(next) = http::follow_html(&response, &address)? {
            if depth == 4 {
                return Err("Too many download redirects".into());
            }
            address = next;
            continue;
        }
        probe = Some(response);
        break;
    }
    let probe = probe.ok_or("No download response")?;
    if probe.status == 200 {
        clean_segments(&folder);
        return sequential(engine, j, Some(probe), &address);
    }
    let range = http::range(&probe.header(win::WINHTTP_QUERY_CONTENT_RANGE));
    let validator = http::validator(&probe);
    let total = range.map(|(_, _, n)| n).unwrap_or(0);
    if probe.status != 206
        || range.map(|(a, b, _)| (a, b)) != Some((0, 0))
        || total < 512 * 1024
        || validator.is_empty()
    {
        drop(probe);
        clean_segments(&folder);
        return sequential(engine, j, None, &address);
    }
    drop(probe);
    let mut count = (total / (2 * 1024 * 1024))
        .max(engine.connections.load(Ordering::Relaxed) as u64)
        .clamp(2, 128) as usize;
    let mut meta = Metadata {
        url: address.clone(),
        validator: validator.clone(),
        total,
        count,
    };
    let old = std::fs::read(folder.join("metadata.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<Metadata>(&b).ok())
        .or_else(|| legacy_meta(&folder));
    let mut reuse = false;
    if let Some(saved) = old {
        if !validator.is_empty()
            && saved.url == address
            && saved.validator == validator
            && saved.total == total
            && (2..=128).contains(&saved.count)
        {
            count = saved.count;
            meta.count = count;
            reuse = true
        }
    }
    if !reuse {
        if folder.exists() {
            j.set_error("Saved segments no longer match the server or have no stable validator. Restarting safely.".into());
        }
        clean_segments(&folder)
    }
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    save_meta(&folder.join("metadata.json"), &meta)?;
    let mut existing = 0;
    for i in 0..count {
        let start = total * i as u64 / count as u64;
        let end = total * (i + 1) as u64 / count as u64 - 1;
        let file = folder.join(format!("{i}.part"));
        let length = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
        if length > end - start + 1 {
            let _ = std::fs::remove_file(file);
        } else {
            existing += length
        }
    }
    j.done.store(existing, Ordering::Relaxed);
    j.total.store(total, Ordering::Relaxed);
    j.lanes.store(
        count.min(engine.connections.load(Ordering::Relaxed)),
        Ordering::Relaxed,
    );
    let next = Arc::new(AtomicUsize::new(0));
    let abort = Arc::new(AtomicBool::new(false));
    let failure = Arc::new(Mutex::new(None::<String>));
    let fallback = Arc::new(AtomicBool::new(false));
    let mut workers = Vec::new();
    for worker in 0..count.min(16) {
        let (engine, j, next, abort, failure, fallback, folder, address, validator) = (
            engine.clone(),
            j.clone(),
            next.clone(),
            abort.clone(),
            failure.clone(),
            fallback.clone(),
            folder.clone(),
            address.clone(),
            validator.clone(),
        );
        workers.push(std::thread::spawn(move || {
            let result = (|| {
                loop {
                    if j.stopped()
                        || abort.load(Ordering::Relaxed)
                        || next.load(Ordering::Relaxed) >= count
                    {
                        break;
                    }
                    if worker >= engine.connections.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(25));
                        continue;
                    }
                    j.lanes.store(
                        count.min(engine.connections.load(Ordering::Relaxed)),
                        Ordering::Relaxed,
                    );
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= count {
                        break;
                    }
                    let start = total * i as u64 / count as u64;
                    let end = total * (i + 1) as u64 / count as u64 - 1;
                    let file = folder.join(format!("{i}.part"));
                    let offset = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
                    if offset == end - start + 1 {
                        continue;
                    }
                    let response = http::request(
                        &engine,
                        &j,
                        &address,
                        Some(&format!("{}-{end}", start + offset)),
                        Some(&validator),
                    )?;
                    let returned = http::validator(&response);
                    if response.status == 200
                        || response.status == 429
                        || http::range(&response.header(win::WINHTTP_QUERY_CONTENT_RANGE))
                            != Some((start + offset, end, total))
                        || (!returned.is_empty() && returned != validator)
                    {
                        fallback.store(true, Ordering::Relaxed);
                        return Err("Server changed or refused the requested byte range".to_owned());
                    }
                    if response.status != 206 {
                        return Err(http::status_error(response.status));
                    }
                    http::download_response(
                        &engine,
                        &j,
                        response,
                        &file,
                        offset > 0,
                        Some(end - start + 1 - offset),
                        Some(&abort),
                    )?;
                }
                Ok::<(), String>(())
            })();
            if let Err(error) = result {
                let mut f = failure.lock().unwrap();
                if f.is_none() {
                    *f = Some(error)
                }
                abort.store(true, Ordering::Relaxed);
            }
        }));
    }
    for worker in workers {
        if worker.join().is_err() {
            return Err("Download worker failed".into());
        }
    }
    if j.stopped() {
        return Ok(());
    }
    if fallback.load(Ordering::Relaxed) {
        clean_segments(&folder);
        return sequential(engine, j, None, &address);
    }
    if let Some(error) = failure.lock().unwrap().take() {
        return Err(error);
    }
    j.assembling.store(true, Ordering::Relaxed);
    j.assembled.store(0, Ordering::Relaxed);
    let assembled = folder.join("assembled.part");
    let mut output = std::fs::File::create(&assembled).map_err(|e| e.to_string())?;
    let mut buffer = vec![0u8; 4 * 1024 * 1024];
    for i in 0..count {
        let mut input =
            std::fs::File::open(folder.join(format!("{i}.part"))).map_err(|e| e.to_string())?;
        loop {
            if j.stopped() {
                drop(output);
                let _ = std::fs::remove_file(&assembled);
                return Ok(());
            }
            let n = input.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            output
                .write_all(&buffer[..n])
                .map_err(|e| format!("Cannot assemble download: {e}"))?;
            j.assembled.fetch_add(n as u64, Ordering::Relaxed);
        }
    }
    output.sync_all().map_err(|e| e.to_string())?;
    let bytes = output.metadata().map_err(|e| e.to_string())?.len();
    drop(output);
    if bytes != total {
        return Err("Assembled size differs from the server".into());
    }
    http::finalize(engine, j, &assembled)?;
    clean_segments(&folder);
    Ok(())
}
pub fn run(engine: Arc<Engine>, j: Arc<Job>) {
    j.active.store(true, Ordering::Release);
    j.status.store(DOWNLOADING, Ordering::Relaxed);
    j.set_error(String::new());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        valid_url(&j.snapshot().url)?;
        if video_url(&j.snapshot().url) {
            crate::video::download(&engine, &j)
        } else {
            let config = engine.network.lock().unwrap().clone();
            if !j.snapshot().url.starts_with("http")
                || (!config.proxy.is_empty() && !config.proxy.starts_with("http://"))
            {
                crate::process::curl_download(&engine, &j)
            } else {
                let mut result = Ok(());
                for attempt in 0..3 {
                    result = http_transfer(&engine, &j);
                    let path = j.snapshot().path;
                    if matches!(&result, Err(e) if e.starts_with("HTTP request") && e.contains("Windows error 12002"))
                        && !j.stopped()
                        && j.done.load(Ordering::Relaxed) == 0
                        && !http::partial(&path).exists()
                        && !PathBuf::from(format!("{}.segments", path.display())).exists()
                    {
                        // A fresh request timed out before receiving any data. Try the
                        // alternate Windows transport without touching resumable files.
                        j.set_error("Trying an alternate connection…".into());
                        result = crate::process::curl_download(&engine, &j);
                        break;
                    }
                    let transient = match &result {
                        Err(e) => [
                            "Windows error 12002",
                            "Windows error 12007",
                            "Windows error 12029",
                            "Windows error 12030",
                            "Windows error 12031",
                            "Windows error 12032",
                            "Windows error 12152",
                            "Incomplete download",
                            "HTTP 500:",
                            "HTTP 502:",
                            "HTTP 503:",
                            "HTTP 504:",
                        ]
                        .iter()
                        .any(|s| e.contains(s)),
                        _ => false,
                    };
                    if !transient || attempt == 2 || j.stopped() {
                        break;
                    }
                    j.set_error("Connection interrupted. Retrying with saved partial data…".into());
                    for _ in 0..(20 << attempt) {
                        if j.stopped() {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
                result
            }
        }
    }));
    match result {
        Ok(Ok(())) if !j.stopped() => {
            j.set_error(String::new());
            j.status.store(COMPLETE, Ordering::Relaxed);
            if let Ok(m) = std::fs::metadata(j.snapshot().path) {
                j.done.store(m.len(), Ordering::Relaxed);
                j.total.store(m.len(), Ordering::Relaxed);
            }
        }
        Ok(Err(error)) if !j.stopped() => {
            j.set_error(error);
            j.status.store(ERROR, Ordering::Relaxed);
        }
        Err(_) if !j.stopped() => {
            j.set_error("Unexpected worker error; partial files were preserved".into());
            j.status.store(ERROR, Ordering::Relaxed);
        }
        _ => {
            if j.state() != CANCELED {
                j.status.store(PAUSED, Ordering::Relaxed);
            }
        }
    }
    j.assembling.store(false, Ordering::Relaxed);
    j.speed.store(0, Ordering::Relaxed);
    j.active.store(false, Ordering::Release);
}
