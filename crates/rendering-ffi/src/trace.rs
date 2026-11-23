use crate::input::{buffer, directions};
use pointer_gesture_rendering::{Bounds, OverlayTrace, Point};

pub struct TraceHandle {
    pub(crate) trace: OverlayTrace,
}
impl TraceHandle {
    pub(crate) fn new(trace: OverlayTrace) -> Self {
        Self { trace }
    }
}
#[repr(C)]
pub struct TraceView {
    pub directions: *const u32,
    pub count: usize,
    pub points: *const Point,
    pub point_count: usize,
    pub tail: Point,
    pub tail_present: u32,
}
#[repr(C)]
pub struct TraceConversion {
    pub trace: *mut TraceHandle,
    pub reused_geometry: u32,
}

/// # Safety
/// Previous must be null or live. Visible snapshot buffers must be readable for
/// their counts during the call. The returned handle is independently owned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_trace_from_snapshot(
    previous: *const TraceHandle,
    visible: u32,
    data: *const u32,
    count: usize,
    points: *const Point,
    point_count: usize,
    tail: Point,
    tail_present: u32,
) -> TraceConversion {
    let converted = (|| {
        if visible != 1 || tail_present > 1 {
            return None;
        }
        OverlayTrace::from_snapshot(
            unsafe { previous.as_ref() }.map(|handle| &handle.trace),
            true,
            unsafe { directions(data, count) }.ok()?,
            unsafe { buffer(points, point_count) }.ok()?,
            (tail_present == 1).then_some(tail),
        )
        .ok()?
    })();
    let (trace, reused_geometry) =
        converted.map_or((std::ptr::null_mut(), 0), |(trace, reused)| {
            (
                Box::into_raw(Box::new(TraceHandle::new(trace))),
                u32::from(reused),
            )
        });
    TraceConversion {
        trace,
        reused_geometry,
    }
}

/// # Safety
/// Buffers must be readable for their counts during this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_trace_create(
    data: *const u32,
    count: usize,
    points: *const Point,
    point_count: usize,
    tail: Point,
    tail_present: u32,
) -> *mut TraceHandle {
    let trace = (|| {
        if tail_present > 1 {
            return None;
        }
        OverlayTrace::new(
            unsafe { directions(data, count) }.ok()?,
            unsafe { buffer(points, point_count) }.ok()?,
            (tail_present != 0).then_some(tail),
        )
        .ok()
    })();
    trace.map_or(std::ptr::null_mut(), |t| {
        Box::into_raw(Box::new(TraceHandle::new(t)))
    })
}
/// # Safety
/// Trace must be null or live/immutable. Returned handle is independently owned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_trace_with_tail(
    trace: *const TraceHandle,
    tail: Point,
    tail_present: u32,
) -> *mut TraceHandle {
    if tail_present > 1 {
        return std::ptr::null_mut();
    }
    unsafe { trace.as_ref() }
        .and_then(|t| t.trace.with_tail((tail_present != 0).then_some(tail)).ok())
        .map_or(std::ptr::null_mut(), |trace| {
            Box::into_raw(Box::new(TraceHandle::new(trace)))
        })
}
/// # Safety
/// Handle must be null or live and immutable for the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_trace_clone(
    trace: *const TraceHandle,
) -> *mut TraceHandle {
    unsafe { trace.as_ref() }.map_or(std::ptr::null_mut(), |t| {
        Box::into_raw(Box::new(TraceHandle::new(t.trace.clone())))
    })
}
/// # Safety
/// Handle must be null or live. Returned buffers borrow it until destruction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_trace_view(
    trace: *const TraceHandle,
) -> TraceView {
    match unsafe { trace.as_ref() } {
        Some(t) => TraceView {
            // Direction is repr(u32); expose only a read-only view of owned values.
            directions: t.trace.directions().as_ptr().cast(),
            count: t.trace.directions().len(),
            points: t.trace.points().as_ptr(),
            point_count: t.trace.points().len(),
            tail: t.trace.tail().unwrap_or(Point { x: 0.0, y: 0.0 }),
            tail_present: u32::from(t.trace.tail().is_some()),
        },
        None => TraceView {
            directions: std::ptr::null(),
            count: 0,
            points: std::ptr::null(),
            point_count: 0,
            tail: Point { x: 0.0, y: 0.0 },
            tail_present: 0,
        },
    }
}
/// # Safety
/// Handle must be null or exclusively owned and destroyed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_trace_destroy(trace: *mut TraceHandle) {
    if !trace.is_null() {
        drop(unsafe { Box::from_raw(trace) });
    }
}
/// # Safety
/// Handle must be null or live and immutable during this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_trace_intersects(
    trace: *const TraceHandle,
    viewport: Bounds,
    line_width: f64,
) -> u32 {
    u32::from(unsafe { trace.as_ref() }.is_some_and(|t| t.trace.intersects(viewport, line_width)))
}
