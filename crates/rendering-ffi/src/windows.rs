//! Windows-specific resource ownership; portable geometry never creates windows.
use crate::trace::TraceHandle;
use pointer_gesture_rendering::{Bounds, CornerStyle, OverlayTiming};
use pointer_gesture_rendering_windows::{Configuration, Error, Overlay};

#[repr(C)]
pub struct WindowsOverlayConfiguration {
    rgba: u32,
    line_width: f64,
    radius_scale: f64,
    timing: OverlayTiming,
}
#[repr(C)]
#[derive(Default)]
pub struct WindowsOverlayError {
    kind: u32,
    system: u32,
}
#[repr(C)]
pub struct WindowsOverlayView {
    window: usize,
    visible: u32,
    bounds: Bounds,
}
fn status(result: Result<(), Error>, error: *mut WindowsOverlayError) -> u32 {
    let (ok, value) = match result {
        Ok(()) => (1, WindowsOverlayError::default()),
        Err(e) => (
            0,
            match e {
                Error::InvalidConfiguration => WindowsOverlayError { kind: 1, system: 0 },
                Error::Inactive => WindowsOverlayError { kind: 2, system: 0 },
                Error::System(system) => WindowsOverlayError { kind: 3, system },
                Error::WorkerStopped => WindowsOverlayError { kind: 4, system: 0 },
            },
        ),
    };
    if let Some(error) = unsafe { error.as_mut() } {
        *error = value;
    }
    ok
}
#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_rendering_windows_abi_version() -> u32 {
    4
}
/// # Safety
/// Error output is null or writable. A success returns one inactive owned overlay.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_windows_create(
    configuration: WindowsOverlayConfiguration,
    error: *mut WindowsOverlayError,
) -> *mut Overlay {
    match Overlay::new(configuration.into()) {
        Ok(overlay) => {
            status(Ok(()), error);
            Box::into_raw(Box::new(overlay))
        }
        Err(e) => {
            status(Err(e), error);
            std::ptr::null_mut()
        }
    }
}
impl From<WindowsOverlayConfiguration> for Configuration {
    fn from(value: WindowsOverlayConfiguration) -> Self {
        Self {
            color: value.rgba.to_be_bytes(),
            line_width: value.line_width,
            corners: CornerStyle {
                radius_scale: value.radius_scale,
            },
            timing: value.timing,
        }
    }
}
/// # Safety
/// Overlay is live/exclusive; error is null or writable. Configuration is copied.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_windows_configure(
    overlay: *mut Overlay,
    configuration: WindowsOverlayConfiguration,
    error: *mut WindowsOverlayError,
) -> u32 {
    status(
        unsafe { overlay.as_mut() }
            .ok_or(Error::InvalidConfiguration)
            .and_then(|o| o.set_configuration(configuration.into())),
        error,
    )
}
/// # Safety
/// Overlay is live and exclusively accessed; error is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_windows_activate(
    overlay: *mut Overlay,
    error: *mut WindowsOverlayError,
) -> u32 {
    status(
        unsafe { overlay.as_mut() }
            .ok_or(Error::InvalidConfiguration)
            .and_then(Overlay::activate),
        error,
    )
}
/// # Safety
/// Overlay is live/exclusive. Trace is null or live/immutable during this call.
/// Geometry is cloned, not retained by pointer. Error is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_windows_update(
    overlay: *mut Overlay,
    trace: *const TraceHandle,
    error: *mut WindowsOverlayError,
) -> u32 {
    status(
        unsafe { overlay.as_mut() }
            .ok_or(Error::InvalidConfiguration)
            .and_then(|o| {
                o.update(unsafe { trace.as_ref() }.map(|t| t.trace.clone()))
                    .map(|_| ())
            }),
        error,
    )
}
/// # Safety
/// Overlay is live and exclusively accessed; error is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_windows_clear(
    overlay: *mut Overlay,
    error: *mut WindowsOverlayError,
) -> u32 {
    status(
        unsafe { overlay.as_mut() }
            .ok_or(Error::InvalidConfiguration)
            .and_then(|o| o.clear().map(|_| ())),
        error,
    )
}
/// # Safety
/// Overlay is live/exclusive. View is writable/non-null; error is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_windows_view(
    overlay: *mut Overlay,
    view: *mut WindowsOverlayView,
    error: *mut WindowsOverlayError,
) -> u32 {
    let result = unsafe { overlay.as_mut() }
        .ok_or(Error::InvalidConfiguration)
        .and_then(|o| o.state())
        .and_then(|s| {
            let view = unsafe { view.as_mut() }.ok_or(Error::InvalidConfiguration)?;
            *view = WindowsOverlayView {
                window: s.window,
                visible: u32::from(s.visible),
                bounds: s.bounds.unwrap_or(Bounds {
                    min_x: 0.,
                    min_y: 0.,
                    max_x: 0.,
                    max_y: 0.,
                }),
            };
            Ok(())
        });
    status(result, error)
}
/// # Safety
/// Overlay is live and exclusively accessed; error is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_windows_deactivate(
    overlay: *mut Overlay,
    error: *mut WindowsOverlayError,
) -> u32 {
    status(
        unsafe { overlay.as_mut() }
            .ok_or(Error::InvalidConfiguration)
            .and_then(Overlay::deactivate),
        error,
    )
}
/// # Safety
/// Overlay is null or exclusively owned and destroyed exactly once. Joins its worker.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_windows_destroy(overlay: *mut Overlay) {
    if !overlay.is_null() {
        drop(unsafe { Box::from_raw(overlay) });
    }
}
