use crate::model::*;
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    sync::Arc,
};
pub const CLASS: &str = "OpenDownloadManagerRust";
fn handoff(engine: Arc<Engine>, request: Value) -> Result<Value, String> {
    crate::ipc::send(&engine, request)
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
                Some("ping") => Ok(json!({"ok":true,"version":"0.2.9-rust"})),
                Some("ping-app") => crate::ipc::send(&engine, json!({"action":"ping"})),
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
