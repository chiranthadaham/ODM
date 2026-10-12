use std::{os::windows::process::CommandExt, process::Command};

pub fn check() -> Result<(), String> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let folder = std::env::temp_dir().join(format!("odm-update-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let script = folder.join("update.ps1");
    std::fs::write(&script, include_str!("../updater/update.ps1")).map_err(|e| e.to_string())?;
    let powershell = std::path::PathBuf::from(
        std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()),
    )
    .join("System32/WindowsPowerShell/v1.0/powershell.exe");
    Command::new(powershell)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&script)
        .args(["-CurrentVersion", env!("CARGO_PKG_VERSION")])
        .creation_flags(0x08000000)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
