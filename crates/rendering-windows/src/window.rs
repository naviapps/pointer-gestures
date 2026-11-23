use crate::desktop::{WindowState, system};
use crate::{Configuration, Error};
use pointer_gesture_rendering::{Bounds, Command, OverlayTrace, Point, live_trace, raster};
use std::{
    ptr::{null, null_mut},
    sync::atomic::{AtomicU64, Ordering},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{HiDpi::*, WindowsAndMessaging::*},
};

pub(super) const REFRESH_LAYOUT: u32 = WM_APP + 0x50;

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_NCHITTEST => HTTRANSPARENT as LRESULT,
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_DISPLAYCHANGE | WM_DPICHANGED => {
            // Coordinates are already physical pixels: do not apply the suggested
            // logical resize. Reclip/repaint after this native callback returns.
            unsafe {
                PostMessageW(window, REFRESH_LAYOUT, 0, 0);
            }
            0
        }
        WM_CLOSE => 0,
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}
pub(super) struct Resources {
    window: HWND,
    class: Vec<u16>,
    instance: HINSTANCE,
    bounds: Option<Bounds>,
}
impl Resources {
    pub(super) fn new() -> Result<Self, Error> {
        if unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) }
            .is_null()
        {
            return Err(system());
        }
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let class: Vec<u16> = format!(
            "NaviPointerGestureOverlay-{}-{:x}-{}\0",
            std::process::id(),
            std::ptr::addr_of!(NEXT) as usize,
            NEXT.fetch_add(1, Ordering::Relaxed)
        )
        .encode_utf16()
        .collect();
        let instance = unsafe { GetModuleHandleW(null()) };
        if instance.is_null() {
            return Err(system());
        }
        let definition = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class.as_ptr(),
            ..Default::default()
        };
        if unsafe { RegisterClassW(&definition) } == 0 {
            return Err(system());
        }
        let window = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED
                    | WS_EX_TRANSPARENT
                    | WS_EX_TOOLWINDOW
                    | WS_EX_NOACTIVATE
                    | WS_EX_TOPMOST,
                class.as_ptr(),
                class.as_ptr(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                null_mut(),
                null_mut(),
                instance,
                null(),
            )
        };
        if window.is_null() {
            let error = system();
            unsafe {
                UnregisterClassW(class.as_ptr(), instance);
            }
            return Err(error);
        }
        Ok(Self {
            window,
            class,
            instance,
            bounds: None,
        })
    }
    pub(super) fn state(&self) -> WindowState {
        WindowState {
            window: self.window as usize,
            visible: self.bounds.is_some(),
            bounds: self.bounds,
        }
    }
    pub(super) fn hide(&mut self) {
        unsafe {
            ShowWindow(self.window, SW_HIDE);
        }
        self.bounds = None;
    }
    pub(super) fn present(
        &mut self,
        trace: &OverlayTrace,
        configuration: Configuration,
    ) -> Result<(), Error> {
        let path = live_trace(
            trace.directions(),
            trace.points(),
            trace.tail(),
            configuration.corners,
        )
        .map_err(|_| Error::InvalidConfiguration)?;
        let mut points = Vec::with_capacity(path.commands.len() * 3);
        for command in &path.commands {
            match *command {
                Command::Move(p) | Command::Line(p) => points.push(p),
                Command::Cubic {
                    control1,
                    control2,
                    to,
                } => points.extend([control1, control2, to]),
                Command::Close => {}
            }
        }
        let Some(mut bounds) = Bounds::from_points(&points) else {
            self.hide();
            return Ok(());
        };
        let padding = configuration.line_width / 2. + 2.;
        let x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) } as f64;
        let y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) } as f64;
        let right = x + unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) } as f64;
        let bottom = y + unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) } as f64;
        bounds.min_x = (bounds.min_x - padding).floor().max(x);
        bounds.min_y = (bounds.min_y - padding).floor().max(y);
        bounds.max_x = (bounds.max_x + padding).ceil().min(right);
        bounds.max_y = (bounds.max_y + padding).ceil().min(bottom);
        if bounds.max_x <= bounds.min_x || bounds.max_y <= bounds.min_y {
            self.hide();
            return Ok(());
        }
        let image = raster::stroke(
            &path,
            (bounds.max_x - bounds.min_x) as u32,
            (bounds.max_y - bounds.min_y) as u32,
            Point {
                x: bounds.min_x,
                y: bounds.min_y,
            },
            1.,
            configuration.line_width,
            configuration.color,
        )
        .map_err(|_| Error::InvalidConfiguration)?;
        let width = image.width() as i32;
        let height = image.height() as i32;
        let pixels = image.into_bgra();
        let surface = Surface::new(width, height, &pixels)?;
        let position = POINT {
            x: bounds.min_x as i32,
            y: bounds.min_y as i32,
        };
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        if unsafe {
            UpdateLayeredWindow(
                self.window,
                null_mut(),
                &position,
                &size,
                surface.dc,
                &POINT { x: 0, y: 0 },
                0,
                &blend,
                ULW_ALPHA,
            )
        } == 0
        {
            return Err(system());
        }
        if unsafe {
            SetWindowPos(
                self.window,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        } == 0
        {
            return Err(system());
        }
        self.bounds = Some(bounds);
        Ok(())
    }
}
impl Drop for Resources {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.window);
            UnregisterClassW(self.class.as_ptr(), self.instance);
        }
    }
}
struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}
impl Surface {
    fn new(width: i32, height: i32, pixels: &[u8]) -> Result<Self, Error> {
        let dc = unsafe { CreateCompatibleDC(null_mut()) };
        if dc.is_null() {
            return Err(system());
        }
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits = null_mut();
        let bitmap =
            unsafe { CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0) };
        if bitmap.is_null() || bits.is_null() {
            let error = system();
            unsafe {
                if !bitmap.is_null() {
                    DeleteObject(bitmap);
                }
                DeleteDC(dc);
            }
            return Err(error);
        }
        let previous = unsafe { SelectObject(dc, bitmap) };
        if previous.is_null() || previous as isize == -1 {
            let error = system();
            unsafe {
                DeleteObject(bitmap);
                DeleteDC(dc);
            }
            return Err(error);
        }
        // The top-down 32-bit DIB has exactly width*height*4 writable bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast(), pixels.len());
        }
        Ok(Self {
            dc,
            bitmap,
            previous,
        })
    }
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}
