use crate::{model::*, win::*};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicPtr, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
pub struct Internet(*mut c_void);
unsafe impl Send for Internet {}
impl Internet {
    fn new(h: *mut c_void) -> Result<Self, String> {
        if h.is_null() {
            Err(error("Network operation"))
        } else {
            Ok(Self(h))
        }
    }
}
impl Drop for Internet {
    fn drop(&mut self) {
        unsafe {
            WinHttpCloseHandle(self.0);
        }
    }
}
pub struct Response {
    _session: Internet,
    _connection: Internet,
    pub handle: Arc<AtomicPtr<c_void>>,
    finished: Arc<AtomicBool>,
    pub status: u32,
}
impl Drop for Response {
    fn drop(&mut self) {
        self.finished.store(true, Ordering::Release);
        let h = self.handle.swap(null_mut(), Ordering::AcqRel);
        if !h.is_null() {
            unsafe {
                WinHttpCloseHandle(h);
            }
        }
    }
}
impl Response {
    pub fn h(&self) -> *mut c_void {
        self.handle.load(Ordering::Acquire)
    }
    pub fn header(&self, kind: u32) -> String {
        unsafe {
            let mut bytes = 0;
            WinHttpQueryHeaders(self.h(), kind, null(), null_mut(), &mut bytes, null_mut());
            if bytes == 0 || bytes > 131072 {
                return String::new();
            }
            let mut buf = vec![0u16; (bytes as usize + 1) / 2];
            if WinHttpQueryHeaders(
                self.h(),
                kind,
                null(),
                buf.as_mut_ptr().cast(),
                &mut bytes,
                null_mut(),
            ) == 0
            {
                String::new()
            } else {
                from_wide(&buf)
            }
        }
    }
    pub fn actual_url(&self) -> String {
        unsafe {
            let mut bytes = 0;
            WinHttpQueryOption(self.h(), WINHTTP_OPTION_URL, null_mut(), &mut bytes);
            if bytes == 0 || bytes > 131072 {
                return String::new();
            }
            let mut buf = vec![0u16; (bytes as usize + 1) / 2];
            if WinHttpQueryOption(
                self.h(),
                WINHTTP_OPTION_URL,
                buf.as_mut_ptr().cast(),
                &mut bytes,
            ) == 0
            {
                String::new()
            } else {
                from_wide(&buf)
            }
        }
    }
    pub fn read(&self, buf: &mut [u8]) -> Result<usize, String> {
        let mut n = 0;
        if unsafe { WinHttpReadData(self.h(), buf.as_mut_ptr().cast(), buf.len() as u32, &mut n) }
            == 0
        {
            Err(error("Read download"))
        } else {
            Ok(n as usize)
        }
    }
}
pub fn request(
    engine: &Engine,
    j: &Arc<Job>,
    address: &str,
    range: Option<&str>,
    validator: Option<&str>,
) -> Result<Response, String> {
    let url = valid_url(address)?;
    let d = j.snapshot();
    for s in [&d.user_agent, &d.referer] {
        if s.len() > 2048 || s.chars().any(|c| c == '\r' || c == '\n' || c == '\0') {
            return Err("Invalid browser context".into());
        }
    }
    let config = engine.network.lock().unwrap().clone();
    let proxy = config.proxy.strip_prefix("http://").unwrap_or("");
    let agent = if d.user_agent.is_empty() {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) OpenDownloadManager/3.0"
    } else {
        &d.user_agent
    };
    let proxy_wide = wide(proxy);
    unsafe {
        let session = Internet::new(WinHttpOpen(
            wide(agent).as_ptr(),
            if proxy.is_empty() {
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY
            } else {
                WINHTTP_ACCESS_TYPE_NAMED_PROXY
            },
            if proxy.is_empty() {
                null()
            } else {
                proxy_wide.as_ptr()
            },
            null(),
            0,
        ))?;
        WinHttpSetTimeouts(session.0, 10000, 10000, 10000, 30000);
        let conn = Internet::new(WinHttpConnect(
            session.0,
            wide(url.host_str().unwrap_or("")).as_ptr(),
            url.port_or_known_default().unwrap_or(80),
            0,
        ))?;
        let resource = format!(
            "{}{}",
            url.path(),
            url.query().map(|q| format!("?{q}")).unwrap_or_default()
        );
        let h = WinHttpOpenRequest(
            conn.0,
            wide("GET").as_ptr(),
            wide(resource).as_ptr(),
            null(),
            null(),
            null(),
            if url.scheme() == "https" {
                WINHTTP_FLAG_SECURE
            } else {
                0
            },
        );
        if h.is_null() {
            return Err(error("Open request"));
        }
        let handle = Arc::new(AtomicPtr::new(h));
        let finished = Arc::new(AtomicBool::new(false));
        let mut response = Response {
            _session: session,
            _connection: conn,
            handle: handle.clone(),
            finished: finished.clone(),
            status: 0,
        };
        let watched = j.clone();
        std::thread::spawn(move || {
            while !finished.load(Ordering::Acquire) {
                if watched.stopped() {
                    let h = handle.swap(null_mut(), Ordering::AcqRel);
                    if !h.is_null() {
                        WinHttpCloseHandle(h);
                    }
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        let policy = WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP;
        WinHttpSetOption(
            h,
            WINHTTP_OPTION_REDIRECT_POLICY,
            (&policy as *const u32).cast(),
            4,
        );
        let mut headers = "Accept-Encoding: identity\r\n".to_owned();
        if let Some(r) = range {
            headers += &format!("Range: bytes={r}\r\n");
        }
        if let Some(v) = validator {
            headers += &format!("If-Range: {v}\r\n");
        }
        if !d.referer.is_empty() {
            headers += &format!("Referer: {}\r\n", d.referer);
        }
        let headers = wide(headers);
        for _ in 0..4 {
            if j.stopped() {
                return Err("Paused".into());
            }
            if WinHttpSendRequest(h, headers.as_ptr(), u32::MAX, null(), 0, 0, 0) == 0
                || WinHttpReceiveResponse(h, null_mut()) == 0
            {
                return Err(error("HTTP request"));
            }
            let mut length = 4;
            if WinHttpQueryHeaders(
                h,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                null(),
                (&mut response.status as *mut u32).cast(),
                &mut length,
                null_mut(),
            ) == 0
            {
                return Err(error("HTTP status"));
            }
            if response.status != 401 && response.status != 407 {
                break;
            }
            let (mut supported, mut first, mut target) = (0, 0, 0);
            if WinHttpQueryAuthSchemes(h, &mut supported, &mut first, &mut target) == 0 {
                break;
            }
            let (user, password) = if response.status == 407 {
                (&config.proxy_user, &config.proxy_password)
            } else {
                let current_origin = url::Url::parse(&response.actual_url())
                    .ok()
                    .map(|u| u.origin().ascii_serialization());
                if current_origin.as_deref() != Some(config.server_origin.as_str()) {
                    break;
                }
                (&config.server_user, &config.server_password)
            };
            if user.is_empty() {
                break;
            }
            let scheme = [
                WINHTTP_AUTH_SCHEME_NEGOTIATE,
                WINHTTP_AUTH_SCHEME_DIGEST,
                WINHTTP_AUTH_SCHEME_NTLM,
                WINHTTP_AUTH_SCHEME_BASIC,
            ]
            .into_iter()
            .find(|s| supported & s != 0)
            .unwrap_or(0);
            if scheme == 0
                || WinHttpSetCredentials(
                    h,
                    if response.status == 407 {
                        WINHTTP_AUTH_TARGET_PROXY
                    } else {
                        WINHTTP_AUTH_TARGET_SERVER
                    },
                    scheme,
                    wide(user).as_ptr(),
                    wide(password).as_ptr(),
                    null_mut(),
                ) == 0
            {
                break;
            }
        }
        Ok(response)
    }
}
pub fn status_error(code: u32) -> String {
    match code{403=>"HTTP 403: website refused this download. Open the website and complete verification, then send its actual file link.".into(),401=>"HTTP 401: authentication required. Set credentials for this server in Settings.".into(),407=>"HTTP 407: proxy authentication required. Check Settings.".into(),_=>format!("HTTP {code}: the website refused this download")}
}
pub fn range(value: &str) -> Option<(u64, u64, u64)> {
    let text = value.strip_prefix("bytes ")?;
    let (span, total) = text.split_once('/')?;
    let (start, end) = span.split_once('-')?;
    let (start, end, total): (u64, u64, u64) =
        (start.parse().ok()?, end.parse().ok()?, total.parse().ok()?);
    if start > end || end >= total {
        return None;
    }
    Some((start, end, total))
}
pub fn validator(response: &Response) -> String {
    let etag = response.header(WINHTTP_QUERY_ETAG);
    if !etag.is_empty() && !etag.starts_with("W/") {
        etag
    } else {
        response.header(WINHTTP_QUERY_LAST_MODIFIED)
    }
}
pub fn download_response(
    engine: &Engine,
    j: &Arc<Job>,
    response: Response,
    file: &Path,
    append: bool,
    expected: Option<u64>,
    abort: Option<&AtomicBool>,
) -> Result<(), String> {
    if response.status >= 400 {
        return Err(status_error(response.status));
    }
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(append)
        .truncate(!append)
        .open(file)
        .map_err(|e| format!("Cannot write download: {e}"))?;
    let mut buffer = vec![0u8; 256 * 1024];
    let mut received = 0u64;
    let mut checkpoint = Instant::now();
    let mut previous = j.done.load(Ordering::Relaxed);
    loop {
        if j.stopped() || abort.is_some_and(|a| a.load(Ordering::Relaxed)) {
            break;
        }
        let n = response.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        received += n as u64;
        if expected.is_some_and(|x| received > x) {
            return Err("Server sent too many bytes".into());
        }
        output
            .write_all(&buffer[..n])
            .map_err(|e| format!("Cannot write download: {e}"))?;
        let done = j.done.fetch_add(n as u64, Ordering::Relaxed) + n as u64;
        let pause = j.pause_after.load(Ordering::Relaxed);
        if pause > 0 && done >= pause {
            j.stop.store(true, Ordering::Relaxed)
        }
        if checkpoint.elapsed() > Duration::from_millis(500) {
            j.speed.store(
                ((done.saturating_sub(previous)) as f64 / checkpoint.elapsed().as_secs_f64())
                    as u64,
                Ordering::Relaxed,
            );
            previous = done;
            checkpoint = Instant::now();
        }
        engine.pace(j, n as u64);
    }
    output.flush().map_err(|e| e.to_string())?;
    if !j.stopped()
        && !abort.is_some_and(|a| a.load(Ordering::Relaxed))
        && expected.is_some_and(|n| received != n)
    {
        return Err("Incomplete download: received size differs from the server".into());
    }
    Ok(())
}
pub fn follow_html(response: &Response, original: &str) -> Result<Option<String>, String> {
    let mime = response
        .header(WINHTTP_QUERY_CONTENT_TYPE)
        .to_ascii_lowercase();
    let disposition = response
        .header(WINHTTP_QUERY_CONTENT_DISPOSITION)
        .to_ascii_lowercase();
    let u = valid_url(original)?;
    let suffix = Path::new(u.path())
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    if !mime.contains("text/html") && !mime.contains("application/xhtml") {
        return Ok(None);
    }
    if ["html", "htm", "xhtml"].contains(&suffix.as_str()) || disposition.starts_with("attachment")
    {
        return Ok(None);
    }
    let mut body = Vec::new();
    let mut buf = [0u8; 8192];
    while body.len() < 262144 {
        let n = response.read(&mut buf)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buf[..n]);
    }
    let body = String::from_utf8_lossy(&body);
    let tags = regex::Regex::new(r"(?is)<meta\b[^>]*>").unwrap();
    let attrs =
        regex::Regex::new(r#"(?is)([a-z_-]+)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#).unwrap();
    let refresh = regex::Regex::new(r"(?is);\s*url\s*=\s*(.+)").unwrap();
    for tag in tags.find_iter(&body) {
        let (mut equiv, mut content) = (String::new(), String::new());
        for cap in attrs.captures_iter(tag.as_str()) {
            let value = (2..=4)
                .find_map(|i| cap.get(i))
                .map(|m| m.as_str())
                .unwrap_or("");
            match cap[1].to_ascii_lowercase().as_str() {
                "http-equiv" => equiv = value.to_ascii_lowercase(),
                "content" => content = value.into(),
                _ => (),
            }
        }
        if equiv != "refresh" {
            continue;
        }
        if let Some(cap) = refresh.captures(&content) {
            let target = cap[1]
                .replace("&amp;", "&")
                .trim_matches([' ', '\'', '"'])
                .to_owned();
            let base = url::Url::parse(&response.actual_url()).unwrap_or(u.clone());
            if let Ok(next) = base.join(&target) {
                if ["http", "https"].contains(&next.scheme())
                    && !(base.scheme() == "https" && next.scheme() == "http")
                    && next.as_str() != original
                {
                    return Ok(Some(next.into()));
                }
            }
        }
    }
    Err("The website returned a web page instead of a downloadable file. Use the page's actual download button.".into())
}
pub fn check_access(
    engine: &Engine,
    address: &str,
    agent: &str,
    referer: &str,
) -> Result<(), String> {
    if video_url(address) {
        return Ok(());
    }
    let j = Job::new(Details {
        url: address.into(),
        user_agent: agent.into(),
        referer: referer.into(),
        ..Default::default()
    });
    let mut url = address.to_owned();
    for _ in 0..5 {
        let r = request(engine, &j, &url, None, None)?;
        if r.status >= 400 {
            return Err(status_error(r.status));
        }
        if let Some(next) = follow_html(&r, &url)? {
            url = next
        } else {
            return Ok(());
        }
    }
    Err("Too many download redirects".into())
}
pub fn finalize(engine: &Engine, j: &Job, temporary: &Path) -> Result<(), String> {
    let original = j.snapshot().path;
    let mut reserved = engine.reservations.lock().unwrap();
    for n in 0..10000 {
        let target = numbered(&original, n);
        if n > 0 && reserved.contains(&path_key(&target)) {
            continue;
        }
        let result = unsafe {
            MoveFileExW(
                wide(temporary.to_string_lossy()).as_ptr(),
                wide(target.to_string_lossy()).as_ptr(),
                MOVEFILE_WRITE_THROUGH,
            )
        };
        if result != 0 {
            reserved.insert(path_key(&target));
            j.details.lock().unwrap().path = target;
            return Ok(());
        }
        let code = unsafe { GetLastError() };
        if code != ERROR_ALREADY_EXISTS && code != ERROR_FILE_EXISTS {
            return Err(error("Finalize download"));
        }
    }
    Err("No unused destination filename".into())
}
pub fn partial(path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.part", path.display()))
}
