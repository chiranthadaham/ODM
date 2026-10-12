pub use std::{
    ffi::c_void,
    ptr::{null, null_mut},
};
pub use windows_sys::Win32::System::Time::*;
pub use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    Networking::WinHttp::*,
    Security::Cryptography::*,
    Storage::FileSystem::*,
    System::{
        Com::*, DataExchange::*, JobObjects::*, LibraryLoader::*, Memory::*, SystemInformation::*,
        Threading::*,
    },
    UI::{Controls::Dialogs::*, Controls::*, Shell::*, WindowsAndMessaging::*},
};
pub fn wide(s: impl AsRef<str>) -> Vec<u16> {
    s.as_ref().encode_utf16().chain(Some(0)).collect()
}
pub fn from_wide(s: &[u16]) -> String {
    String::from_utf16_lossy(&s[..s.iter().position(|&c| c == 0).unwrap_or(s.len())])
}
pub unsafe fn text(h: HWND) -> String {
    let n = SendMessageW(h, WM_GETTEXTLENGTH, 0, 0).clamp(0, 32767) as usize;
    let mut b = vec![0; n + 1];
    SendMessageW(h, WM_GETTEXT, b.len(), b.as_mut_ptr() as isize);
    from_wide(&b)
}
pub unsafe fn set_text(h: HWND, s: &str) {
    if text(h) != s {
        SetWindowTextW(h, wide(s).as_ptr());
    }
}
pub fn error(action: &str) -> String {
    format!("{action} (Windows error {})", unsafe { GetLastError() })
}
pub struct Handle(pub HANDLE);
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
pub fn now_filetime() -> u64 {
    let mut t = FILETIME::default();
    unsafe { GetSystemTimeAsFileTime(&mut t) };
    (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime)
}
pub fn shell_open(path: &std::path::Path) {
    let path = path.to_string_lossy().into_owned();
    std::thread::spawn(move || unsafe {
        CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
        ShellExecuteW(
            null_mut(),
            wide("open").as_ptr(),
            wide(path).as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        );
        CoUninitialize();
    });
}
pub fn open_browser_store(chrome: bool, address: &str) {
    let Ok(url) = url::Url::parse(address) else {
        return;
    };
    let host = if chrome {
        "chromewebstore.google.com"
    } else {
        "addons.mozilla.org"
    };
    if url.scheme() != "https" || url.host_str() != Some(host) {
        return;
    }
    let address = format!("\"{}\"", url.as_str());
    std::thread::spawn(move || unsafe {
        CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
        let result = ShellExecuteW(
            null_mut(),
            wide("open").as_ptr(),
            wide(if chrome { "chrome.exe" } else { "firefox.exe" }).as_ptr(),
            wide(address).as_ptr(),
            null(),
            SW_SHOWNORMAL,
        );
        if result as isize <= 32 {
            MessageBoxW(
                null_mut(),
                wide(if chrome {
                    "Chrome was not found. Install Chrome, then try again."
                } else {
                    "Firefox was not found. Install Firefox, then try again."
                })
                .as_ptr(),
                wide("Browser plug-in").as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
        }
        CoUninitialize();
    });
}
pub fn move_replace(from: &std::path::Path, to: &std::path::Path) -> Result<(), String> {
    if unsafe {
        MoveFileExW(
            wide(from.to_string_lossy()).as_ptr(),
            wide(to.to_string_lossy()).as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        Err(error("Save file"))
    } else {
        Ok(())
    }
}
pub fn size(n: u64) -> String {
    let mut x = n as f64;
    let u = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut i = 0;
    while x >= 1024. && i < 4 {
        x /= 1024.;
        i += 1;
    }
    format!("{x:.1} {}", u[i])
}
pub fn eta(seconds: u64) -> String {
    match seconds {
        0..=59 => format!("{seconds}s"),
        60..=3599 => format!("{}m {}s", seconds / 60, seconds % 60),
        3600..=86399 => format!("{}h {}m", seconds / 3600, seconds % 3600 / 60),
        _ => format!("{}d {}h", seconds / 86400, seconds % 86400 / 3600),
    }
}
pub fn date(value: u64) -> String {
    if value == 0 {
        return "—".into();
    }
    let t = FILETIME {
        dwLowDateTime: value as u32,
        dwHighDateTime: (value >> 32) as u32,
    };
    let mut local = FILETIME::default();
    let mut time = SYSTEMTIME::default();
    unsafe {
        if FileTimeToLocalFileTime(&t, &mut local) == 0
            || FileTimeToSystemTime(&local, &mut time) == 0
        {
            return "—".into();
        }
    }
    format!(
        "{:04}-{:02}-{:02}  {:02}:{:02}:{:02}",
        time.wYear, time.wMonth, time.wDay, time.wHour, time.wMinute, time.wSecond
    )
}
pub use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
