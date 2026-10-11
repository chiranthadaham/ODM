use crate::{http, model::*, win::*};
use std::{
    path::Path,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
use windows_sys::Win32::System::Pipes::*;
pub fn quote(value: &str) -> String {
    let mut out = "\"".to_owned();
    let mut slashes = 0;
    for c in value.chars() {
        if c == '\\' {
            slashes += 1;
            continue;
        }
        if c == '"' {
            out.push_str(&"\\".repeat(slashes * 2 + 1));
            out.push('"')
        } else {
            out.push_str(&"\\".repeat(slashes));
            out.push(c)
        }
        slashes = 0;
    }
    out.push_str(&"\\".repeat(slashes * 2));
    out.push('"');
    out
}
pub fn run<F: FnMut(&str)>(
    executable: &Path,
    args: &[String],
    stdin: &[u8],
    j: &Arc<Job>,
    engine: Option<&Engine>,
    mut line: F,
) -> Result<(u32, String), String> {
    unsafe {
        let attrs = windows_sys::Win32::Security::SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<windows_sys::Win32::Security::SECURITY_ATTRIBUTES>()
                as u32,
            lpSecurityDescriptor: null_mut(),
            bInheritHandle: 1,
        };
        let (mut read, mut write, mut input, mut feed) =
            (null_mut(), null_mut(), null_mut(), null_mut());
        if CreatePipe(&mut read, &mut write, &attrs, 0) == 0 {
            return Err(error("Create helper output"));
        }
        let (read, write) = (Handle(read), Handle(write));
        SetHandleInformation(read.0, HANDLE_FLAG_INHERIT, 0);
        if CreatePipe(&mut input, &mut feed, &attrs, 0) == 0 {
            return Err(error("Create helper input"));
        }
        let (input, feed) = (Handle(input), Handle(feed));
        SetHandleInformation(feed.0, HANDLE_FLAG_INHERIT, 0);
        let group = Handle(CreateJobObjectW(null(), null()));
        if group.0.is_null() {
            return Err(error("Create process group"));
        }
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            group.0,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&limits) as u32,
        ) == 0
        {
            return Err(error("Protect helper process"));
        }
        let mut startup = STARTUPINFOW::default();
        startup.cb = std::mem::size_of_val(&startup) as u32;
        startup.dwFlags = STARTF_USESTDHANDLES | STARTF_USESHOWWINDOW;
        startup.wShowWindow = SW_HIDE as u16;
        startup.hStdInput = input.0;
        startup.hStdOutput = write.0;
        startup.hStdError = write.0;
        let mut info = PROCESS_INFORMATION::default();
        let mut command = quote(&executable.to_string_lossy());
        for a in args {
            command.push(' ');
            command.push_str(&quote(a))
        }
        let mut command = wide(command);
        let work = wide(
            executable
                .parent()
                .unwrap_or(Path::new("."))
                .to_string_lossy(),
        );
        if CreateProcessW(
            wide(executable.to_string_lossy()).as_ptr(),
            command.as_mut_ptr(),
            null(),
            null(),
            1,
            CREATE_NO_WINDOW | CREATE_SUSPENDED,
            null(),
            work.as_ptr(),
            &startup,
            &mut info,
        ) == 0
        {
            return Err(error("Start helper"));
        }
        let process = Handle(info.hProcess);
        let thread = Handle(info.hThread);
        if AssignProcessToJobObject(group.0, process.0) == 0 {
            TerminateProcess(process.0, 1);
            return Err(error("Protect helper group"));
        }
        ResumeThread(thread.0);
        drop(write);
        drop(input);
        if !stdin.is_empty() {
            let mut written = 0;
            if WriteFile(
                feed.0,
                stdin.as_ptr(),
                stdin.len() as u32,
                &mut written,
                null_mut(),
            ) == 0
            {
                return Err(error("Configure helper"));
            }
        }
        drop(feed);
        let initial = engine.map(|e| {
            (
                e.limit_kib.load(Ordering::Relaxed),
                e.connections.load(Ordering::Relaxed),
            )
        });
        let mut poll = Instant::now();
        let mut pending = Vec::new();
        let mut tail = String::new();
        let started = Instant::now();
        loop {
            if poll.elapsed() >= Duration::from_millis(250) {
                line("");
                poll = Instant::now();
            }
            let mut available = 0;
            PeekNamedPipe(
                read.0,
                null_mut(),
                0,
                null_mut(),
                &mut available,
                null_mut(),
            );
            if available > 0 {
                let mut buffer = vec![0u8; (available as usize).min(65536)];
                let mut n = 0;
                if ReadFile(
                    read.0,
                    buffer.as_mut_ptr().cast(),
                    buffer.len() as u32,
                    &mut n,
                    null_mut(),
                ) != 0
                {
                    pending.extend_from_slice(&buffer[..n as usize]);
                    while let Some(at) = pending.iter().position(|b| *b == b'\n' || *b == b'\r') {
                        let text = String::from_utf8_lossy(&pending[..at]).into_owned();
                        pending.drain(..=at);
                        line(&text);
                        tail.push_str(&text);
                        tail.push('\n');
                        if tail.len() > 8192 {
                            tail = tail
                                .chars()
                                .rev()
                                .take(4096)
                                .collect::<String>()
                                .chars()
                                .rev()
                                .collect();
                        }
                    }
                }
            }
            if j.stopped() {
                TerminateJobObject(group.0, 2);
                WaitForSingleObject(process.0, 5000);
                return Err("Paused".into());
            }
            if let (Some(e), Some(settings)) = (engine, initial) {
                if settings
                    != (
                        e.limit_kib.load(Ordering::Relaxed),
                        e.connections.load(Ordering::Relaxed),
                    )
                {
                    TerminateJobObject(group.0, 2);
                    WaitForSingleObject(process.0, 5000);
                    return Err("SETTINGS_CHANGED".into());
                }
            }
            if WaitForSingleObject(process.0, 20) == WAIT_OBJECT_0 {
                let mut more = 0;
                PeekNamedPipe(read.0, null_mut(), 0, null_mut(), &mut more, null_mut());
                if more == 0 {
                    break;
                }
            }
            if pending.len() > 262144 {
                return Err("Helper returned an oversized output line".into());
            }
            if started.elapsed() > Duration::from_secs(86400 * 7) {
                return Err("Helper timed out".into());
            }
        }
        if !pending.is_empty() {
            let text = String::from_utf8_lossy(&pending);
            line(&text);
            tail.push_str(&text);
        }
        let mut code = 1;
        GetExitCodeProcess(process.0, &mut code);
        Ok((code, tail))
    }
}
fn config(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
pub fn curl_download(engine: &Engine, j: &Arc<Job>) -> Result<(), String> {
    let d = j.snapshot();
    let part = http::partial(&d.path);
    let headers = part.with_extension("part.headers");
    if let Some(p) = part.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?
    }
    let curl = std::path::PathBuf::from(
        std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
    )
    .join("System32/curl.exe");
    if !curl.exists() {
        return Err("Windows curl transport is unavailable".into());
    }
    let n = engine.network.lock().unwrap().clone();
    let mut settings = format!(
        "url = \"{}\"\noutput = \"{}\"\ndump-header = \"{}\"\n",
        config(&d.url),
        config(&part.to_string_lossy()),
        config(&headers.to_string_lossy())
    );
    if !n.proxy.is_empty() {
        settings += &format!("proxy = \"{}\"\n", config(&n.proxy));
    }
    if !n.proxy_user.is_empty() {
        settings += &format!(
            "proxy-user = \"{}\"\n",
            config(&format!("{}:{}", n.proxy_user, n.proxy_password))
        );
    }
    if !n.server_user.is_empty()
        && valid_url(&d.url)?.origin().ascii_serialization() == n.server_origin
    {
        settings += &format!(
            "user = \"{}\"\n",
            config(&format!("{}:{}", n.server_user, n.server_password))
        );
    }
    if !d.user_agent.is_empty() {
        settings += &format!("user-agent = \"{}\"\n", config(&d.user_agent));
    }
    if !d.referer.is_empty() {
        settings += &format!("referer = \"{}\"\n", config(&d.referer));
    }
    loop {
        if j.stopped() {
            return Ok(());
        }
        let allowed = if d.url.starts_with("https:") {
            "=https"
        } else {
            "=http,https,ftp,ftps"
        };
        let mut args = vec![
            "--config",
            "-",
            "--location",
            "--max-redirs",
            "10",
            "--fail",
            "--show-error",
            "--silent",
            "--anyauth",
            "--proxy-anyauth",
            "--connect-timeout",
            "15",
            "--speed-time",
            "30",
            "--speed-limit",
            "1",
            "--retry",
            "2",
            "--proto",
            "=http,https,ftp,ftps",
            "--proto-redir",
            allowed,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        let limit = engine.limit_kib.load(Ordering::Relaxed);
        if !n.proxy.is_empty() {
            args.extend(["--noproxy".into(), String::new()]);
        }
        if limit > 0 {
            args.extend(["--limit-rate".into(), format!("{}K", limit)])
        }
        let offset = if d.url.starts_with("ftp") {
            std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };
        if offset > 0 {
            args.extend(["--continue-at".into(), "-".into()])
        }
        j.lanes.store(1, Ordering::Relaxed);
        let mut last = Instant::now();
        let mut previous = offset;
        let result = run(&curl, &args, settings.as_bytes(), j, Some(engine), |_| {
            if let Ok(m) = std::fs::metadata(&part) {
                j.done.store(m.len(), Ordering::Relaxed);
                if last.elapsed().as_secs_f64() >= 0.5 {
                    j.speed.store(
                        (m.len().saturating_sub(previous) as f64 / last.elapsed().as_secs_f64())
                            as u64,
                        Ordering::Relaxed,
                    );
                    previous = m.len();
                    last = Instant::now();
                }
            }
            if let Ok(s) = std::fs::read_to_string(&headers) {
                let final_headers = s.trim().rsplit("\r\n\r\n").next().unwrap_or("");
                for line in final_headers.lines() {
                    if let Some((key, v)) = line.split_once(':') {
                        if key.eq_ignore_ascii_case("content-length") {
                            if let Ok(n) = v.trim().parse::<u64>() {
                                j.total.store(n + offset, Ordering::Relaxed);
                            }
                        }
                    }
                }
            }
        });
        let (code, tail) = match result {
            Err(e) if e == "SETTINGS_CHANGED" => continue,
            other => other?,
        };
        if j.stopped() {
            return Ok(());
        }
        if code != 0 {
            return Err(format!("Network transfer failed: {}", tail.trim()));
        }
        if let Ok(s) = std::fs::read_to_string(&headers) {
            let final_headers = s
                .trim()
                .rsplit("\r\n\r\n")
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            let suffix = valid_url(&d.url)?.path().to_ascii_lowercase();
            if final_headers.contains("content-type: text/html")
                && !final_headers.contains("content-disposition: attachment")
                && !suffix.ends_with(".html")
                && !suffix.ends_with(".htm")
            {
                return Err(
                    "The website returned a web page instead of a downloadable file.".into(),
                );
            }
        }
        let bytes = std::fs::metadata(&part).map_err(|e| e.to_string())?.len();
        j.done.store(bytes, Ordering::Relaxed);
        j.total.store(bytes, Ordering::Relaxed);
        http::finalize(engine, j, &part)?;
        let _ = std::fs::remove_file(&headers);
        return Ok(());
    }
}
