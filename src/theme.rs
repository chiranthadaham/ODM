use crate::win::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
static MODE: AtomicUsize = AtomicUsize::new(0);
static DARK: AtomicBool = AtomicBool::new(false);
pub fn dark() -> bool {
    DARK.load(Ordering::Relaxed)
}
pub fn name() -> &'static str {
    match MODE.load(Ordering::Relaxed) {
        1 => "Theme: Light",
        2 => "Theme: Dark",
        _ => "Theme: System",
    }
}
fn path() -> std::path::PathBuf {
    std::env::var_os("ODM_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default())
                .join("OpenDownloadManagerRust")
        })
        .join("theme.txt")
}
pub fn init() {
    let mode = std::fs::read_to_string(path())
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .unwrap_or(0)
        .min(2);
    MODE.store(mode, Ordering::Relaxed);
    refresh();
}
pub fn cycle() {
    MODE.store((MODE.load(Ordering::Relaxed) + 1) % 3, Ordering::Relaxed);
    let _ = std::fs::write(path(), MODE.load(Ordering::Relaxed).to_string());
    refresh();
}
pub fn refresh() -> bool {
    use windows_sys::Win32::System::Registry::*;
    let mut light = 1u32;
    let mut size = 4u32;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize").as_ptr(),
            wide("AppsUseLightTheme").as_ptr(),
            RRF_RT_REG_DWORD,
            null_mut(),
            (&mut light as *mut u32).cast(),
            &mut size,
        );
    }
    let value = match MODE.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => light == 0,
    };
    DARK.swap(value, Ordering::Relaxed) != value
}
pub unsafe fn surface_brush() -> HBRUSH {
    use std::sync::OnceLock;
    static LIGHT: OnceLock<usize> = OnceLock::new();
    static DARK_BRUSH: OnceLock<usize> = OnceLock::new();
    let cell = if dark() { &DARK_BRUSH } else { &LIGHT };
    *cell.get_or_init(|| CreateSolidBrush(if dark() { 0x00282420 } else { 0x00ffffff }) as usize)
        as HBRUSH
}
