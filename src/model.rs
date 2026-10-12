use crate::win;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};
pub const QUEUED: usize = 0;
pub const DOWNLOADING: usize = 1;
pub const PAUSED: usize = 2;
pub const COMPLETE: usize = 3;
pub const ERROR: usize = 4;
pub const CANCELED: usize = 5;
pub const NAMES: [&str; 7] = [
    "Queued",
    "Downloading",
    "Paused",
    "Complete",
    "Error",
    "Canceled",
    "Scheduled",
];
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Details {
    pub url: String,
    pub path: PathBuf,
    pub error: String,
    pub etag: String,
    pub modified: String,
    pub user_agent: String,
    pub referer: String,
    pub expected_sha256: String,
    pub actual_sha256: String,
    pub integrity_status: String,
}
pub struct Job {
    pub id: u64,
    pub details: Mutex<Details>,
    pub status: AtomicUsize,
    pub stop: AtomicBool,
    pub active: AtomicBool,
    pub resume_requested: AtomicBool,
    pub done: AtomicU64,
    pub total: AtomicU64,
    pub speed: AtomicU64,
    pub assembled: AtomicU64,
    pub assembling: AtomicBool,
    pub lanes: AtomicUsize,
    pub added: u64,
    pub notified: AtomicBool,
    pub pause_after: AtomicU64,
    pub pace: Mutex<(Instant, u64)>,
}
impl Job {
    pub fn new(details: Details) -> Arc<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Arc::new(Self {
            id: win::now_filetime() ^ NEXT.fetch_add(1, Ordering::Relaxed),
            details: Mutex::new(details),
            status: AtomicUsize::new(QUEUED),
            stop: AtomicBool::new(false),
            active: AtomicBool::new(false),
            resume_requested: AtomicBool::new(false),
            done: AtomicU64::new(0),
            total: AtomicU64::new(0),
            speed: AtomicU64::new(0),
            assembled: AtomicU64::new(0),
            assembling: AtomicBool::new(false),
            lanes: AtomicUsize::new(0),
            added: win::now_filetime(),
            notified: AtomicBool::new(false),
            pause_after: AtomicU64::new(0),
            pace: Mutex::new((Instant::now(), 0)),
        })
    }
    pub fn snapshot(&self) -> Details {
        self.details
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
    pub fn state(&self) -> usize {
        self.status.load(Ordering::Relaxed)
    }
    pub fn set_error(&self, error: String) {
        self.details.lock().unwrap_or_else(|e| e.into_inner()).error = error;
    }
}
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Network {
    pub proxy: String,
    pub proxy_user: String,
    pub proxy_password: String,
    pub server_origin: String,
    pub server_user: String,
    pub server_password: String,
}
pub struct Engine {
    pub connections: AtomicUsize,
    pub limit_kib: AtomicU64,
    pub max_active: AtomicUsize,
    pub organize: AtomicBool,
    pub hash_downloads: AtomicBool,
    pub network: Mutex<Network>,
    pub http_pool: Mutex<Vec<(String, Arc<crate::http::Connection>)>>,
    pub reservations: Mutex<HashSet<PathBuf>>,
    pub data: PathBuf,
    pub tools_override: Mutex<Option<PathBuf>>,
}
impl Engine {
    pub fn new() -> Arc<Self> {
        let data = std::env::var_os("ODM_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default())
                    .join("OpenDownloadManagerRust")
            });
        let _ = std::fs::create_dir_all(&data);
        Arc::new(Self {
            connections: AtomicUsize::new(8),
            limit_kib: AtomicU64::new(0),
            max_active: AtomicUsize::new(2),
            organize: AtomicBool::new(false),
            hash_downloads: AtomicBool::new(false),
            network: Mutex::new(Network::default()),
            http_pool: Mutex::new(Vec::new()),
            reservations: Mutex::new(HashSet::new()),
            data,
            tools_override: Mutex::new(None),
        })
    }
    pub fn reserve(&self, path: &Path) -> Result<PathBuf, String> {
        let mut names = self.reservations.lock().unwrap();
        for n in 0..10000 {
            let candidate = numbered(path, n);
            if !candidate.exists()
                && !names.contains(&path_key(&candidate))
                && !PathBuf::from(format!("{}.part", candidate.display())).exists()
                && !PathBuf::from(format!("{}.segments", candidate.display())).exists()
            {
                names.insert(path_key(&candidate));
                return Ok(candidate);
            }
        }
        Err("No unused filename is available. Choose a different save name.".into())
    }
    pub fn pace(&self, j: &Job, bytes: u64) {
        let limit = self.limit_kib.load(Ordering::Relaxed);
        if limit == 0 {
            return;
        }
        let deadline = {
            let mut next = j.pace.lock().unwrap();
            if next.1 != limit {
                *next = (Instant::now(), limit);
            }
            next.0 = next.0.max(Instant::now())
                + std::time::Duration::from_secs_f64(bytes as f64 / (limit as f64 * 1024.));
            next.0
        };
        while Instant::now() < deadline
            && !j.stopped()
            && self.limit_kib.load(Ordering::Relaxed) == limit
        {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let current = self.limit_kib.load(Ordering::Relaxed);
        if current != limit || j.stopped() {
            let mut next = j.pace.lock().unwrap();
            if next.1 == limit {
                *next = (Instant::now(), current);
            }
        }
    }
}
pub fn path_key(path: &Path) -> PathBuf {
    let mut buffer = vec![0u16; 32768];
    let length = unsafe {
        win::GetFullPathNameW(
            win::wide(path.to_string_lossy()).as_ptr(),
            buffer.len() as u32,
            buffer.as_mut_ptr(),
            win::null_mut(),
        )
    };
    let full = if length > 0 && (length as usize) < buffer.len() {
        win::from_wide(&buffer)
    } else {
        path.to_string_lossy().into_owned()
    };
    PathBuf::from(full.to_lowercase())
}
pub fn numbered(path: &Path, n: usize) -> PathBuf {
    if n == 0 {
        return path.into();
    }
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let suffix = path
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    path.with_file_name(format!("{stem} ({n}){suffix}"))
}
pub fn valid_url(value: &str) -> Result<url::Url, String> {
    if value.len() > 16384 || value.chars().any(|c| c == '\0' || c == '\r' || c == '\n') {
        return Err("Invalid URL".into());
    }
    let u = url::Url::parse(value).map_err(|_| "Invalid URL")?;
    if !["http", "https", "ftp", "ftps"].contains(&u.scheme()) || u.host_str().is_none() {
        return Err("Use an HTTP, HTTPS, FTP or FTPS URL".into());
    }
    Ok(u)
}
pub fn video_url(value: &str) -> bool {
    if let Ok(u) = url::Url::parse(value) {
        let h = u.host_str().unwrap_or("").to_ascii_lowercase();
        let p = u.path().to_ascii_lowercase();
        h == "youtu.be"
            || h == "youtube.com"
            || h.ends_with(".youtube.com")
            || p.ends_with(".m3u8")
            || p.ends_with(".mpd")
    } else {
        false
    }
}
pub fn default_name(value: &str) -> String {
    let mut name = url::Url::parse(value)
        .ok()
        .and_then(|u| {
            u.path_segments()
                .and_then(|mut s| s.next_back())
                .map(str::to_owned)
        })
        .unwrap_or_default();
    name = name
        .chars()
        .map(|c| {
            if c < ' ' || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(150)
        .collect();
    name = name.trim_end_matches(['.', ' ']).to_owned();
    if name.is_empty() {
        name = "download.bin".into()
    }
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit())
    {
        name.insert(0, '_')
    }
    name
}
pub fn file_type(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    for (kind, extensions) in [
        ("Video", "mp4 mkv webm avi mov ts 3gp flv m4v mpg mpeg wmv"),
        ("Audio", "mp3 m4a aac flac wav ogg opus wma aiff"),
        (
            "Document",
            "pdf doc docx xls xlsx ppt pptx txt csv rtf epub",
        ),
        ("Archive", "zip rar 7z gz tar bz2 xz iso zst"),
        ("Image", "png jpg jpeg gif svg webp bmp heic tiff"),
        ("Application", "exe msi msix apk appx"),
    ] {
        if extensions.split(' ').any(|s| s == ext) {
            return kind;
        }
    }
    "Other"
}
