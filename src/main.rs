#![windows_subsystem = "windows"]
mod bridge;
mod http;
mod model;
mod paint;
mod process;
mod runtime;
mod store;
mod transfer;
mod ui;
mod video;
mod win;
use model::*;
use std::{path::PathBuf, sync::atomic::Ordering};
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let engine = Engine::new();
    if let Err(e) = store::load_network(&engine) {
        unsafe {
            win::MessageBoxW(
                win::null_mut(),
                win::wide(e).as_ptr(),
                win::wide("Open Download Manager").as_ptr(),
                win::MB_ICONERROR,
            );
        }
        std::process::exit(1)
    }
    if args.iter().any(|s| {
        s == "--native-host"
            || s.starts_with("chrome-extension://")
            || s == "opendownloadmanager@local"
    }) {
        std::process::exit(bridge::native_host(engine))
    }
    if args.get(1).is_some_and(|s| s == "--download") && args.len() >= 4 {
        let path = PathBuf::from(&args[3]);
        let path = if path.is_absolute() {
            path
        } else {
            std::env::current_dir().unwrap_or_default().join(path)
        };
        let report = PathBuf::from(format!("{}.result.txt", path.display()));
        let j = Job::new(Details {
            url: args[2].clone(),
            path,
            ..Default::default()
        });
        for pair in args[4..].chunks(2) {
            if pair.len() != 2 {
                continue;
            }
            match pair[0].as_str() {
                "--etag" => j.details.lock().unwrap().etag = pair[1].clone(),
                "--pause-after" => j
                    .pause_after
                    .store(pair[1].parse().unwrap_or(0), Ordering::Relaxed),
                "--connections" => engine.connections.store(
                    pair[1].parse::<usize>().unwrap_or(8).clamp(1, 16),
                    Ordering::Relaxed,
                ),
                "--limit" => engine
                    .limit_kib
                    .store(pair[1].parse().unwrap_or(0), Ordering::Relaxed),
                "--tools-dir" => {
                    *engine.tools_override.lock().unwrap() = Some(pair[1].clone().into())
                }
                _ => (),
            }
        }
        transfer::run(engine, j.clone());
        let d = j.snapshot();
        let _ = std::fs::write(
            report,
            format!(
                "{}\n{}\nPath: {}\nConnections: {}",
                NAMES[j.state()],
                d.error,
                d.path.display(),
                j.lanes.load(Ordering::Relaxed)
            ),
        );
        std::process::exit(if j.state() == COMPLETE {
            0
        } else if j.state() == PAUSED {
            2
        } else {
            1
        })
    }
    ui::run(engine, args.iter().any(|s| s == "--background"));
}
#[cfg(test)]
mod tests;
mod workflow;
