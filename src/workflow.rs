use crate::{model::*, paint, store, ui::*, win::*};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{atomic::Ordering as O, Arc},
    time::Instant,
};
struct Popup {
    app: *mut App,
    kind: Kind,
    h: HWND,
    last: Instant,
    rate: f64,
    observed: u64,
}
pub enum Kind {
    New,
    Start(Value),
    Progress(Arc<Job>),
    Complete(Arc<Job>),
    Settings,
    BrowserPlugins,
    Network,
    Renew(Arc<Job>),
}
pub unsafe fn is_start(h: HWND) -> bool {
    let p = GetWindowLongPtrW(h, GWLP_USERDATA) as *mut Popup;
    !p.is_null() && matches!((*p).kind, Kind::Start(_))
}
pub unsafe fn verified(h: HWND, id: usize, ok: bool) {
    let p = GetWindowLongPtrW(h, GWLP_USERDATA) as *mut Popup;
    if p.is_null() {
        return;
    }
    if let Kind::Start(v) = &mut (*p).kind {
        if v["promptId"].as_u64() == Some(id as u64) {
            v["checking"] = Value::Bool(false);
            v["refused"] = Value::Bool(!ok);
            set_text(
                GetDlgItem(h, 206),
                if ok {
                    "Ready to download"
                } else {
                    "Website refused access. Keep this download in the browser."
                },
            );
            EnableWindow(GetDlgItem(h, 203), ok as i32);
            if ok && v["startRequested"].as_bool().unwrap_or(false) {
                PostMessageW(h, WM_COMMAND, 203, 0);
            }
        }
    }
}
pub unsafe fn open(a: &mut App, kind: Kind) {
    if let Kind::Progress(job) = &kind {
        for &h in &a.dialogs {
            let p = GetWindowLongPtrW(h, GWLP_USERDATA) as *mut Popup;
            if !p.is_null() {
                if let Kind::Progress(existing) = &(*p).kind {
                    if existing.id == job.id {
                        ShowWindow(h, SW_RESTORE);
                        SetForegroundWindow(h);
                        return;
                    }
                }
            }
        }
    }
    let (title, w, h) = match &kind {
        Kind::New => ("New download", 700, 285),
        Kind::Start(_) => ("New download — Save and Start", 690, 270),
        Kind::Progress(_) => ("Download progress", 650, 275),
        Kind::Complete(_) => ("Download complete", 640, 205),
        Kind::Settings => ("Settings", 510, 310),
        Kind::BrowserPlugins => ("Browser plug-in", 560, 245),
        Kind::Network => ("Connection settings", 700, 570),
        Kind::Renew(_) => ("Renew download link", 700, 240),
    };
    let p = Box::new(Popup {
        app: a,
        kind,
        h: null_mut(),
        last: Instant::now(),
        rate: 0.,
        observed: 0,
    });
    let p = Box::into_raw(p);
    let window = CreateWindowExW(
        WS_EX_APPWINDOW | WS_EX_CONTROLPARENT,
        wide("ODMWorkflowRust").as_ptr(),
        wide(title).as_ptr(),
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN,
        (GetSystemMetrics(SM_CXSCREEN) - w) / 2,
        (GetSystemMetrics(SM_CYSCREEN) - h) / 2,
        w,
        h,
        null_mut(),
        null_mut(),
        GetModuleHandleW(null()),
        p.cast(),
    );
    if window.is_null() {
        message(a.h, &error("Open popup"));
        return;
    }
    a.dialogs.push(window);
    ShowWindow(window, SW_SHOW);
    SetForegroundWindow(window);
}
pub unsafe extern "system" fn proc(h: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    if msg == WM_NCCREATE {
        let p = (*(lp as *const CREATESTRUCTW)).lpCreateParams as *mut Popup;
        (*p).h = h;
        SetWindowLongPtrW(h, GWLP_USERDATA, p as isize);
    }
    let ptr = GetWindowLongPtrW(h, GWLP_USERDATA) as *mut Popup;
    if ptr.is_null() {
        return DefWindowProcW(h, msg, wp, lp);
    }
    let p = &mut *ptr;
    let a = &mut *p.app;
    match msg {
        WM_CREATE => {
            paint::modern(h);
            let f = a.font;
            match &p.kind {
                Kind::BrowserPlugins => {
                    label(h, "Choose your browser", 700, 24, 20, 480, 28, f);
                    label(
                        h,
                        "Open its official extension store to install the plug-in.",
                        703,
                        24,
                        56,
                        490,
                        28,
                        f,
                    );
                    button(h, "Chrome", 701, 24, 100, 240, f);
                    button(h, "Firefox", 702, 280, 100, 240, f);
                    label(
                        h,
                        "Until published, these buttons open a store search.",
                        704,
                        24,
                        150,
                        490,
                        28,
                        a.small,
                    );
                }
                Kind::New => {
                    label(h, "Link", 610, 24, 18, 600, 24, f);
                    edit(h, "", 600, 24, 44, 566, f, 0);
                    button(h, "Paste", 602, 600, 44, 60, f);
                    label(h, "Save to", 611, 24, 96, 600, 24, f);
                    edit(h, &a.folder.to_string_lossy(), 601, 24, 122, 566, f, 0);
                    button(h, "Browse", 603, 600, 122, 60, f);
                    button(h, "Cancel", 2, 420, 186, 100, f);
                    button(h, "Download", 604, 530, 186, 130, f);
                }
                Kind::Start(v) => {
                    let u = v["url"].as_str().unwrap_or("");
                    label(h, "Download link", 200, 20, 18, 640, 24, f);
                    edit(h, u, 204, 20, 44, 640, f, ES_READONLY as u32);
                    label(h, "Save file as", 205, 20, 85, 640, 24, f);
                    let name = v["filename"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .map(|s| {
                            default_name(&format!("https://local/{}", s.replace(['\\', '/'], "_")))
                        })
                        .unwrap_or_else(|| default_name(u));
                    edit(
                        h,
                        &a.folder.join(name).to_string_lossy(),
                        201,
                        20,
                        111,
                        525,
                        f,
                        0,
                    );
                    button(h, "Browse", 202, 555, 111, 105, f);
                    label(
                        h,
                        if v["checking"].as_bool().unwrap_or(false) {
                            "Checking website access… You can choose where to save now."
                        } else {
                            "Ready to download"
                        },
                        206,
                        20,
                        151,
                        640,
                        24,
                        a.small,
                    );
                    button(h, "Start download", 203, 390, 185, 150, f);
                    button(h, "Cancel", 2, 550, 185, 110, f);
                }
                Kind::Progress(j) => {
                    label(
                        h,
                        &j.snapshot()
                            .path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy(),
                        501,
                        22,
                        20,
                        590,
                        26,
                        a.bold,
                    );
                    label(h, "Starting download…", 502, 22, 58, 590, 38, f);
                    label(h, "", 503, 22, 148, 590, 26, a.small);
                    button(h, "Pause", 504, 212, 190, 120, f);
                    button(h, "Cancel", 505, 342, 190, 145, f);
                    button(h, "Hide", 506, 497, 190, 115, f);
                    SetTimer(h, 1, 250, None);
                }
                Kind::Complete(j) => {
                    label(h, "Download complete", 400, 20, 20, 590, 26, a.bold);
                    edit(
                        h,
                        &j.snapshot().path.to_string_lossy(),
                        404,
                        20,
                        58,
                        590,
                        f,
                        ES_READONLY as u32,
                    );
                    button(h, "Open", 401, 210, 115, 120, f);
                    button(h, "Open folder", 402, 340, 115, 130, f);
                    button(h, "Close", 2, 480, 115, 130, f);
                }
                Kind::Settings => {
                    for (i, (s, v)) in [
                        (
                            "Maximum simultaneous downloads",
                            a.engine.max_active.load(O::Relaxed) as u64,
                        ),
                        (
                            "Connections per download",
                            a.engine.connections.load(O::Relaxed) as u64,
                        ),
                        (
                            "Speed limit per download (KiB/s, 0 = unlimited)",
                            a.engine.limit_kib.load(O::Relaxed),
                        ),
                    ]
                    .iter()
                    .enumerate()
                    {
                        label(
                            h,
                            s,
                            320 + i as i32,
                            20,
                            25 + i as i32 * 49,
                            340,
                            25,
                            a.small,
                        );
                        edit(
                            h,
                            &v.to_string(),
                            310 + i as i32,
                            370,
                            22 + i as i32 * 49,
                            100,
                            f,
                            ES_NUMBER as u32,
                        );
                    }
                    label(h,"Changes apply to active downloads. Higher connection counts depend on the server's limits.",323,20,178,450,42,a.small);
                    button(h, "Connections…", 315, 20, 231, 175, f);
                    button(h, "Save", 314, 220, 231, 135, f);
                    button(h, "Cancel", 2, 365, 231, 105, f);
                }
                Kind::Network => {
                    let n = a.engine.network.lock().unwrap().clone();
                    for (i, (s, v)) in [
                        "Proxy address",
                        "Proxy username",
                        "Proxy password",
                        "Server origin",
                        "Server username",
                        "Server password",
                    ]
                    .iter()
                    .zip([
                        n.proxy,
                        n.proxy_user,
                        n.proxy_password,
                        n.server_origin,
                        n.server_user,
                        n.server_password,
                    ])
                    .enumerate()
                    {
                        label(h, s, 350 + i as i32, 24, 30 + i as i32 * 58, 146, 24, f);
                        edit(
                            h,
                            &v,
                            330 + i as i32,
                            180,
                            24 + i as i32 * 58,
                            470,
                            f,
                            if i == 2 || i == 5 {
                                ES_PASSWORD as u32
                            } else {
                                0
                            },
                        );
                    }
                    label(h,"Proxy: http://host:port or socks5://host:port.\r\nCredentials are encrypted for this Windows user.\r\nServer credentials apply only to the specified origin.",356,24,382,630,90,a.small);
                    button(h, "Save", 338, 420, 486, 110, f);
                    button(h, "Cancel", 2, 540, 486, 110, f);
                }
                Kind::Renew(j) => {
                    label(h, "New download link", 360, 24, 20, 620, 24, f);
                    edit(h, &j.snapshot().url, 361, 24, 52, 630, f, 0);
                    label(
                        h,
                        "Partial data is reused only when the server's file validator matches.",
                        362,
                        24,
                        104,
                        630,
                        26,
                        a.small,
                    );
                    button(h, "Renew download", 363, 370, 150, 170, f);
                    button(h, "Cancel", 2, 550, 150, 105, f);
                }
            }
            0
        }
        WM_COMMAND => {
            let id = wp as u32 & 65535;
            if id == 2 || id == 506 {
                PostMessageW(h, WM_CLOSE, 0, 0);
                return 0;
            }
            match id {
                701 | 702 => {
                    let chrome = id == 701;
                    let key = if chrome {
                        "chrome_store_url"
                    } else {
                        "firefox_store_url"
                    };
                    let fallback = if chrome {
                        "https://chromewebstore.google.com/search/Open%20Download%20Manager"
                    } else {
                        "https://addons.mozilla.org/en-US/firefox/search/?q=Open%20Download%20Manager"
                    };
                    let file = std::env::current_exe()
                        .ok()
                        .and_then(|p| p.parent().map(|d| d.join("browser-stores.json")));
                    let config: Value = file
                        .and_then(|p| std::fs::read(p).ok())
                        .and_then(|b| serde_json::from_slice(&b).ok())
                        .unwrap_or(Value::Null);
                    let candidate = config[key].as_str().unwrap_or(fallback);
                    let prefix = if chrome {
                        "https://chromewebstore.google.com/"
                    } else {
                        "https://addons.mozilla.org/"
                    };
                    let url = if candidate.starts_with(prefix) {
                        candidate
                    } else {
                        fallback
                    };
                    open_browser_store(chrome, url);
                }
                602 => {
                    if let Some(s) = clipboard(h) {
                        set_text(GetDlgItem(h, 600), s.trim());
                    }
                }
                603 => {
                    if let Some(f) = folder(h) {
                        set_text(GetDlgItem(h, 601), &f.to_string_lossy());
                    }
                }
                604 => {
                    let u = value(h, 600).trim().to_owned();
                    if let Err(e) = valid_url(&u) {
                        message(h, &e)
                    } else {
                        a.folder = value(h, 601).into();
                        a.choices.push_back(serde_json::json!({"url":u}));
                        PostMessageW(h, WM_CLOSE, 0, 0);
                        PostMessageW(a.h, WM_APP + 74, 0, 0);
                    }
                }
                202 => {
                    if let Some(path) = select_file(h, &value(h, 201), true) {
                        set_text(GetDlgItem(h, 201), &path.to_string_lossy());
                    }
                }
                203 => {
                    if let Kind::Start(v) = &mut p.kind {
                        if v["checking"].as_bool().unwrap_or(false) {
                            v["startRequested"] = Value::Bool(true);
                            set_text(GetDlgItem(h, 203), "Checking…");
                            EnableWindow(GetDlgItem(h, 203), 0);
                        } else if !v["refused"].as_bool().unwrap_or(false) {
                            let path = PathBuf::from(value(h, 201));
                            if !path.is_absolute() || path.file_name().is_none() {
                                message(h, "Choose a full save path.")
                            } else {
                                let v = v.clone();
                                let id = v["promptId"].as_u64().unwrap_or(0) as usize;
                                if let Some(folder) = path.parent() {
                                    a.folder = folder.into();
                                }
                                match a.add(v["url"].as_str().unwrap_or("").into(), path, &v) {
                                    Ok(()) => {
                                        a.decisions.insert(id, 1);
                                        PostMessageW(h, WM_CLOSE, 0, 0);
                                    }
                                    Err(e) => message(h, &e),
                                }
                            }
                        }
                    }
                }
                504 => {
                    if let Kind::Progress(j) = &p.kind {
                        if j.state() == DOWNLOADING || j.state() == QUEUED {
                            j.status.store(PAUSED, O::Relaxed);
                            j.resume_requested.store(false, O::Relaxed);
                            j.stop.store(true, O::Relaxed);
                        } else if j.state() != COMPLETE {
                            if j.active.load(O::Acquire) {
                                j.resume_requested.store(true, O::Release);
                            } else {
                                j.stop.store(false, O::Relaxed);
                                j.status.store(QUEUED, O::Relaxed);
                            }
                        }
                    }
                }
                505 => {
                    if let Kind::Progress(j) = &p.kind {
                        j.status.store(CANCELED, O::Relaxed);
                        j.resume_requested.store(false, O::Relaxed);
                        j.stop.store(true, O::Relaxed);
                    }
                    PostMessageW(h, WM_CLOSE, 0, 0);
                }
                401 | 402 => {
                    if let Kind::Complete(j) = &p.kind {
                        let path = j.snapshot().path;
                        if id == 401 {
                            shell_open(&path)
                        } else if let Some(parent) = path.parent() {
                            shell_open(parent)
                        }
                    }
                }
                315 => open(a, Kind::Network),
                314 => {
                    let values = (
                        value(h, 310).parse::<usize>(),
                        value(h, 311).parse::<usize>(),
                        value(h, 312).parse::<u64>(),
                    );
                    if let (Ok(active), Ok(connections), Ok(limit)) = values {
                        if (1..=8).contains(&active)
                            && (1..=16).contains(&connections)
                            && limit <= 100_000_000
                        {
                            a.engine.max_active.store(active, O::Relaxed);
                            a.engine.connections.store(connections, O::Relaxed);
                            a.engine.limit_kib.store(limit, O::Relaxed);
                            a.save();
                            PostMessageW(h, WM_CLOSE, 0, 0);
                        } else {
                            message(h,"Use 1–8 simultaneous downloads, 1–16 connections, and a nonnegative speed limit.")
                        }
                    } else {
                        message(h, "Enter whole numbers.")
                    }
                }
                338 => {
                    let n = Network {
                        proxy: value(h, 330).trim().into(),
                        proxy_user: value(h, 331),
                        proxy_password: value(h, 332),
                        server_origin: value(h, 333).trim().into(),
                        server_user: value(h, 334),
                        server_password: value(h, 335),
                    };
                    match store::save_network(&a.engine, n) {
                        Ok(()) => {
                            PostMessageW(h, WM_CLOSE, 0, 0);
                        }
                        Err(e) => message(h, &e),
                    }
                }
                363 => {
                    if let Kind::Renew(j) = &p.kind {
                        let u = value(h, 361).trim().to_owned();
                        if let Err(e) = valid_url(&u) {
                            message(h, &e)
                        } else {
                            let mut d = j.details.lock().unwrap();
                            d.url = u;
                            d.user_agent.clear();
                            d.referer.clear();
                            d.error.clear();
                            drop(d);
                            j.stop.store(false, O::Relaxed);
                            j.status.store(QUEUED, O::Relaxed);
                            j.notified.store(false, O::Relaxed);
                            a.save();
                            PostMessageW(h, WM_CLOSE, 0, 0);
                        }
                    }
                }
                _ => (),
            }
            0
        }
        WM_TIMER => {
            if let Kind::Progress(j) = &p.kind {
                if j.state() == COMPLETE {
                    PostMessageW(h, WM_CLOSE, 0, 0);
                    return 0;
                }
                let now = Instant::now();
                let dt = now.duration_since(p.last).as_secs_f64();
                let done = j.done.load(O::Relaxed);
                let total = j.total.load(O::Relaxed);
                let speed = j.speed.load(O::Relaxed);
                if dt >= 1. {
                    if done >= p.observed {
                        let observed = (done - p.observed) as f64 / dt;
                        p.rate = if p.rate == 0. {
                            observed
                        } else {
                            p.rate + (observed - p.rate) * (dt / (4. + dt))
                        };
                    } else {
                        p.rate = 0.
                    }
                    p.observed = done;
                    p.last = now;
                }
                let assembling = j.assembling.load(O::Relaxed);
                let status = if assembling {
                    "Assembling file"
                } else {
                    NAMES[j.state()]
                };
                set_text(
                    GetDlgItem(h, 502),
                    &format!(
                        "{}   {} / {}",
                        status,
                        size(done),
                        if total == 0 {
                            "—".into()
                        } else {
                            size(total)
                        }
                    ),
                );
                let remaining =
                    if p.rate > 0. && total > done && j.state() == DOWNLOADING && !assembling {
                        format!(
                            "   {} remaining",
                            eta(((total - done) as f64 / p.rate).ceil() as u64)
                        )
                    } else {
                        String::new()
                    };
                let info = j.snapshot();
                let detail = if !info.error.is_empty() {
                    info.error
                } else {
                    format!(
                        "{}/s   {} connections{}",
                        size(speed),
                        j.lanes.load(O::Relaxed),
                        remaining
                    )
                };
                set_text(GetDlgItem(h, 503), &detail);
                set_text(
                    GetDlgItem(h, 504),
                    if j.state() == DOWNLOADING || j.state() == QUEUED {
                        "Pause"
                    } else {
                        "Resume"
                    },
                );
                let r = RECT {
                    left: 22,
                    top: 110,
                    right: 613,
                    bottom: 137,
                };
                InvalidateRect(h, &r, 0);
            }
            0
        }
        WM_PAINT | WM_PRINTCLIENT => {
            paint::buffered(
                h,
                if msg == WM_PRINTCLIENT {
                    Some(wp as HDC)
                } else {
                    None
                },
                paint::WHITE,
                |dc| {
                    if let Kind::Progress(j) = &p.kind {
                        let r = RECT {
                            left: 22,
                            top: 110,
                            right: 612,
                            bottom: 136,
                        };
                        paint::rounded(dc, r, 0x00f2e8e2, 0x00f2e8e2, 8.);
                        let total = j.total.load(O::Relaxed);
                        let done = if j.assembling.load(O::Relaxed) {
                            j.assembled.load(O::Relaxed)
                        } else {
                            j.done.load(O::Relaxed)
                        };
                        if total > 0 && done > 0 {
                            let right =
                                22 + ((done.min(total) as u128 * 590) / total as u128) as i32;
                            paint::rounded(
                                dc,
                                RECT {
                                    right: right.max(26),
                                    ..r
                                },
                                paint::BLUE,
                                paint::BLUE,
                                8.,
                            );
                        }
                    }
                },
            );
            0
        }
        WM_ERASEBKGND => 1,
        WM_DRAWITEM => {
            paint::button(&*(lp as *const DRAWITEMSTRUCT), a.font, false);
            1
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT => {
            SetTextColor(wp as HDC, paint::INK);
            SetBkColor(wp as HDC, paint::WHITE);
            GetStockObject(WHITE_BRUSH) as isize
        }
        WM_CLOSE => {
            if let Kind::Start(v) = &p.kind {
                let id = v["promptId"].as_u64().unwrap_or(0) as usize;
                if *a.decisions.get(&id).unwrap_or(&0) == 0 {
                    a.decisions.insert(id, 2);
                }
            }
            DestroyWindow(h);
            0
        }
        WM_NCDESTROY => {
            a.dialogs.retain(|&d| d != h);
            SetWindowLongPtrW(h, GWLP_USERDATA, 0);
            drop(Box::from_raw(ptr));
            DefWindowProcW(h, msg, wp, lp)
        }
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}
