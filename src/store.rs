use crate::{model::*, win::*};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::Path,
    sync::{atomic::Ordering, mpsc, Arc},
};
#[derive(Serialize, Deserialize)]
pub struct Row {
    details: Details,
    state: usize,
    done: u64,
    total: u64,
    added: u64,
    id: u64,
}
#[derive(Serialize, Deserialize)]
pub struct Queue {
    format: u32,
    connections: usize,
    limit_kib: u64,
    max_active: usize,
    #[serde(default)]
    organize: bool,
    #[serde(default)]
    hash_downloads: bool,
    jobs: Vec<Row>,
}
pub struct Store {
    sender: Option<mpsc::Sender<Queue>>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Store {
    pub fn new(engine: &Engine) -> Self {
        let path = engine.data.join("queue.json");
        let (tx, rx) = mpsc::channel::<Queue>();
        let thread = std::thread::spawn(move || {
            while let Ok(mut queue) = rx.recv() {
                while let Ok(new) = rx.try_recv() {
                    queue = new
                }
                let result = (|| {
                    let data = serde_json::to_vec(&queue).map_err(|e| e.to_string())?;
                    let tmp = path.with_extension("tmp");
                    use std::io::Write;
                    let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
                    file.write_all(&data).map_err(|e| e.to_string())?;
                    file.sync_all().map_err(|e| e.to_string())?;
                    drop(file);
                    if read_queue(&path).is_ok() {
                        let backup_tmp = path.with_extension("backup.tmp");
                        std::fs::copy(&path, &backup_tmp).map_err(|e| e.to_string())?;
                        std::fs::OpenOptions::new()
                            .write(true)
                            .open(&backup_tmp)
                            .and_then(|f| f.sync_all())
                            .map_err(|e| e.to_string())?;
                        move_replace(&backup_tmp, &path.with_extension("backup.json"))?;
                    }
                    move_replace(&tmp, &path)
                })();
                if let Err(e) = result {
                    let _ = std::fs::write(path.with_extension("save-error.txt"), e);
                }
            }
        });
        Self {
            sender: Some(tx),
            thread: Some(thread),
        }
    }
    pub fn save(&self, engine: &Engine, jobs: &[Arc<Job>]) {
        let jobs = jobs
            .iter()
            .map(|j| Row {
                details: j.snapshot(),
                state: if j.state() == DOWNLOADING {
                    PAUSED
                } else {
                    j.state()
                },
                done: j.done.load(Ordering::Relaxed),
                total: j.total.load(Ordering::Relaxed),
                added: j.added,
                id: j.id,
            })
            .collect();
        let queue = Queue {
            format: 1,
            connections: engine.connections.load(Ordering::Relaxed),
            limit_kib: engine.limit_kib.load(Ordering::Relaxed),
            max_active: engine.max_active.load(Ordering::Relaxed),
            organize: engine.organize.load(Ordering::Relaxed),
            hash_downloads: engine.hash_downloads.load(Ordering::Relaxed),
            jobs,
        };
        if let Some(tx) = &self.sender {
            let _ = tx.send(queue);
        }
    }
}
impl Drop for Store {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn restore(row: Row) -> Arc<Job> {
    let mut j = Job::new(row.details);
    let job = Arc::get_mut(&mut j).unwrap();
    job.id = row.id;
    job.added = row.added;
    job.status.store(
        if row.state == QUEUED || row.state == DOWNLOADING || row.state == 6 {
            PAUSED
        } else {
            row.state.min(CANCELED)
        },
        Ordering::Relaxed,
    );
    job.done.store(row.done, Ordering::Relaxed);
    job.total.store(row.total, Ordering::Relaxed);
    job.notified.store(true, Ordering::Relaxed);
    if job.state() == PAUSED && job.snapshot().error.is_empty() {
        job.set_error("Saved download recovered. Resume to continue; if the server cannot safely resume, ODM starts this file again.".into());
    }
    j
}
fn read_queue(path: &Path) -> Result<Queue, String> {
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Queue is too large".into());
    }
    let queue: Queue = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if queue.format != 1 || queue.jobs.len() > 10000 {
        return Err("Invalid queue format".into());
    }
    Ok(queue)
}
pub fn load(engine: &Engine) -> Result<Vec<Arc<Job>>, String> {
    let path = engine.data.join("queue.json");
    let backup = path.with_extension("backup.json");
    let pending = path.with_extension("tmp");
    if path.exists() || backup.exists() || pending.exists() {
        let (queue,recovered)=match read_queue(&path){
            Ok(q)=>(q,false),
            Err(primary)=> match read_queue(&pending).or_else(|_|read_queue(&backup)) {
                Ok(q)=>(q,true),Err(_)=>return Err(format!("Saved queue could not be read; backup recovery also failed: {primary}. The files have been left untouched."))
            }
        };
        engine
            .connections
            .store(queue.connections.clamp(1, 16), Ordering::Relaxed);
        engine.limit_kib.store(queue.limit_kib, Ordering::Relaxed);
        engine
            .max_active
            .store(queue.max_active.clamp(1, 8), Ordering::Relaxed);
        engine.organize.store(queue.organize, Ordering::Relaxed);
        engine
            .hash_downloads
            .store(queue.hash_downloads, Ordering::Relaxed);
        let jobs: Vec<_> = queue.jobs.into_iter().map(restore).collect();
        if recovered {
            let _=std::fs::write(engine.data.join("recovery-notice.txt"),"The saved queue was recovered from a valid temporary file or backup. Incomplete downloads remain paused.");
            for job in &jobs {
                if job.state() != COMPLETE {
                    job.set_error("Queue recovered from backup. Resume to check the server and continue safely.".into());
                }
            }
        }
        return Ok(jobs);
    }
    let custom = engine.data.join("queue.dat");
    if custom.exists() {
        read_legacy(engine, &custom)
    } else {
        Ok(Vec::new())
    }
}
fn read_legacy(engine: &Engine, path: &Path) -> Result<Vec<Arc<Job>>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let mut input = std::io::Cursor::new(bytes);
    let mut magic = [0; 8];
    input.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if ![b"ODMCPP01", b"ODMCPP02", b"ODMCPP03"].contains(&&magic) {
        return Err("Invalid legacy queue".into());
    }
    fn number<const N: usize>(r: &mut impl Read) -> Result<[u8; N], String> {
        let mut b = [0; N];
        r.read_exact(&mut b).map_err(|e| e.to_string())?;
        Ok(b)
    }
    fn string(r: &mut impl Read) -> Result<String, String> {
        let n = u32::from_le_bytes(number(r)?) as usize;
        if n > 1024 * 1024 {
            return Err("Invalid queue string".into());
        }
        let mut b = vec![0; n];
        r.read_exact(&mut b).map_err(|e| e.to_string())?;
        String::from_utf8(b).map_err(|e| e.to_string())
    }
    let count = u32::from_le_bytes(number(&mut input)?) as usize;
    if count > 10000 {
        return Err("Queue is too large".into());
    }
    engine.max_active.store(
        i32::from_le_bytes(number(&mut input)?).clamp(1, 8) as usize,
        Ordering::Relaxed,
    );
    engine.limit_kib.store(
        i32::from_le_bytes(number(&mut input)?).max(0) as u64,
        Ordering::Relaxed,
    );
    if &magic != b"ODMCPP01" {
        engine.connections.store(
            i32::from_le_bytes(number(&mut input)?).clamp(1, 16) as usize,
            Ordering::Relaxed,
        )
    }
    let mut jobs = Vec::new();
    for _ in 0..count {
        let url = string(&mut input)?;
        let path = string(&mut input)?;
        let _schedule = string(&mut input)?;
        let etag = string(&mut input)?;
        let modified = string(&mut input)?;
        let state = i32::from_le_bytes(number(&mut input)?) as usize;
        let done = u64::from_le_bytes(number(&mut input)?);
        let total = u64::from_le_bytes(number(&mut input)?);
        let added = if &magic == b"ODMCPP03" {
            u64::from_le_bytes(number(&mut input)?)
        } else {
            0
        };
        let job = Job::new(Details {
            url,
            path: path.into(),
            etag,
            modified,
            ..Default::default()
        });
        jobs.push(restore(Row {
            details: job.snapshot(),
            id: job.id,
            state,
            done,
            total,
            added,
        }));
    }
    Ok(jobs)
}
pub fn validate_network(n: &Network) -> Result<(), String> {
    for value in [
        &n.proxy,
        &n.proxy_user,
        &n.proxy_password,
        &n.server_origin,
        &n.server_user,
        &n.server_password,
    ] {
        if value.len() > 8192 || value.chars().any(|c| c == '\0' || c == '\r' || c == '\n') {
            return Err("Invalid connection setting".into());
        }
    }
    if !n.proxy.is_empty() {
        let u = valid_url(&n.proxy)
            .or_else(|_| url::Url::parse(&n.proxy).map_err(|e| e.to_string()))?;
        if !["http", "https", "socks4", "socks4a", "socks5", "socks5h"].contains(&u.scheme())
            || !u.username().is_empty()
            || u.password().is_some()
        {
            return Err("Enter proxy credentials separately".into());
        }
    }
    if !n.server_user.is_empty() {
        let origin = valid_url(&n.server_origin)?.origin().ascii_serialization();
        if origin != n.server_origin {
            return Err("Enter the exact server origin, for example https://example.com".into());
        }
    }
    Ok(())
}
pub fn save_network(engine: &Engine, n: Network) -> Result<(), String> {
    validate_network(&n)?;
    let mut data = serde_json::to_vec(&n).map_err(|e| e.to_string())?;
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_mut_ptr(),
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        if CryptProtectData(
            &input,
            wide("Open Download Manager connection settings").as_ptr(),
            null(),
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        ) == 0
        {
            return Err(error("Protect settings"));
        }
        let encrypted = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData.cast());
        data.fill(0);
        let path = engine.data.join("network.dat");
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, encrypted).map_err(|e| e.to_string())?;
        move_replace(&tmp, &path)?;
    }
    *engine.network.lock().unwrap() = n;
    Ok(())
}
pub fn load_network(engine: &Engine) -> Result<(), String> {
    let mut path = engine.data.join("network.dat");
    if !path.exists() && std::env::var_os("ODM_DATA_DIR").is_none() {
        path = Path::new(&std::env::var_os("LOCALAPPDATA").unwrap_or_default())
            .join("OpenDownloadManagerCpp/network.dat")
    }
    if !path.exists() {
        return Ok(());
    }
    let mut data = std::fs::read(path).map_err(|e| e.to_string())?;
    if data.len() > 65536 {
        return Err("Invalid protected settings".into());
    }
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: data.len() as u32,
            pbData: data.as_mut_ptr(),
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        if CryptUnprotectData(
            &input,
            null_mut(),
            null(),
            null(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        ) == 0
        {
            return Err(error("Read protected settings"));
        }
        let mut plain = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        std::ptr::write_bytes(output.pbData, 0, output.cbData as usize);
        LocalFree(output.pbData.cast());
        let n: Network = serde_json::from_slice(&plain).map_err(|e| e.to_string())?;
        plain.fill(0);
        validate_network(&n)?;
        *engine.network.lock().unwrap() = n;
    }
    Ok(())
}
