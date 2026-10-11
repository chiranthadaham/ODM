use crate::{model::Engine, win::*};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
    sync::OnceLock,
};
use windows_sys::Win32::Devices::DeviceAndDriverInstallation::*;
static CAB: &[u8] = include_bytes!("../assets/video-tools.cab");
static INDEX: &str = include_str!("../assets/video-tools.index");
pub fn sha(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
struct Context {
    folder: PathBuf,
    expected: BTreeMap<String, String>,
    error: Option<String>,
}
unsafe extern "system" fn callback(
    context: *const c_void,
    event: u32,
    one: usize,
    _two: usize,
) -> u32 {
    let c = &mut *(context as *mut Context);
    if event == SPFILENOTIFY_FILEINCABINET {
        let file = &mut *(one as *mut FILE_IN_CABINET_INFO_W);
        let mut length = 0;
        while length < 260 && *file.NameInCabinet.add(length) != 0 {
            length += 1
        }
        let name = String::from_utf16_lossy(std::slice::from_raw_parts(file.NameInCabinet, length));
        if !c.expected.contains_key(&name) || name.contains(['/', '\\', ':']) {
            c.error = Some("Unsafe video runtime entry".into());
            return FILEOP_ABORT;
        }
        let path = wide(c.folder.join(name).to_string_lossy());
        if path.len() > file.FullTargetName.len() {
            c.error = Some("Runtime cache path is too long".into());
            return FILEOP_ABORT;
        }
        file.FullTargetName[..path.len()].copy_from_slice(&path);
        return FILEOP_DOIT;
    }
    if event == SPFILENOTIFY_FILEEXTRACTED {
        let file = &*(one as *const FILEPATHS_W);
        if file.Win32Error != 0 {
            c.error = Some(format!("Runtime extraction error {}", file.Win32Error));
            return file.Win32Error;
        }
    }
    if event == SPFILENOTIFY_NEEDNEWCABINET {
        c.error = Some("Unexpected cabinet continuation".into());
        return ERROR_INVALID_DATA;
    }
    0
}
pub fn tools(engine: &Engine) -> Result<PathBuf, String> {
    if let Some(path) = engine.tools_override.lock().unwrap().clone() {
        return Ok(path);
    }
    static CACHED: OnceLock<PathBuf> = OnceLock::new();
    if let Some(path) = CACHED.get() {
        return Ok(path.clone());
    }
    let mut lines = INDEX.lines();
    let version = lines.next().unwrap_or("").trim();
    if version.len() != 20 || !version.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid video runtime index".into());
    }
    let mut expected = BTreeMap::new();
    for line in lines {
        let mut fields = line.split_whitespace();
        let digest = fields.next().unwrap_or("");
        let name = fields.next().unwrap_or("");
        if digest.len() != 64 || name.is_empty() || name.contains(['/', '\\', ':']) {
            return Err("Invalid runtime manifest".into());
        }
        expected.insert(name.to_owned(), digest.to_owned());
    }
    let lock = Handle(unsafe {
        CreateMutexW(
            null(),
            0,
            wide(format!("Local\\OpenDownloadManagerRuntime-{version}")).as_ptr(),
        )
    });
    if lock.0.is_null() {
        return Err(error("Lock video runtime"));
    }
    let waited = unsafe { WaitForSingleObject(lock.0, 120000) };
    if waited != WAIT_OBJECT_0 && waited != WAIT_ABANDONED {
        return Err("Video runtime is busy; retry shortly".into());
    }
    struct Unlock(HANDLE);
    impl Drop for Unlock {
        fn drop(&mut self) {
            unsafe {
                ReleaseMutex(self.0);
            }
        }
    }
    let _unlock = Unlock(lock.0);
    let folder = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default())
        .join("OpenDownloadManagerCpp")
        .join(format!("tools-{version}"));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let valid = |files: &BTreeMap<String, String>| {
        files
            .iter()
            .all(|(name, digest)| sha(&folder.join(name)).ok().as_ref() == Some(digest))
    };
    if !valid(&expected) {
        let archive = folder.join("runtime.cab");
        std::fs::write(&archive, CAB).map_err(|e| e.to_string())?;
        let mut context = Context {
            folder: folder.clone(),
            expected,
            error: None,
        };
        let result = unsafe {
            SetupIterateCabinetW(
                wide(archive.to_string_lossy()).as_ptr(),
                0,
                Some(callback),
                (&mut context as *mut Context).cast(),
            )
        };
        let _ = std::fs::remove_file(archive);
        if result == 0 {
            return Err(context
                .error
                .unwrap_or_else(|| error("Unpack video runtime")));
        }
        if !valid(&context.expected) {
            return Err("Video runtime checksum verification failed".into());
        }
    }
    let _ = CACHED.set(folder.clone());
    Ok(folder)
}
