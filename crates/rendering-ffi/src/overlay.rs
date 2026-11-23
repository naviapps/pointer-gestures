use crate::trace::TraceHandle;
use pointer_gesture_rendering::{OverlayState, OverlayTiming, OverlayUpdate};

pub struct OverlayFrame {
    trace: Option<TraceHandle>,
    changed: bool,
    deadline: Option<u64>,
}
impl From<OverlayUpdate> for OverlayFrame {
    fn from(update: OverlayUpdate) -> Self {
        Self {
            trace: update.trace.map(TraceHandle::new),
            changed: update.changed,
            deadline: update.deadline_ns,
        }
    }
}
#[repr(C)]
pub struct OverlayView {
    pub trace: *const TraceHandle,
    pub changed: u32,
    pub deadline_present: u32,
    pub deadline_ns: u64,
}

#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_rendering_overlay_configuration(
    interval_seconds: f64,
    maximum_point_lag: f64,
) -> OverlayTiming {
    OverlayTiming::new(interval_seconds, maximum_point_lag)
}
/// # Safety
/// Initial trace is null (hidden) or live; it is cloned, never retained by pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_overlay_create(
    configuration: OverlayTiming,
    trace: *const TraceHandle,
    now_ns: u64,
) -> *mut OverlayState {
    Box::into_raw(Box::new(OverlayState::new(
        configuration,
        unsafe { trace.as_ref() }.map(|t| t.trace.clone()),
        now_ns,
    )))
}
/// # Safety
/// OverlayState must be live and exclusively accessed; trace is null or live/immutable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_overlay_update(
    overlay: *mut OverlayState,
    trace: *const TraceHandle,
    now_ns: u64,
) -> *mut OverlayFrame {
    unsafe { overlay.as_mut() }.map_or(std::ptr::null_mut(), |o| {
        Box::into_raw(Box::new(
            o.update(unsafe { trace.as_ref() }.map(|t| t.trace.clone()), now_ns)
                .into(),
        ))
    })
}
/// # Safety
/// OverlayState must be live and exclusively accessed. The frame is newly owned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_overlay_clear(
    overlay: *mut OverlayState,
    now_ns: u64,
) -> *mut OverlayFrame {
    unsafe { overlay.as_mut() }.map_or(std::ptr::null_mut(), |o| {
        Box::into_raw(Box::new(o.clear(now_ns).into()))
    })
}
/// # Safety
/// OverlayState must be live and exclusively accessed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_overlay_tick(
    overlay: *mut OverlayState,
    now_ns: u64,
) -> *mut OverlayFrame {
    unsafe { overlay.as_mut() }.map_or(std::ptr::null_mut(), |o| {
        Box::into_raw(Box::new(o.tick(now_ns).into()))
    })
}
/// # Safety
/// OverlayState must be live and exclusively accessed. The frame is newly owned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_overlay_configure(
    overlay: *mut OverlayState,
    configuration: OverlayTiming,
    now_ns: u64,
) -> *mut OverlayFrame {
    unsafe { overlay.as_mut() }.map_or(std::ptr::null_mut(), |o| {
        Box::into_raw(Box::new(o.reconfigure(configuration, now_ns).into()))
    })
}
/// # Safety
/// OverlayState must be null or exclusively owned and destroyed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_overlay_destroy(overlay: *mut OverlayState) {
    if !overlay.is_null() {
        drop(unsafe { Box::from_raw(overlay) });
    }
}
/// # Safety
/// Frame must be null or live. Its trace view is borrowed until frame destruction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_overlay_frame_view(
    frame: *const OverlayFrame,
) -> OverlayView {
    unsafe { frame.as_ref() }.map_or(
        OverlayView {
            trace: std::ptr::null(),
            changed: 0,
            deadline_present: 0,
            deadline_ns: 0,
        },
        |f| OverlayView {
            trace: f.trace.as_ref().map_or(std::ptr::null(), |t| t as *const _),
            changed: u32::from(f.changed),
            deadline_present: u32::from(f.deadline.is_some()),
            deadline_ns: f.deadline.unwrap_or(0),
        },
    )
}
/// # Safety
/// Frame must be null or exclusively owned and destroyed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_overlay_frame_destroy(frame: *mut OverlayFrame) {
    if !frame.is_null() {
        drop(unsafe { Box::from_raw(frame) });
    }
}
