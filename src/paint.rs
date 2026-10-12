use crate::win::*;
use std::sync::OnceLock;
pub const BG: u32 = 0x00faf8f6;
pub const INK: u32 = 0x003b291e;
pub const MUTED: u32 = 0x00867568;
pub const BLUE: u32 = 0x00ff7a00;
pub const BORDER: u32 = 0x00ede7e2;
pub const WHITE: u32 = 0x00ffffff;
#[repr(C)]
struct Startup {
    version: u32,
    callback: *mut c_void,
    suppress_background: i32,
    suppress_codecs: i32,
}
#[link(name = "gdiplus", kind = "raw-dylib")]
extern "system" {
    fn GdiplusStartup(token: *mut usize, input: *const Startup, output: *mut c_void) -> i32;
    fn GdipCreateFromHDC(dc: HDC, g: *mut *mut c_void) -> i32;
    fn GdipDeleteGraphics(g: *mut c_void) -> i32;
    fn GdipSetSmoothingMode(g: *mut c_void, mode: i32) -> i32;
    fn GdipSetPixelOffsetMode(g: *mut c_void, mode: i32) -> i32;
    fn GdipCreatePath(mode: i32, path: *mut *mut c_void) -> i32;
    fn GdipDeletePath(path: *mut c_void) -> i32;
    fn GdipAddPathArc(
        path: *mut c_void,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        start: f32,
        sweep: f32,
    ) -> i32;
    fn GdipAddPathLine(path: *mut c_void, x: f32, y: f32, a: f32, b: f32) -> i32;
    fn GdipAddPathBezier(
        path: *mut c_void,
        x: f32,
        y: f32,
        a: f32,
        b: f32,
        c: f32,
        d: f32,
        e: f32,
        f: f32,
    ) -> i32;
    fn GdipClosePathFigure(path: *mut c_void) -> i32;
    fn GdipStartPathFigure(path: *mut c_void) -> i32;
    fn GdipCreateSolidFill(color: u32, brush: *mut *mut c_void) -> i32;
    fn GdipDeleteBrush(brush: *mut c_void) -> i32;
    fn GdipCreatePen1(color: u32, width: f32, unit: i32, pen: *mut *mut c_void) -> i32;
    fn GdipDeletePen(pen: *mut c_void) -> i32;
    fn GdipSetPenStartCap(pen: *mut c_void, cap: i32) -> i32;
    fn GdipSetPenEndCap(pen: *mut c_void, cap: i32) -> i32;
    fn GdipSetPenLineJoin(pen: *mut c_void, join: i32) -> i32;
    fn GdipFillPath(g: *mut c_void, brush: *mut c_void, path: *mut c_void) -> i32;
    fn GdipDrawPath(g: *mut c_void, pen: *mut c_void, path: *mut c_void) -> i32;
}
fn argb(c: u32) -> u32 {
    0xff000000 | ((c & 255) << 16) | (c & 0xff00) | ((c >> 16) & 255)
}
struct Graphics {
    g: *mut c_void,
    p: *mut c_void,
}
impl Graphics {
    unsafe fn new(dc: HDC) -> Self {
        static TOKEN: OnceLock<usize> = OnceLock::new();
        TOKEN.get_or_init(|| {
            let mut token = 0;
            GdiplusStartup(
                &mut token,
                &Startup {
                    version: 1,
                    callback: null_mut(),
                    suppress_background: 0,
                    suppress_codecs: 0,
                },
                null_mut(),
            );
            token
        });
        let (mut g, mut p) = (null_mut(), null_mut());
        GdipCreateFromHDC(dc, &mut g);
        GdipSetSmoothingMode(g, 4);
        GdipSetPixelOffsetMode(g, 4);
        GdipCreatePath(0, &mut p);
        Self { g, p }
    }
    unsafe fn line(&self, x: f32, y: f32, a: f32, b: f32) {
        GdipAddPathLine(self.p, x, y, a, b);
    }
    unsafe fn bezier(&self, v: [f32; 8]) {
        GdipAddPathBezier(self.p, v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]);
    }
    unsafe fn round(&self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        let d = r * 2.;
        GdipAddPathArc(self.p, x, y, d, d, 180., 90.);
        GdipAddPathArc(self.p, x + w - d, y, d, d, 270., 90.);
        GdipAddPathArc(self.p, x + w - d, y + h - d, d, d, 0., 90.);
        GdipAddPathArc(self.p, x, y + h - d, d, d, 90., 90.);
        GdipClosePathFigure(self.p);
    }
    unsafe fn draw(&self, color: u32, width: f32) {
        let mut pen = null_mut();
        GdipCreatePen1(argb(color), width, 2, &mut pen);
        GdipSetPenStartCap(pen, 2);
        GdipSetPenEndCap(pen, 2);
        GdipSetPenLineJoin(pen, 2);
        GdipDrawPath(self.g, pen, self.p);
        GdipDeletePen(pen);
    }
    unsafe fn fill(&self, color: u32) {
        let mut brush = null_mut();
        GdipCreateSolidFill(argb(color), &mut brush);
        GdipFillPath(self.g, brush, self.p);
        GdipDeleteBrush(brush);
    }
}
impl Drop for Graphics {
    fn drop(&mut self) {
        unsafe {
            GdipDeletePath(self.p);
            GdipDeleteGraphics(self.g);
        }
    }
}
pub unsafe fn fill(dc: HDC, r: RECT, color: u32) {
    let b = CreateSolidBrush(color);
    FillRect(dc, &r, b);
    DeleteObject(b);
}
pub unsafe fn rounded(dc: HDC, r: RECT, fill_color: u32, stroke: u32, radius: f32) {
    if r.right <= r.left || r.bottom <= r.top {
        return;
    }
    let g = Graphics::new(dc);
    g.round(
        r.left as f32 + 0.5,
        r.top as f32 + 0.5,
        (r.right - r.left - 1) as f32,
        (r.bottom - r.top - 1) as f32,
        radius,
    );
    g.fill(fill_color);
    g.draw(stroke, 1.);
}
pub unsafe fn label(dc: HDC, value: &str, mut r: RECT, font: HFONT, color: u32, flags: u32) {
    let old = SelectObject(dc, font);
    SetTextColor(dc, color);
    SetBkMode(dc, TRANSPARENT as i32);
    let value = wide(value);
    DrawTextW(dc, value.as_ptr(), (value.len() - 1) as i32, &mut r, flags);
    SelectObject(dc, old);
}
pub unsafe fn font(px: i32, weight: u32) -> HFONT {
    CreateFontW(
        -px,
        0,
        0,
        0,
        weight as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        CLEARTYPE_QUALITY as u32,
        DEFAULT_PITCH as u32,
        wide("Segoe UI").as_ptr(),
    )
}
pub unsafe fn icon(dc: HDC, box_: RECT, mut id: u32, color: u32, pause: bool) {
    if id == 602 {
        id = 105
    }
    if id == 603 {
        id = 102
    }
    let x = ((box_.left + box_.right) / 2 - 12) as f32;
    let y = ((box_.top + box_.bottom) / 2 - 12) as f32;
    let g = Graphics::new(dc);
    let line = |a: f32, b: f32, c: f32, d: f32| {
        GdipStartPathFigure(g.p);
        g.line(x + a, y + b, x + c, y + d);
    };
    match id {
        122 => {
            line(12., 5., 12., 19.);
            line(5., 12., 19., 12.);
        }
        120 => {
            if pause {
                g.round(x + 6., y + 5., 4., 14., 1.);
                GdipStartPathFigure(g.p);
                g.round(x + 14., y + 5., 4., 14., 1.);
            } else {
                g.line(x + 7.5, y + 5., x + 18., y + 11.);
                g.bezier([
                    x + 18.,
                    y + 11.,
                    x + 19.,
                    y + 11.5,
                    x + 19.,
                    y + 12.5,
                    x + 18.,
                    y + 13.,
                ]);
                g.line(x + 18., y + 13., x + 7.5, y + 19.);
                g.bezier([
                    x + 7.5,
                    y + 19.,
                    x + 6.7,
                    y + 19.5,
                    x + 6.,
                    y + 19.,
                    x + 6.,
                    y + 18.,
                ]);
                g.line(x + 6., y + 18., x + 6., y + 6.);
                g.bezier([
                    x + 6.,
                    y + 6.,
                    x + 6.,
                    y + 5.,
                    x + 6.7,
                    y + 4.5,
                    x + 7.5,
                    y + 5.,
                ]);
                GdipClosePathFigure(g.p);
            }
            g.fill(color);
            return;
        }
        112 => {
            g.round(x + 6., y + 6., 12., 12., 2.3);
            g.fill(color);
            return;
        }
        102 | 113 => {
            g.line(x + 4., y + 6., x + 9., y + 6.);
            g.line(x + 9., y + 6., x + 11., y + 8.);
            g.line(x + 11., y + 8., x + 20., y + 8.);
            g.bezier([
                x + 20.,
                y + 8.,
                x + 21.,
                y + 8.,
                x + 21.,
                y + 9.,
                x + 21.,
                y + 10.,
            ]);
            g.line(x + 21., y + 10., x + 21., y + 18.);
            g.bezier([
                x + 21.,
                y + 18.,
                x + 21.,
                y + 19.,
                x + 20.,
                y + 19.,
                x + 19.,
                y + 19.,
            ]);
            g.line(x + 19., y + 19., x + 4., y + 19.);
            g.bezier([
                x + 4.,
                y + 19.,
                x + 3.,
                y + 19.,
                x + 3.,
                y + 18.,
                x + 3.,
                y + 17.,
            ]);
            g.line(x + 3., y + 17., x + 3., y + 8.);
            g.bezier([
                x + 3.,
                y + 8.,
                x + 3.,
                y + 6.,
                x + 3.,
                y + 6.,
                x + 4.,
                y + 6.,
            ]);
        }
        105 => {
            g.round(x + 5., y + 5., 14., 16., 2.);
            GdipStartPathFigure(g.p);
            g.round(x + 8., y + 3., 8., 5., 1.5);
            line(9., 12., 15., 12.);
            line(9., 16., 14., 16.);
        }
        116 => {
            line(4., 6., 20., 6.);
            line(9., 3., 15., 3.);
            GdipStartPathFigure(g.p);
            g.line(x + 6., y + 6., x + 7., y + 19.);
            g.bezier([
                x + 7.,
                y + 19.,
                x + 7.,
                y + 20.,
                x + 8.,
                y + 21.,
                x + 9.,
                y + 21.,
            ]);
            g.line(x + 9., y + 21., x + 15., y + 21.);
            g.bezier([
                x + 15.,
                y + 21.,
                x + 16.,
                y + 21.,
                x + 17.,
                y + 20.,
                x + 17.,
                y + 19.,
            ]);
            g.line(x + 17., y + 19., x + 18., y + 6.);
            line(10., 10., 10.5, 17.);
            line(14., 10., 13.5, 17.);
        }
        114 | 115 => {
            g.line(x + 6., y + 2., x + 14., y + 2.);
            g.line(x + 14., y + 2., x + 20., y + 8.);
            g.line(x + 20., y + 8., x + 20., y + 20.);
            g.bezier([
                x + 20.,
                y + 20.,
                x + 20.,
                y + 21.,
                x + 19.,
                y + 22.,
                x + 18.,
                y + 22.,
            ]);
            g.line(x + 18., y + 22., x + 6., y + 22.);
            g.bezier([
                x + 6.,
                y + 22.,
                x + 5.,
                y + 22.,
                x + 4.,
                y + 21.,
                x + 4.,
                y + 20.,
            ]);
            g.line(x + 4., y + 20., x + 4., y + 4.);
            g.bezier([
                x + 4.,
                y + 4.,
                x + 4.,
                y + 3.,
                x + 5.,
                y + 2.,
                x + 6.,
                y + 2.,
            ]);
            line(14., 2., 14., 8.);
            line(14., 8., 20., 8.);
            if id == 114 {
                GdipStartPathFigure(g.p);
                g.bezier([
                    x + 11.,
                    y + 12.,
                    x + 9.,
                    y + 10.,
                    x + 6.,
                    y + 12.,
                    x + 7.,
                    y + 15.,
                ]);
                g.bezier([
                    x + 7.,
                    y + 15.,
                    x + 7.5,
                    y + 17.,
                    x + 9.5,
                    y + 18.,
                    x + 11.,
                    y + 16.,
                ]);
                GdipStartPathFigure(g.p);
                g.bezier([
                    x + 13.,
                    y + 12.,
                    x + 15.,
                    y + 10.,
                    x + 18.,
                    y + 12.,
                    x + 17.,
                    y + 15.,
                ]);
                g.bezier([
                    x + 17.,
                    y + 15.,
                    x + 16.5,
                    y + 17.,
                    x + 14.5,
                    y + 18.,
                    x + 13.,
                    y + 16.,
                ]);
                line(10., 14., 14., 14.);
            } else {
                line(12., 18., 12., 11.);
                line(9., 14., 12., 11.);
                line(12., 11., 15., 14.);
            }
        }
        _ => (),
    };
    g.draw(color, 1.65);
}
pub unsafe fn button(d: &DRAWITEMSTRUCT, body: HFONT, main: bool) {
    let primary = [604, 203, 314, 401, 504].contains(&d.CtlID);
    let disabled = d.itemState & ODS_DISABLED != 0;
    let pressed = d.itemState & ODS_SELECTED != 0;
    let icons = [122, 120, 112, 113, 114, 115, 116, 602, 603, 105, 102].contains(&d.CtlID);
    fill(d.hDC, d.rcItem, if main { BG } else { WHITE });
    let mut r = d.rcItem;
    InflateRect(&mut r, -1, -1);
    let surface = if disabled {
        0x00f6f2ef
    } else if primary {
        if pressed {
            0x00dc6700
        } else {
            BLUE
        }
    } else if pressed {
        0x00f7ede5
    } else {
        WHITE
    };
    rounded(
        d.hDC,
        r,
        surface,
        if primary { surface } else { 0x00f0e9e5 },
        10.,
    );
    if icons {
        icon(
            d.hDC,
            r,
            d.CtlID,
            if disabled { 0x00bbada4 } else { INK },
            text(d.hwndItem) == "Pause",
        )
    } else {
        label(
            d.hDC,
            &text(d.hwndItem),
            r,
            body,
            if disabled {
                MUTED
            } else if primary {
                WHITE
            } else {
                INK
            },
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }
    if d.itemState & ODS_FOCUS != 0 {
        InflateRect(&mut r, -4, -4);
        DrawFocusRect(d.hDC, &r);
    }
}
pub unsafe fn modern(h: HWND) {
    #[link(name = "dwmapi", kind = "raw-dylib")]
    extern "system" {
        fn DwmSetWindowAttribute(h: HWND, key: u32, data: *const c_void, size: u32) -> i32;
    }
    let corner = 2u32;
    DwmSetWindowAttribute(h, 33, (&corner as *const u32).cast(), 4);
    DwmSetWindowAttribute(h, 35, (&WHITE as *const u32).cast(), 4);
    DwmSetWindowAttribute(h, 36, (&INK as *const u32).cast(), 4);
}
pub unsafe fn buffered<F: FnOnce(HDC)>(h: HWND, provided: Option<HDC>, color: u32, draw: F) {
    let mut ps = PAINTSTRUCT::default();
    let dc = provided.unwrap_or_else(|| BeginPaint(h, &mut ps));
    let mut r = RECT::default();
    GetClientRect(h, &mut r);
    let mem = CreateCompatibleDC(dc);
    let bitmap = CreateCompatibleBitmap(dc, r.right.max(1), r.bottom.max(1));
    let old = SelectObject(mem, bitmap);
    fill(mem, r, color);
    draw(mem);
    BitBlt(dc, 0, 0, r.right, r.bottom, mem, 0, 0, SRCCOPY);
    SelectObject(mem, old);
    DeleteObject(bitmap);
    DeleteDC(mem);
    if provided.is_none() {
        EndPaint(h, &ps);
    }
}
unsafe extern "system" fn edit_proc(
    h: HWND,
    msg: u32,
    wp: usize,
    lp: isize,
    _id: usize,
    _data: usize,
) -> isize {
    if msg == WM_NCCALCSIZE {
        let result = DefSubclassProc(h, msg, wp, lp);
        let r = if wp != 0 {
            &mut (*(lp as *mut NCCALCSIZE_PARAMS)).rgrc[0]
        } else {
            &mut *(lp as *mut RECT)
        };
        r.top += 6;
        r.bottom -= 6;
        r.left += 1;
        r.right -= 1;
        return result;
    }
    if msg == WM_NCPAINT {
        let mut r = RECT::default();
        GetWindowRect(h, &mut r);
        let dc = GetWindowDC(h);
        let mut client = RECT::default();
        GetClientRect(h, &mut client);
        let mut p = POINT::default();
        ClientToScreen(h, &mut p);
        ExcludeClipRect(
            dc,
            p.x - r.left,
            p.y - r.top,
            p.x - r.left + client.right,
            p.y - r.top + client.bottom,
        );
        let r = RECT {
            left: 0,
            top: 0,
            right: r.right - r.left,
            bottom: r.bottom - r.top,
        };
        fill(dc, r, WHITE);
        rounded(
            dc,
            r,
            WHITE,
            if GetFocus() == h { BLUE } else { BORDER },
            8.,
        );
        ReleaseDC(h, dc);
        return 0;
    }
    let result = DefSubclassProc(h, msg, wp, lp);
    if msg == WM_SETFOCUS || msg == WM_KILLFOCUS {
        RedrawWindow(h, null(), null_mut(), RDW_INVALIDATE | RDW_FRAME);
    }
    if msg == WM_NCDESTROY {
        RemoveWindowSubclass(h, Some(edit_proc), 1);
    }
    result
}
pub unsafe fn style_edit(h: HWND) {
    SetWindowSubclass(h, Some(edit_proc), 1, 0);
    SendMessageW(h, EM_SETMARGINS as u32, 3, 10 | (10 << 16));
    SetWindowPos(
        h,
        null_mut(),
        0,
        0,
        0,
        0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
    );
}
