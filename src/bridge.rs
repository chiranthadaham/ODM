use crate::{http, model::*, win::*};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    os::windows::process::CommandExt,
    sync::Arc,
    time::Duration,
};
pub const CLASS: &str = "OpenDownloadManagerRust";
fn handoff(engine: Arc<Engine>, mut request: Value) -> Result<Value, String> {
    let address = request["url"].as_str().ok_or("Missing URL")?;
    let url = valid_url(address)?;
    if !["http", "https"].contains(&url.scheme()) {
        return Err("Only HTTP and HTTPS browser links are supported".into());
    }
    for key in ["userAgent", "referer"] {
        let value = request[key].as_str().unwrap_or("");
        if value.len() > 8192 || value.chars().any(|c| c == '\0' || c == '\r' || c == '\n') {
            return Err("Invalid browser request context".into());
        }
    }
    let checking = request["verifyAccess"].as_bool().unwrap_or(false) && !video_url(address);
    let (address, agent, referer) = (
        address.to_owned(),
        request["userAgent"].as_str().unwrap_or("").to_owned(),
        request["referer"].as_str().unwrap_or("").to_owned(),
    );
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = if checking {
            http::check_access(&engine, &address, &agent, &referer)
        } else {
            Ok(())
        };
        let _ = tx.send(result);
    });
    unsafe {
        let mut app = FindWindowW(wide(CLASS).as_ptr(), null());
        if app.is_null() {
            std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
                .arg("--background")
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .map_err(|e| e.to_string())?;
            for _ in 0..400 {
                std::thread::sleep(Duration::from_millis(25));
                app = FindWindowW(wide(CLASS).as_ptr(), null());
                if !app.is_null() {
                    break;
                }
            }
        }
        if app.is_null() {
            return Err("The download manager did not start".into());
        }
        let id = GetCurrentProcessId();
        request["promptId"] = json!(id);
        request["checking"] = json!(checking);
        let payload = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
        let data = COPYDATASTRUCT {
            dwData: 0x4f444d33,
            cbData: payload.len() as u32,
            lpData: payload.as_ptr() as *mut c_void,
        };
        let mut accepted = 0;
        if SendMessageTimeoutW(
            app,
            WM_COPYDATA,
            0,
            (&data as *const COPYDATASTRUCT) as isize,
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            4000,
            &mut accepted,
        ) == 0
            || accepted == 0
        {
            return Err(
                "The app could not accept this link; keep downloading in the browser".into(),
            );
        }
        let mut notified = !checking;
        for _ in 0..24000 {
            if !notified {
                if let Ok(result) = rx.try_recv() {
                    let mut ignored = 0;
                    SendMessageTimeoutW(
                        app,
                        WM_APP + 73,
                        id as usize,
                        if result.is_ok() { 1 } else { 2 },
                        SMTO_ABORTIFHUNG | SMTO_BLOCK,
                        1000,
                        &mut ignored,
                    );
                    result?;
                    notified = true;
                }
            }
            if IsWindow(app) == 0 {
                return Err("The app closed before the download started".into());
            }
            let mut decision = 0;
            if SendMessageTimeoutW(
                app,
                WM_APP + 71,
                id as usize,
                0,
                SMTO_ABORTIFHUNG | SMTO_BLOCK,
                1000,
                &mut decision,
            ) != 0
                && decision != 0
            {
                return if decision == 1 {
                    Ok(json!({"ok":true,"queued":true}))
                } else {
                    Ok(json!({"ok":false,"canceled":true,"error":"Download canceled"}))
                };
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Err("Download prompt timed out".into())
    }
}
pub fn native_host(engine: Arc<Engine>) -> i32 {
    let (mut input, mut output) = (std::io::stdin().lock(), std::io::stdout().lock());
    loop {
        let mut prefix = [0u8; 4];
        if input.read_exact(&mut prefix).is_err() {
            return 0;
        }
        let size = u32::from_le_bytes(prefix) as usize;
        if size == 0 || size > 65536 {
            return 1;
        }
        let mut data = vec![0u8; size];
        if input.read_exact(&mut data).is_err() {
            return 1;
        }
        let result = (|| {
            let request: Value = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
            match request["action"].as_str() {
                Some("ping") => Ok(json!({"ok":true,"version":"0.2.0-rust"})),
                Some("download") => handoff(engine.clone(), request),
                _ => Err("Unknown native message action".into()),
            }
        })();
        let reply = result.unwrap_or_else(|e: String| json!({"ok":false,"error":e}));
        let bytes = serde_json::to_vec(&reply).unwrap_or_default();
        if output
            .write_all(&(bytes.len() as u32).to_le_bytes())
            .and_then(|_| output.write_all(&bytes))
            .and_then(|_| output.flush())
            .is_err()
        {
            return 1;
        }
    }
}
