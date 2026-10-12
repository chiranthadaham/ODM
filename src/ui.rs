use crate::{bridge, model::*, paint, store, transfer, win::*};
use serde_json::Value;
const SS_NOPREFIX: u32 = 0x80;
const SS_ENDELLIPSIS: u32 = 0x4000;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::Read,
    path::PathBuf,
    sync::{atomic::Ordering as O, mpsc, Arc},
};
enum FileResult {
    Import(Result<Vec<String>, String>),
    Export(Result<(), String>),
    Integrity(Arc<Job>, String, Result<String, String>),
}
pub struct App {
    pub engine: Arc<Engine>,
    pub jobs: Vec<Arc<Job>>,
    store: store::Store,
    pub h: HWND,
    list: HWND,
    pub font: HFONT,
    pub small: HFONT,
    pub bold: HFONT,
    brush: HBRUSH,
    pub dialogs: Vec<HWND>,
    workers: Vec<std::thread::JoinHandle<()>>,
    rows: Vec<Vec<Vec<u16>>>,
    sort: usize,
    descending: bool,
    pub choices: VecDeque<Value>,
    pub decisions: HashMap<usize, isize>,
    exiting: bool,
    ticks: u64,
    pub folder: PathBuf,
    io_tx: mpsc::Sender<FileResult>,
    io_rx: mpsc::Receiver<FileResult>,
    importing: bool,
    exporting: bool,
    tooltip: HWND,
    tip_text: Vec<Vec<u16>>,
}
unsafe fn draw_cell(a: &App, d: &mut NMLVCUSTOMDRAW) -> isize {
    match d.nmcd.dwDrawStage {
        CDDS_PREPAINT => (CDRF_NOTIFYITEMDRAW | CDRF_NOTIFYPOSTPAINT) as isize,
        CDDS_POSTPAINT => {
            if a.jobs.is_empty() {
                let mut r = RECT::default();
                GetClientRect(a.list, &mut r);
                r.top = (r.bottom / 2 - 30).max(40);
                r.bottom = r.top + 60;
                paint::label(
                    d.nmcd.hdc,
                    "No downloads yet\nClick + to add a download.",
                    r,
                    a.font,
                    paint::muted(),
                    DT_CENTER,
                );
            }
            CDRF_DODEFAULT as isize
        }
        CDDS_ITEMPREPAINT => CDRF_NOTIFYSUBITEMDRAW as isize,
        stage if stage == CDDS_ITEMPREPAINT | CDDS_SUBITEM => {
            let row = d.nmcd.dwItemSpec;
            let col = d.iSubItem as usize;
            let Some(j) = a.jobs.get(row) else {
                return CDRF_DODEFAULT as isize;
            };
            let mut r = RECT {
                left: LVIR_BOUNDS as i32,
                top: col as i32,
                ..Default::default()
            };
            if SendMessageW(
                a.list,
                LVM_GETSUBITEMRECT,
                row,
                (&mut r as *mut RECT) as isize,
            ) == 0
            {
                return CDRF_DODEFAULT as isize;
            }
            let selected = SendMessageW(a.list, LVM_GETITEMSTATE, row, LVIS_SELECTED as isize)
                & LVIS_SELECTED as isize
                != 0;
            if col == 0 {
                r.right = r.left + SendMessageW(a.list, LVM_GETCOLUMNWIDTH, 0, 0) as i32;
            }
            paint::fill(
                d.nmcd.hdc,
                r,
                if selected {
                    if crate::theme::dark() {
                        0x0060442c
                    } else {
                        0x00ffe7d1
                    }
                } else {
                    paint::white()
                },
            );
            if col != 1 && col != 2 {
                r.left += 12;
                r.right -= 12;
                if let Some(value) = a.rows.get(row).and_then(|cells| cells.get(col)) {
                    paint::label(
                        d.nmcd.hdc,
                        &from_wide(value),
                        r,
                        a.font,
                        paint::ink(),
                        DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
                    );
                }
                return CDRF_SKIPDEFAULT as isize;
            }
            InflateRect(&mut r, -10, -9);
            if col == 1 {
                let (surface, ink) = match j.state() {
                    COMPLETE => (0x00e4f5e5, 0x005b7f28),
                    crate::model::ERROR => (0x00e5e5ff, 0x003c38e5),
                    DOWNLOADING => (0x00fff1df, paint::BLUE),
                    PAUSED => (0x00dff4ff, 0x002d7490),
                    _ => (0x00f6f1ed, paint::muted()),
                };
                paint::rounded(d.nmcd.hdc, r, surface, surface, 6.);
                paint::label(
                    d.nmcd.hdc,
                    &from_wide(&a.rows[row][1]),
                    r,
                    a.small,
                    if crate::theme::dark() {
                        paint::ink()
                    } else {
                        ink
                    },
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
            } else {
                let total = j.total.load(O::Relaxed);
                let done = j.done.load(O::Relaxed);
                r.top += 5;
                r.bottom -= 5;
                paint::rounded(d.nmcd.hdc, r, 0x00eee5df, 0x00eee5df, 5.);
                if total > 0 && done > 0 {
                    let end = r.left
                        + ((r.right - r.left) as u128 * done.min(total) as u128 / total as u128)
                            as i32;
                    paint::rounded(
                        d.nmcd.hdc,
                        RECT {
                            right: end.max(r.left + 3),
                            ..r
                        },
                        paint::BLUE,
                        paint::BLUE,
                        5.,
                    );
                }
                paint::label(
                    d.nmcd.hdc,
                    &from_wide(&a.rows[row][2]),
                    RECT {
                        top: r.top - 6,
                        bottom: r.bottom + 6,
                        ..r
                    },
                    a.small,
                    paint::ink(),
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
            }
            CDRF_SKIPDEFAULT as isize
        }
        _ => CDRF_DODEFAULT as isize,
    }
}
pub unsafe fn child(
    h: HWND,
    class: &str,
    label: &str,
    id: i32,
    style: u32,
    x: i32,
    y: i32,
    w: i32,
    height: i32,
    font: HFONT,
) -> HWND {
    let c = CreateWindowExW(
        0,
        wide(class).as_ptr(),
        wide(label).as_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        x,
        y,
        w,
        height,
        h,
        id as usize as HMENU,
        GetModuleHandleW(null()),
        null(),
    );
    SendMessageW(c, WM_SETFONT, font as usize, 1);
    if class == "EDIT" {
        paint::style_edit(c)
    }
    c
}
pub unsafe fn button(h: HWND, s: &str, id: i32, x: i32, y: i32, w: i32, f: HFONT) -> HWND {
    child(
        h,
        "BUTTON",
        s,
        id,
        BS_OWNERDRAW as u32 | WS_TABSTOP,
        x,
        y,
        w,
        36,
        f,
    )
}
pub unsafe fn edit(
    h: HWND,
    s: &str,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    f: HFONT,
    flags: u32,
) -> HWND {
    child(
        h,
        "EDIT",
        s,
        id,
        WS_TABSTOP | ES_AUTOHSCROLL as u32 | flags,
        x,
        y,
        w,
        36,
        f,
    )
}
pub unsafe fn label(h: HWND, s: &str, id: i32, x: i32, y: i32, w: i32, height: i32, f: HFONT) {
    child(
        h,
        "STATIC",
        s,
        id,
        SS_NOPREFIX
            | if [144, 501, 503, 400, 206].contains(&id) {
                SS_ENDELLIPSIS
            } else {
                0
            },
        x,
        y,
        w,
        height,
        f,
    );
}
pub unsafe fn value(h: HWND, id: i32) -> String {
    text(GetDlgItem(h, id))
}
pub unsafe fn message(h: HWND, s: &str) {
    MessageBoxW(
        h,
        wide(s).as_ptr(),
        wide("Open Download Manager").as_ptr(),
        MB_OK | MB_ICONERROR,
    );
}
pub unsafe fn clipboard(h: HWND) -> Option<String> {
    if OpenClipboard(h) == 0 {
        return None;
    }
    let m = GetClipboardData(13);
    let result = if m.is_null() {
        None
    } else {
        let p = GlobalLock(m) as *const u16;
        if p.is_null() {
            None
        } else {
            let mut n = 0;
            while n < 65536 && *p.add(n) != 0 {
                n += 1
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
            GlobalUnlock(m);
            Some(s)
        }
    };
    CloseClipboard();
    result
}
unsafe fn copy(h: HWND, s: &str) {
    if OpenClipboard(h) == 0 {
        return;
    }
    let s = wide(s);
    let m = GlobalAlloc(GMEM_MOVEABLE, s.len() * 2);
    if !m.is_null() {
        let p = GlobalLock(m);
        if !p.is_null() {
            std::ptr::copy_nonoverlapping(s.as_ptr(), p.cast(), s.len());
            GlobalUnlock(m);
            EmptyClipboard();
            if SetClipboardData(13, m).is_null() {
                GlobalFree(m);
            }
        } else {
            GlobalFree(m);
        }
    }
    CloseClipboard();
}
pub unsafe fn select_file(h: HWND, initial: &str, save: bool) -> Option<PathBuf> {
    let mut b = vec![0u16; 32768];
    let s = wide(initial);
    let n = s.len().min(b.len());
    b[..n].copy_from_slice(&s[..n]);
    let filter = wide("All files\0*.*\0\0");
    let mut of = OPENFILENAMEW::default();
    of.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    of.hwndOwner = h;
    of.lpstrFile = b.as_mut_ptr();
    of.nMaxFile = b.len() as u32;
    of.lpstrFilter = filter.as_ptr();
    of.Flags = OFN_EXPLORER
        | OFN_NOCHANGEDIR
        | OFN_PATHMUSTEXIST
        | if save {
            OFN_OVERWRITEPROMPT
        } else {
            OFN_FILEMUSTEXIST
        };
    if (if save {
        GetSaveFileNameW(&mut of)
    } else {
        GetOpenFileNameW(&mut of)
    }) != 0
    {
        Some(from_wide(&b).into())
    } else {
        None
    }
}
pub unsafe fn folder(h: HWND) -> Option<PathBuf> {
    let title = wide("Choose download folder");
    let mut b = BROWSEINFOW::default();
    b.hwndOwner = h;
    b.lpszTitle = title.as_ptr();
    b.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE;
    let p = SHBrowseForFolderW(&b);
    if p.is_null() {
        return None;
    }
    let mut out = vec![0; 32768];
    let ok = SHGetPathFromIDListW(p, out.as_mut_ptr());
    CoTaskMemFree(p.cast());
    if ok != 0 {
        Some(from_wide(&out).into())
    } else {
        None
    }
}
impl App {
    unsafe fn apply_theme(&mut self) {
        DeleteObject(self.brush);
        self.brush = CreateSolidBrush(paint::bg());
        SetWindowTextW(GetDlgItem(self.h, 126), wide(crate::theme::name()).as_ptr());
        SendMessageW(self.list, LVM_SETBKCOLOR, 0, paint::white() as isize);
        SendMessageW(self.list, LVM_SETTEXTBKCOLOR, 0, paint::white() as isize);
        SendMessageW(self.list, LVM_SETTEXTCOLOR, 0, paint::ink() as isize);
        #[link(name = "uxtheme", kind = "raw-dylib")]
        extern "system" {
            fn SetWindowTheme(h: HWND, app: *const u16, id: *const u16) -> i32;
        }
        let native_theme = wide(if crate::theme::dark() {
            "DarkMode_Explorer"
        } else {
            "Explorer"
        });
        SetWindowTheme(self.list, native_theme.as_ptr(), null());
        let header = SendMessageW(self.list, LVM_GETHEADER, 0, 0) as HWND;
        SetWindowTheme(header, native_theme.as_ptr(), null());
        SetWindowSubclass(header, Some(paint::header_proc), 500, 0);
        paint::modern(self.h);
        for &dialog in &self.dialogs {
            paint::modern(dialog);
            RedrawWindow(dialog, null(), null_mut(), RDW_INVALIDATE | RDW_ALLCHILDREN);
        }
        RedrawWindow(self.h, null(), null_mut(), RDW_INVALIDATE | RDW_ALLCHILDREN);
    }
    unsafe fn selected_jobs(&self) -> Vec<Arc<Job>> {
        let mut jobs = Vec::new();
        let mut at = -1isize;
        loop {
            at = SendMessageW(
                self.list,
                LVM_GETNEXTITEM,
                at as usize,
                LVNI_SELECTED as isize,
            );
            if at < 0 {
                break;
            }
            if let Some(j) = self.jobs.get(at as usize) {
                jobs.push(j.clone())
            }
        }
        jobs
    }
    unsafe fn tip(&mut self, control: HWND, text: &str) {
        self.tip_text.push(wide(text));
        let mut info = TTTOOLINFOW::default();
        info.cbSize = std::mem::size_of::<TTTOOLINFOW>() as u32;
        info.uFlags = TTF_IDISHWND | TTF_SUBCLASS;
        info.hwnd = self.h;
        info.uId = control as usize;
        info.lpszText = self.tip_text.last_mut().unwrap().as_mut_ptr();
        SendMessageW(
            self.tooltip,
            TTM_ADDTOOLW,
            0,
            (&info as *const TTTOOLINFOW) as isize,
        );
    }
    unsafe fn selected(&self) -> Option<Arc<Job>> {
        let i = SendMessageW(
            self.list,
            LVM_GETNEXTITEM,
            usize::MAX,
            LVNI_SELECTED as isize,
        );
        if i < 0 {
            None
        } else {
            self.jobs.get(i as usize).cloned()
        }
    }
    pub unsafe fn save(&self) {
        self.store.save(&self.engine, &self.jobs)
    }
    pub unsafe fn verify_file(&mut self, job: Arc<Job>, expected: String) {
        if job.active.load(O::Acquire) || job.state() != COMPLETE {
            message(self.h, "Finish the download before verifying its file.");
            return;
        }
        if job.snapshot().integrity_status == "Calculating SHA-256…" {
            return;
        }
        job.details.lock().unwrap().integrity_status = "Calculating SHA-256…".into();
        let tx = self.io_tx.clone();
        self.workers.push(std::thread::spawn(move || {
            let result = crate::integrity::hash(&job.snapshot().path, None);
            let _ = tx.send(FileResult::Integrity(job, expected, result));
        }));
    }
    pub unsafe fn add(
        &mut self,
        url: String,
        path: PathBuf,
        context: &Value,
    ) -> Result<(), String> {
        if self.jobs.len() >= 10000 {
            return Err(
                "The queue contains 10,000 downloads. Clear inactive entries before adding more."
                    .into(),
            );
        }
        let expected = crate::integrity::normalize(context["sha256"].as_str().unwrap_or(""))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create download folder: {e}"))?;
        }
        let path = self.engine.reserve(&path)?;
        let j = Job::new(Details {
            url,
            path,
            user_agent: context["userAgent"].as_str().unwrap_or("").into(),
            referer: context["referer"].as_str().unwrap_or("").into(),
            expected_sha256: expected,
            ..Default::default()
        });
        self.jobs.push(j.clone());
        self.refresh();
        SendMessageW(self.list, LVM_ENSUREVISIBLE, self.jobs.len() - 1, 0);
        crate::workflow::open(self, crate::workflow::Kind::Progress(j));
        self.save();
        PostMessageW(self.h, WM_APP + 74, 0, 0);
        Ok(())
    }
    unsafe fn refresh(&mut self) {
        let mut positions = Vec::new();
        let mut selected = HashSet::new();
        let mut at = -1isize;
        loop {
            at = SendMessageW(
                self.list,
                LVM_GETNEXTITEM,
                at as usize,
                LVNI_SELECTED as isize,
            );
            if at < 0 {
                break;
            }
            if let Some(j) = self.jobs.get(at as usize) {
                positions.push(at as usize);
                selected.insert(j.id);
            }
        }
        let col = self.sort;
        let reverse = self.descending;
        self.jobs.sort_by(|a, b| {
            let d = a.snapshot();
            let e = b.snapshot();
            let order = match col {
                0 => d.path.file_name().cmp(&e.path.file_name()),
                1 => a.state().cmp(&b.state()),
                2 => {
                    let x =
                        a.done.load(O::Relaxed) as u128 * b.total.load(O::Relaxed).max(1) as u128;
                    let y =
                        b.done.load(O::Relaxed) as u128 * a.total.load(O::Relaxed).max(1) as u128;
                    x.cmp(&y)
                }
                3 => a.done.load(O::Relaxed).cmp(&b.done.load(O::Relaxed)),
                4 => a.speed.load(O::Relaxed).cmp(&b.speed.load(O::Relaxed)),
                5 => a.lanes.load(O::Relaxed).cmp(&b.lanes.load(O::Relaxed)),
                6 => file_type(&d.path).cmp(file_type(&e.path)),
                _ => a.added.cmp(&b.added),
            };
            let order = if reverse { order.reverse() } else { order };
            order.then(a.id.cmp(&b.id))
        });
        self.rows = self
            .jobs
            .iter()
            .map(|j| {
                let d = j.snapshot();
                let done = j.done.load(O::Relaxed);
                let total = j.total.load(O::Relaxed);
                let speed = j.speed.load(O::Relaxed);
                [
                    d.path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    if j.assembling.load(O::Relaxed) {
                        "Assembling".into()
                    } else {
                        NAMES[j.state()].into()
                    },
                    if total == 0 {
                        "—".into()
                    } else {
                        format!("{}%", done.saturating_mul(100) / total.max(1))
                    },
                    format!(
                        "{} / {}",
                        size(done),
                        if total == 0 {
                            "—".into()
                        } else {
                            size(total)
                        }
                    ),
                    if speed == 0 {
                        "—".into()
                    } else {
                        format!("{}/s", size(speed))
                    },
                    j.lanes.load(O::Relaxed).to_string(),
                    file_type(&d.path).into(),
                    date(j.added),
                ]
                .iter()
                .map(wide)
                .collect()
            })
            .collect();
        SendMessageW(
            self.list,
            LVM_SETITEMCOUNT,
            self.rows.len(),
            (LVSICF_NOSCROLL | LVSICF_NOINVALIDATEALL) as isize,
        );
        let new_positions = self
            .jobs
            .iter()
            .enumerate()
            .filter(|(_, j)| selected.contains(&j.id))
            .map(|(i, _)| i)
            .collect::<Vec<_>>();
        if new_positions != positions {
            let clear = LVITEMW {
                stateMask: LVIS_SELECTED,
                state: 0,
                ..Default::default()
            };
            SendMessageW(
                self.list,
                LVM_SETITEMSTATE,
                usize::MAX,
                (&clear as *const LVITEMW) as isize,
            );
            for (i, j) in self.jobs.iter().enumerate() {
                if selected.contains(&j.id) {
                    let item = LVITEMW {
                        stateMask: LVIS_SELECTED,
                        state: LVIS_SELECTED,
                        ..Default::default()
                    };
                    SendMessageW(
                        self.list,
                        LVM_SETITEMSTATE,
                        i,
                        (&item as *const LVITEMW) as isize,
                    );
                }
            }
        }
        InvalidateRect(self.list, null(), 0);
        let selected = self.selected();
        let pause = selected
            .as_ref()
            .is_some_and(|j| j.state() == DOWNLOADING || j.state() == QUEUED);
        set_text(
            GetDlgItem(self.h, 120),
            if pause { "Pause" } else { "Resume" },
        );
        EnableWindow(
            GetDlgItem(self.h, 120),
            selected.as_ref().is_some_and(|j| j.state() != COMPLETE) as i32,
        );
        EnableWindow(
            GetDlgItem(self.h, 112),
            selected
                .as_ref()
                .is_some_and(|j| j.state() == DOWNLOADING || j.state() == QUEUED)
                as i32,
        );
        EnableWindow(
            GetDlgItem(self.h, 116),
            selected
                .as_ref()
                .is_some_and(|j| !j.active.load(O::Relaxed)) as i32,
        );
        set_text(
            GetDlgItem(self.h, 144),
            &selected.map(|j| j.snapshot().error).unwrap_or_default(),
        );
        let r = RECT {
            left: 20,
            top: 56,
            right: 1600,
            bottom: 92,
        };
        InvalidateRect(self.h, &r, 0);
    }
    unsafe fn tick(&mut self) {
        self.ticks += 1;
        self.workers.retain_mut(|t| !t.is_finished());
        for result in self.io_rx.try_iter().collect::<Vec<_>>() {
            match result {
                FileResult::Integrity(job, expected, result) => {
                    let mut details = job.details.lock().unwrap();
                    match result {
                        Ok(actual) => {
                            details.expected_sha256 = expected.clone();
                            details.integrity_status = if expected.is_empty() {
                                "SHA-256 calculated (no reference checksum supplied)"
                            } else if actual == expected {
                                "SHA-256 verified"
                            } else {
                                "Mismatch — do not use this file"
                            }
                            .into();
                            details.actual_sha256 = actual;
                        }
                        Err(error) => details.integrity_status = error,
                    }
                    drop(details);
                    self.save();
                }
                FileResult::Import(result) => {
                    self.importing = false;
                    match result {
                        Ok(urls) => {
                            for url in urls {
                                self.choices.push_back(serde_json::json!({"url":url}));
                            }
                        }
                        Err(e) => message(self.h, &e),
                    }
                }
                FileResult::Export(result) => {
                    self.exporting = false;
                    if let Err(e) = result {
                        message(self.h, &e)
                    }
                }
            }
        }
        if self.exiting {
            if self.workers.is_empty() && !self.importing && !self.exporting {
                self.save();
                DestroyWindow(self.h);
            }
            return;
        }
        let mut active = self
            .jobs
            .iter()
            .filter(|j| j.active.load(O::Relaxed))
            .count();
        for j in &self.jobs {
            if !j.active.load(O::Acquire)
                && j.resume_requested.swap(false, O::AcqRel)
                && j.state() != COMPLETE
            {
                j.stop.store(false, O::Relaxed);
                j.status.store(QUEUED, O::Relaxed);
            }
            if j.state() == QUEUED
                && !j.active.load(O::Relaxed)
                && active < self.engine.max_active.load(O::Relaxed)
            {
                j.stop.store(false, O::Relaxed);
                j.active.store(true, O::Relaxed);
                j.status.store(DOWNLOADING, O::Relaxed);
                let (e, j) = (self.engine.clone(), j.clone());
                self.workers
                    .push(std::thread::spawn(move || transfer::run(e, j)));
                active += 1
            }
        }
        let completed = self
            .jobs
            .iter()
            .filter(|j| j.state() == COMPLETE && !j.notified.swap(true, O::Relaxed))
            .cloned()
            .collect::<Vec<_>>();
        for j in completed {
            crate::workflow::open(self, crate::workflow::Kind::Complete(j));
        }
        if self.ticks % 20 == 0 {
            self.save()
        }
        self.refresh();
        if !self.dialogs.iter().any(|&h| crate::workflow::is_start(h)) {
            if let Some(v) = self.choices.pop_front() {
                crate::workflow::open(self, crate::workflow::Kind::Start(v));
            }
        }
    }
    unsafe fn command(&mut self, id: u32) {
        use crate::workflow::{open, Kind};
        let selected = self.selected();
        match id {
            122 => open(self, Kind::New),
            120 | 110 | 111 => {
                for j in self.selected_jobs() {
                    if j.state() == COMPLETE {
                        continue;
                    }
                    if id == 111 || (id == 120 && (j.state() == DOWNLOADING || j.state() == QUEUED))
                    {
                        j.status.store(PAUSED, O::Relaxed);
                        j.resume_requested.store(false, O::Relaxed);
                        j.stop.store(true, O::Relaxed);
                    } else if j.state() != COMPLETE {
                        j.notified.store(false, O::Relaxed);
                        if j.active.load(O::Acquire) {
                            j.resume_requested.store(true, O::Release);
                        } else {
                            j.stop.store(false, O::Relaxed);
                            j.status.store(QUEUED, O::Relaxed);
                        }
                        open(self, Kind::Progress(j));
                    }
                }
            }
            112 => {
                for j in self.selected_jobs() {
                    if j.state() == COMPLETE {
                        continue;
                    }
                    j.resume_requested.store(false, O::Relaxed);
                    j.status.store(CANCELED, O::Relaxed);
                    j.stop.store(true, O::Relaxed);
                }
            }
            113 => {
                if let Some(j) = selected {
                    if let Some(p) = j.snapshot().path.parent() {
                        shell_open(p)
                    }
                }
            }
            170 => {
                if let Some(j) = selected {
                    if j.state() == COMPLETE {
                        shell_open(&j.snapshot().path)
                    }
                }
            }
            171 => {
                if let Some(j) = selected {
                    copy(self.h, &j.snapshot().url)
                }
            }
            116 | 172 => {
                let removed = self
                    .selected_jobs()
                    .into_iter()
                    .filter(|j| !j.active.load(O::Relaxed))
                    .collect::<Vec<_>>();
                let ids = removed.iter().map(|j| j.id).collect::<HashSet<_>>();
                self.jobs.retain(|j| !ids.contains(&j.id));
                for j in removed {
                    self.engine
                        .reservations
                        .lock()
                        .unwrap()
                        .remove(&path_key(&j.snapshot().path));
                }
            }
            121 => self
                .jobs
                .retain(|j| j.active.load(O::Relaxed) || j.state() == QUEUED),
            117 => open(self, Kind::Settings),
            174 => {
                if let Some(job) = selected {
                    open(self, Kind::Checksum(job));
                }
            }
            124 => open(self, Kind::BrowserPlugins),
            125 => {
                if let Err(error) = crate::update::check() {
                    message(self.h, &error);
                }
            }
            126 => {
                crate::theme::cycle();
                self.apply_theme();
            }
            118 => {
                MessageBoxW(self.h,wide("Open Download Manager 0.2.0\nNative Rust edition\n\nMIT licensed. Third-party notices are included in the licenses folder.").as_ptr(),wide("About Open Download Manager").as_ptr(),MB_OK);
            }
            173 => {
                if let Some(j) = selected {
                    if !j.active.load(O::Relaxed) && j.state() != COMPLETE {
                        open(self, Kind::Renew(j))
                    }
                }
            }
            114 => {
                if self.importing {
                    message(self.h, "An import is already in progress.");
                    return;
                }
                if let Some(p) = select_file(self.h, "", false) {
                    self.importing = true;
                    let tx = self.io_tx.clone();
                    std::thread::spawn(move || {
                        let result = (|| {
                            let mut bytes = Vec::new();
                            std::fs::File::open(p)
                                .map_err(|e| e.to_string())?
                                .take(10 * 1024 * 1024 + 1)
                                .read_to_end(&mut bytes)
                                .map_err(|e| e.to_string())?;
                            if bytes.len() > 10 * 1024 * 1024 {
                                return Err("URL list exceeds 10 MB".into());
                            }
                            let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
                            let mut urls = Vec::new();
                            for line in text.trim_start_matches('\u{feff}').lines() {
                                let line = line.trim();
                                if line.is_empty() || line.starts_with('#') {
                                    continue;
                                }
                                valid_url(line)?;
                                urls.push(line.to_owned());
                                if urls.len() > 10000 {
                                    return Err("URL list exceeds 10,000 URLs".into());
                                }
                            }
                            Ok(urls)
                        })();
                        let _ = tx.send(FileResult::Import(result));
                    });
                }
            }
            115 => {
                if self.exporting {
                    message(self.h, "An export is already in progress.");
                    return;
                }
                if let Some(p) = select_file(self.h, "downloads.txt", true) {
                    let content = self
                        .jobs
                        .iter()
                        .map(|j| j.snapshot().url)
                        .collect::<Vec<_>>()
                        .join("\r\n");
                    self.exporting = true;
                    let tx = self.io_tx.clone();
                    std::thread::spawn(move || {
                        let _ = tx.send(FileResult::Export(
                            std::fs::write(p, content).map_err(|e| e.to_string()),
                        ));
                    });
                }
            }
            119 => {
                self.exiting = true;
                for j in &self.jobs {
                    if j.active.load(O::Relaxed) {
                        j.status.store(PAUSED, O::Relaxed);
                        j.resume_requested.store(false, O::Relaxed);
                        j.stop.store(true, O::Relaxed);
                    }
                }
            }
            _ => (),
        }
        self.refresh();
        self.save();
    }
}
unsafe extern "system" fn main_proc(h: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    if msg == WM_NCCREATE {
        let p = (*(lp as *const CREATESTRUCTW)).lpCreateParams as *mut App;
        SetWindowLongPtrW(h, GWLP_USERDATA, p as isize);
        (*p).h = h;
    }
    let p = GetWindowLongPtrW(h, GWLP_USERDATA) as *mut App;
    if p.is_null() {
        return DefWindowProcW(h, msg, wp, lp);
    }
    let a = &mut *p;
    match msg {
        WM_CREATE => {
            paint::modern(h);
            a.tooltip = CreateWindowExW(
                WS_EX_TOPMOST,
                wide("tooltips_class32").as_ptr(),
                null(),
                WS_POPUP | TTS_ALWAYSTIP,
                0,
                0,
                0,
                0,
                h,
                null_mut(),
                GetModuleHandleW(null()),
                null(),
            );
            SendMessageW(a.tooltip, TTM_SETMAXTIPWIDTH, 0, 300);
            for (i, (id, name)) in [
                (122, "New download"),
                (120, "Resume"),
                (112, "Stop"),
                (113, "Open folder"),
                (114, "Import URLs"),
                (115, "Export list"),
                (116, "Remove download"),
            ]
            .iter()
            .enumerate()
            {
                let c = button(h, name, *id, 24 + i as i32 * 54, 20, 44, a.font);
                a.tip(
                    c,
                    if *id == 120 {
                        "Pause or resume selected download"
                    } else {
                        name
                    },
                );
            }
            button(h, "Clear all", 121, 1000, 20, 110, a.font);
            let settings = button(h, "Settings", 117, 24, 700, 44, a.font);
            a.tip(settings, "Settings");
            button(h, "Browser plug-in", 124, 152, 700, 160, a.font);
            button(h, "Check for updates", 125, 320, 700, 156, a.font);
            let theme = button(h, crate::theme::name(), 126, 418, 700, 44, a.font);
            a.tip(theme, "Switch theme: System / Light / Dark");
            button(h, "About", 118, 1000, 700, 96, a.font);
            label(h, "", 144, 28, 760, 1100, 28, a.small);
            a.list = child(
                h,
                "SysListView32",
                "",
                130,
                LVS_REPORT | LVS_OWNERDATA | LVS_SHOWSELALWAYS | WS_TABSTOP,
                24,
                90,
                1100,
                560,
                a.font,
            );
            SendMessageW(
                a.list,
                LVM_SETEXTENDEDLISTVIEWSTYLE,
                0,
                (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as isize,
            );
            let images = ImageList_Create(1, 44, ILC_COLOR32, 1, 1);
            SendMessageW(
                a.list,
                LVM_SETIMAGELIST,
                LVSIL_SMALL as usize,
                images as isize,
            );
            for (i, s) in [
                "File",
                "Status",
                "Progress",
                "Received / Total",
                "Speed",
                "Connections",
                "Type",
                "Date added",
            ]
            .iter()
            .enumerate()
            {
                let mut s = wide(s);
                let c = LVCOLUMNW {
                    mask: LVCF_TEXT | LVCF_WIDTH,
                    cx: [260, 110, 120, 180, 90, 110, 90, 185][i],
                    pszText: s.as_mut_ptr(),
                    ..Default::default()
                };
                SendMessageW(
                    a.list,
                    LVM_INSERTCOLUMNW,
                    i,
                    (&c as *const LVCOLUMNW) as isize,
                );
            }
            let mut n = NOTIFYICONDATAW::default();
            n.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            n.hWnd = h;
            n.uID = 1;
            n.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
            n.uCallbackMessage = WM_APP + 70;
            n.hIcon = LoadIconW(GetModuleHandleW(null()), 1usize as *const u16);
            let t = wide("Open Download Manager");
            n.szTip[..t.len()].copy_from_slice(&t);
            Shell_NotifyIconW(NIM_ADD, &n);
            SetTimer(h, 1, 250, None);
            a.apply_theme();
            0
        }
        WM_GETMINMAXINFO => {
            (*(lp as *mut MINMAXINFO)).ptMinTrackSize = POINT { x: 1060, y: 760 };
            0
        }
        WM_SIZE => {
            let w = (lp as u32 & 65535) as i32;
            let height = ((lp as u32 >> 16) & 65535) as i32;
            MoveWindow(
                a.list,
                24,
                106,
                (w - 48).max(200),
                (height - 210).max(100),
                1,
            );
            SetWindowRgn(
                a.list,
                CreateRoundRectRgn(0, 0, (w - 48).max(200), (height - 210).max(100), 16, 16),
                1,
            );
            let widths = [(w - 48 - 885).max(180), 110, 120, 180, 90, 110, 90, 185];
            for (column, width) in widths.iter().enumerate() {
                SendMessageW(a.list, LVM_SETCOLUMNWIDTH, column, *width as isize);
            }
            MoveWindow(GetDlgItem(h, 121), w - 134, 20, 110, 36, 1);
            MoveWindow(GetDlgItem(h, 117), 24, height - 76, 44, 36, 1);
            MoveWindow(GetDlgItem(h, 124), 80, height - 76, 160, 36, 1);
            MoveWindow(GetDlgItem(h, 125), 252, height - 76, 156, 36, 1);
            MoveWindow(GetDlgItem(h, 126), 420, height - 76, 44, 36, 1);
            MoveWindow(GetDlgItem(h, 118), w - 120, height - 76, 96, 36, 1);
            MoveWindow(
                GetDlgItem(h, 144),
                480,
                height - 70,
                (w - 616).max(0),
                26,
                1,
            );
            InvalidateRect(h, null(), 0);
            0
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED => {
            if crate::theme::refresh() {
                a.apply_theme();
            }
            0
        }
        WM_TIMER => {
            if a.ticks % 20 == 0 && crate::theme::refresh() {
                a.apply_theme();
            }
            a.tick();
            0
        }
        m if m == WM_APP + 74 => {
            a.tick();
            0
        }
        WM_COMMAND => {
            a.command(wp as u32 & 65535);
            0
        }
        WM_CLOSE => {
            ShowWindow(h, SW_HIDE);
            0
        }
        WM_ERASEBKGND => 1,
        WM_DRAWITEM => {
            paint::button(&*(lp as *const DRAWITEMSTRUCT), a.font, true);
            1
        }
        WM_CTLCOLORSTATIC => {
            SetTextColor(wp as HDC, paint::muted());
            SetBkMode(wp as HDC, TRANSPARENT as i32);
            a.brush as isize
        }
        WM_PAINT | WM_PRINTCLIENT => {
            paint::buffered(
                h,
                if msg == WM_PRINTCLIENT {
                    Some(wp as HDC)
                } else {
                    None
                },
                paint::bg(),
                |dc| {
                    let mut r = RECT::default();
                    GetClientRect(h, &mut r);
                    paint::rounded(
                        dc,
                        RECT {
                            left: 16,
                            top: 96,
                            right: r.right - 16,
                            bottom: r.bottom - 94,
                        },
                        paint::white(),
                        paint::border(),
                        12.,
                    );
                    paint::label(
                        dc,
                        "Download queue",
                        RECT {
                            left: 28,
                            top: 64,
                            right: 300,
                            bottom: 92,
                        },
                        a.bold,
                        paint::ink(),
                        DT_SINGLELINE | DT_VCENTER,
                    );
                    let active = a.jobs.iter().filter(|j| j.active.load(O::Relaxed)).count();
                    let speed = a.jobs.iter().map(|j| j.speed.load(O::Relaxed)).sum::<u64>();
                    let summary = format!(
                        "{} active     {} queued     {} complete     {}/s",
                        active,
                        a.jobs.iter().filter(|j| j.state() == QUEUED).count(),
                        a.jobs.iter().filter(|j| j.state() == COMPLETE).count(),
                        size(speed)
                    );
                    paint::label(
                        dc,
                        &summary,
                        RECT {
                            left: 320,
                            top: 64,
                            right: r.right - 28,
                            bottom: 92,
                        },
                        a.small,
                        paint::muted(),
                        DT_SINGLELINE | DT_VCENTER | DT_RIGHT,
                    );
                },
            );
            0
        }
        WM_NOTIFY => {
            let n = &*(lp as *const NMHDR);
            if n.idFrom == 130 {
                match n.code {
                    NM_CUSTOMDRAW => draw_cell(a, &mut *(lp as *mut NMLVCUSTOMDRAW)),
                    LVN_GETDISPINFOW => {
                        let d = &mut *(lp as *mut NMLVDISPINFOW);
                        if d.item.mask & LVIF_TEXT != 0 {
                            if let Some(s) = a
                                .rows
                                .get(d.item.iItem as usize)
                                .and_then(|r| r.get(d.item.iSubItem as usize))
                            {
                                d.item.pszText = s.as_ptr() as *mut u16;
                            }
                        }
                        0
                    }
                    LVN_COLUMNCLICK => {
                        let d = &*(lp as *const NMLISTVIEW);
                        if a.sort == d.iSubItem as usize {
                            a.descending = !a.descending
                        } else {
                            a.sort = d.iSubItem as usize;
                            a.descending = false
                        }
                        a.refresh();
                        0
                    }
                    NM_DBLCLK => {
                        if let Some(j) = a.selected() {
                            if j.state() == COMPLETE {
                                shell_open(&j.snapshot().path)
                            } else {
                                crate::workflow::open(a, crate::workflow::Kind::Progress(j));
                            }
                        }
                        0
                    }
                    NM_RCLICK => {
                        let m = CreatePopupMenu();
                        for (id, s) in [
                            (170, "Open file"),
                            (113, "Open folder"),
                            (110, "Resume / Retry"),
                            (111, "Pause"),
                            (112, "Stop"),
                            (171, "Copy link"),
                            (173, "Renew link"),
                            (174, "Verify SHA-256…"),
                            (172, "Remove download"),
                        ] {
                            AppendMenuW(m, MF_STRING, id, wide(s).as_ptr());
                        }
                        let mut pt = POINT::default();
                        GetCursorPos(&mut pt);
                        let id = TrackPopupMenu(m, TPM_RETURNCMD, pt.x, pt.y, 0, h, null());
                        DestroyMenu(m);
                        if id != 0 {
                            a.command(id as u32);
                        }
                        0
                    }
                    _ => DefWindowProcW(h, msg, wp, lp),
                }
            } else {
                DefWindowProcW(h, msg, wp, lp)
            }
        }
        WM_COPYDATA => {
            let d = &*(lp as *const COPYDATASTRUCT);
            if d.dwData != 0x4f444d33 || d.cbData > 65536 || d.lpData.is_null() {
                return 0;
            }
            let v: Value = match serde_json::from_slice(std::slice::from_raw_parts(
                d.lpData as *const u8,
                d.cbData as usize,
            )) {
                Ok(v) => v,
                Err(_) => return 0,
            };
            if valid_url(v["url"].as_str().unwrap_or("")).is_err() || a.choices.len() > 100 {
                return 0;
            }
            a.decisions
                .insert(v["promptId"].as_u64().unwrap_or(0) as usize, 0);
            a.choices.push_back(v);
            PostMessageW(h, WM_APP + 74, 0, 0);
            1
        }
        m if m == WM_APP + 71 => *a.decisions.get(&wp).unwrap_or(&0),
        m if m == WM_APP + 73 => {
            for v in &mut a.choices {
                if v["promptId"].as_u64() == Some(wp as u64) {
                    v["checking"] = Value::Bool(false);
                    v["refused"] = Value::Bool(lp != 1);
                }
            }
            for &d in &a.dialogs {
                crate::workflow::verified(d, wp, lp == 1);
            }
            1
        }
        m if m == WM_APP + 70 => {
            if lp as u32 == WM_LBUTTONUP || lp as u32 == WM_LBUTTONDBLCLK {
                ShowWindow(h, SW_RESTORE);
                SetForegroundWindow(h);
            } else if lp as u32 == WM_RBUTTONUP {
                let m = CreatePopupMenu();
                AppendMenuW(m, MF_STRING, 190, wide("Open Download Manager").as_ptr());
                AppendMenuW(m, MF_STRING, 119, wide("Exit").as_ptr());
                let mut pt = POINT::default();
                GetCursorPos(&mut pt);
                SetForegroundWindow(h);
                let id = TrackPopupMenu(m, TPM_RETURNCMD, pt.x, pt.y, 0, h, null());
                DestroyMenu(m);
                if id == 190 {
                    ShowWindow(h, SW_RESTORE);
                } else if id == 119 {
                    a.command(119)
                }
            }
            0
        }
        WM_DESTROY => {
            let mut n = NOTIFYICONDATAW::default();
            n.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            n.hWnd = h;
            n.uID = 1;
            Shell_NotifyIconW(NIM_DELETE, &n);
            for d in a.dialogs.clone() {
                DestroyWindow(d);
            }
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(h, msg, wp, lp),
    }
}
pub fn run(engine: Arc<Engine>, background: bool) {
    crate::theme::init();
    let browser_plugins = std::env::args().any(|arg| arg == "--browser-plugins");
    unsafe {
        let mutex = Handle(CreateMutexW(
            null(),
            0,
            wide("Local\\OpenDownloadManagerRust").as_ptr(),
        ));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let h = FindWindowW(wide(bridge::CLASS).as_ptr(), null());
            if browser_plugins && !h.is_null() {
                PostMessageW(h, WM_COMMAND, 124, 0);
            }
            if !background && !h.is_null() {
                ShowWindow(h, SW_RESTORE);
                SetForegroundWindow(h);
            }
            return;
        }
        let _mutex = mutex;
        CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
        let controls = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES | ICC_STANDARD_CLASSES,
        };
        InitCommonControlsEx(&controls);
        let jobs = match store::load(&engine) {
            Ok(j) => j,
            Err(e) => {
                message(null_mut(), &format!("Could not load saved queue: {e}"));
                return;
            }
        };
        for j in &jobs {
            engine
                .reservations
                .lock()
                .unwrap()
                .insert(path_key(&j.snapshot().path));
        }
        let (io_tx, io_rx) = mpsc::channel();
        let mut a = Box::new(App {
            store: store::Store::new(&engine),
            engine,
            jobs,
            h: null_mut(),
            list: null_mut(),
            font: paint::font(16, FW_NORMAL),
            small: paint::font(13, FW_NORMAL),
            bold: paint::font(17, FW_SEMIBOLD),
            brush: CreateSolidBrush(paint::bg()),
            dialogs: Vec::new(),
            workers: Vec::new(),
            rows: Vec::new(),
            sort: 7,
            descending: false,
            choices: VecDeque::new(),
            decisions: HashMap::new(),
            exiting: false,
            ticks: 0,
            folder: PathBuf::from(std::env::var_os("USERPROFILE").unwrap_or_default())
                .join("Downloads"),
            io_tx,
            io_rx,
            importing: false,
            exporting: false,
            tooltip: null_mut(),
            tip_text: Vec::new(),
        });
        for (class, proc) in [
            (
                bridge::CLASS,
                Some(main_proc as unsafe extern "system" fn(HWND, u32, usize, isize) -> isize),
            ),
            (
                "ODMWorkflowRust",
                Some(
                    crate::workflow::proc
                        as unsafe extern "system" fn(HWND, u32, usize, isize) -> isize,
                ),
            ),
        ] {
            let class = wide(class);
            let c = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: proc,
                hInstance: GetModuleHandleW(null()),
                hIcon: LoadIconW(GetModuleHandleW(null()), 1usize as *const u16),
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                lpszClassName: class.as_ptr(),
                ..Default::default()
            };
            RegisterClassExW(&c);
        }
        let h = CreateWindowExW(
            0,
            wide(bridge::CLASS).as_ptr(),
            wide("Open Download Manager").as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1200,
            850,
            null_mut(),
            null_mut(),
            GetModuleHandleW(null()),
            (&mut *a as *mut App).cast(),
        );
        if h.is_null() {
            message(null_mut(), &error("Create main window"));
            return;
        }
        a.refresh();
        if !a.jobs.is_empty() {
            SendMessageW(a.list, LVM_ENSUREVISIBLE, a.jobs.len() - 1, 0);
        }
        if !background {
            ShowWindow(h, SW_SHOW);
            UpdateWindow(h);
        }
        if browser_plugins {
            PostMessageW(h, WM_COMMAND, 124, 0);
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            let dialogs = a.dialogs.clone();
            if !(msg.message >= WM_KEYFIRST
                && msg.message <= WM_KEYLAST
                && dialogs
                    .iter()
                    .any(|&d| IsWindow(d) != 0 && IsDialogMessageW(d, &msg) != 0))
            {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        a.save();
        DeleteObject(a.font);
        DeleteObject(a.small);
        DeleteObject(a.bold);
        DeleteObject(a.brush);
        CoUninitialize();
    }
}
