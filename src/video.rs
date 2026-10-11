use crate::{http, model::*, process, runtime};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
};
pub fn download(engine: &Engine, j: &Arc<Job>) -> Result<(), String> {
    j.set_error("Preparing video support…".into());
    let tools = runtime::tools(engine)?;
    for name in ["yt-dlp.exe", "ffmpeg.exe", "ffprobe.exe", "deno.exe"] {
        if !tools.join(name).is_file() {
            return Err(format!("Video helper missing: {name}"));
        }
    }
    let d = j.snapshot();
    let parent = d.path.parent().ok_or("Choose a download folder")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let identity = format!("{:x}", Sha256::digest(d.path.to_string_lossy().as_bytes()));
    let folder = parent.join(format!(".odm-video-{}", &identity[..20]));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let mut attempt = 0;
    loop {
        if j.stopped() {
            return Ok(());
        }
        j.lanes.store(
            engine.connections.load(Ordering::Relaxed).min(4),
            Ordering::Relaxed,
        );
        j.set_error("Finding available video and audio formats…".into());
        let mut args=["--ignore-config","--no-playlist","--no-simulate","--newline","--progress","--encoding","utf-8","--windows-filenames","--trim-filenames","180","--no-overwrites","--continue","--no-cache-dir","--retries","3","--fragment-retries","3","--abort-on-unavailable-fragments","--socket-timeout","15","--http-chunk-size","4M","--format","bv*+ba/b","--merge-output-format","mp4","--print","after_move:ODM_FILE:%(filepath)s","--progress-template","download:ODM_PROGRESS:%(progress.downloaded_bytes)s|%(progress.total_bytes,progress.total_bytes_estimate)s|%(progress.speed)s"].into_iter().map(str::to_owned).collect::<Vec<_>>();
        args.extend([
            "--concurrent-fragments".into(),
            engine
                .connections
                .load(Ordering::Relaxed)
                .min(4)
                .to_string(),
            "--ffmpeg-location".into(),
            tools.to_string_lossy().into_owned(),
            "--js-runtimes".into(),
            format!("deno:{}", tools.join("deno.exe").display()),
            "--paths".into(),
            folder.to_string_lossy().into_owned(),
            "--output".into(),
            "%(title).120B [%(id)s].%(ext)s".into(),
        ]);
        let limit = engine.limit_kib.load(Ordering::Relaxed);
        if limit > 0 {
            args.extend(["--limit-rate".into(), format!("{limit}K")])
        }
        if d.url.to_ascii_lowercase().contains(".m3u8") {
            args.push("--hls-use-mpegts".into());
        }
        let n = engine.network.lock().unwrap().clone();
        let mut private = String::new();
        if !n.proxy.is_empty() {
            let mut proxy = url::Url::parse(&n.proxy).map_err(|e| e.to_string())?;
            if !n.proxy_user.is_empty() {
                let _ = proxy.set_username(&n.proxy_user);
                let _ = proxy.set_password(Some(&n.proxy_password));
            }
            private = format!(
                "--proxy \"{}\"\n",
                proxy.as_str().replace('\\', "\\\\").replace('"', "\\\"")
            );
            args.extend(["--config-locations".into(), "-".into()]);
        }
        args.extend(["--".into(), d.url.clone()]);
        let mut final_path = None;
        let outcome = process::run(
            &tools.join("yt-dlp.exe"),
            &args,
            private.as_bytes(),
            j,
            Some(engine),
            |line| {
                if let Some(path) = line.strip_prefix("ODM_FILE:") {
                    final_path = Some(PathBuf::from(path.trim()));
                }
                if let Some(p) = line.strip_prefix("ODM_PROGRESS:") {
                    let values = p.split('|').collect::<Vec<_>>();
                    if values.len() >= 3 {
                        j.done
                            .store(values[0].parse().unwrap_or(0), Ordering::Relaxed);
                        j.total
                            .store(values[1].parse().unwrap_or(0), Ordering::Relaxed);
                        j.speed.store(
                            values[2].parse::<f64>().unwrap_or(0.) as u64,
                            Ordering::Relaxed,
                        );
                        j.set_error(String::new());
                    }
                }
            },
        );
        match outcome {
            Err(e) if e == "SETTINGS_CHANGED" => continue,
            Err(e) => return Err(e),
            Ok((code, tail)) if code != 0 => {
                if tail.contains("403") && attempt < 2 {
                    attempt += 1;
                    continue;
                }
                return Err(tail);
            }
            _ => (),
        }
        let mut source = final_path.ok_or("Video helper did not produce a completed file")?;
        let folder_abs = folder.canonicalize().map_err(|e| e.to_string())?;
        let source_abs = source.canonicalize().map_err(|e| e.to_string())?;
        if !source_abs.starts_with(&folder_abs) || !source_abs.is_file() {
            return Err("Invalid video output path".into());
        }
        if valid_url(&d.url)?
            .path()
            .to_ascii_lowercase()
            .ends_with(".m3u8")
        {
            j.assembling.store(true, Ordering::Relaxed);
            j.set_error("Finalizing HLS transport stream…".into());
            let temporary = folder.join(".odm-hls-final.part");
            let _ = std::fs::remove_file(&temporary);
            let args = ["-hide_banner", "-loglevel", "error", "-nostdin", "-i"]
                .into_iter()
                .map(str::to_owned)
                .chain([source.to_string_lossy().into_owned()])
                .chain(
                    ["-map", "0:v?", "-map", "0:a?", "-c", "copy", "-f", "mpegts"]
                        .into_iter()
                        .map(str::to_owned),
                )
                .chain([temporary.to_string_lossy().into_owned()])
                .collect::<Vec<_>>();
            let (code, tail) =
                process::run(&tools.join("ffmpeg.exe"), &args, &[], j, None, |_| {})?;
            if code != 0 {
                return Err(format!("Cannot finalize HLS; source was preserved: {tail}"));
            }
            if j.stopped() {
                return Ok(());
            }
            let target = source.with_extension("ts");
            crate::win::move_replace(&temporary, &target)?;
            if source != target {
                let _ = std::fs::remove_file(source);
            }
            source = target;
        }
        let mut target = d.path.clone();
        target.set_file_name(source.file_name().ok_or("Invalid video filename")?);
        j.details.lock().unwrap().path = target;
        http::finalize(engine, j, &source)?;
        for entry in std::fs::read_dir(&folder).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            if entry.file_type().map_err(|e| e.to_string())?.is_file() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        let _ = std::fs::remove_dir(folder);
        j.set_error(String::new());
        return Ok(());
    }
}
