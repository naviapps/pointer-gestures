// Exercise the production wire adapter without changing how its cdylib is built.
#[path = "../src/input.rs"]
pub mod input;
#[path = "../src/overlay.rs"]
mod overlay;
#[path = "../src/trace.rs"]
pub mod trace;

use overlay::*;
use pointer_gesture_rendering::{Direction, Point};
use std::ptr::null;
use trace::*;

struct Frame(*mut OverlayFrame);

impl Frame {
    fn new(frame: *mut OverlayFrame) -> Self {
        assert!(!frame.is_null());
        Self(frame)
    }

    fn assert_view(&self, changed: u32, deadline: Option<u64>, endpoint: Option<f64>) {
        let view = unsafe { pointer_gesture_rendering_overlay_frame_view(self.0) };
        assert_eq!(view.changed, changed);
        assert_eq!(view.deadline_present, u32::from(deadline.is_some()));
        assert_eq!(view.deadline_ns, deadline.unwrap_or(0));
        if let Some(x) = endpoint {
            assert!(!view.trace.is_null());
            let trace = unsafe { pointer_gesture_rendering_trace_view(view.trace) };
            assert_eq!(trace.count, 1);
            assert_eq!(trace.point_count, 2);
            assert_eq!(trace.tail_present, 1);
            assert!(!trace.directions.is_null() && !trace.points.is_null());
            // Both buffers are borrowed from this live frame.
            assert_eq!(unsafe { *trace.directions }, Direction::Right as u32);
            assert_eq!(unsafe { *trace.points.add(1) }, Point { x, y: 0.0 });
            assert_eq!(trace.tail, Point { x, y: 1.0 });
        } else {
            assert!(view.trace.is_null());
        }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        unsafe { pointer_gesture_rendering_overlay_frame_destroy(self.0) };
    }
}

fn trace_at(x: f64) -> *mut TraceHandle {
    let directions = [Direction::Right as u32];
    let points = [Point { x: 0.0, y: 0.0 }, Point { x, y: 0.0 }];
    let trace = unsafe {
        pointer_gesture_rendering_trace_create(
            directions.as_ptr(),
            directions.len(),
            points.as_ptr(),
            points.len(),
            Point { x, y: 1.0 },
            1,
        )
    };
    assert!(!trace.is_null());
    trace
}

#[test]
fn retained_frames_publish_final_geometry_and_its_hide_deadline_together() {
    let timing = pointer_gesture_rendering_overlay_configuration(0.01, 24.0);
    let overlay = unsafe { pointer_gesture_rendering_overlay_create(timing, null(), 0) };
    assert!(!overlay.is_null());
    let initial = trace_at(1.0);
    let next = trace_at(2.0);

    let shown =
        Frame::new(unsafe { pointer_gesture_rendering_overlay_update(overlay, initial, 0) });
    let queued = Frame::new(unsafe { pointer_gesture_rendering_overlay_update(overlay, next, 1) });
    queued.assert_view(0, Some(10_000_000), Some(1.0));
    let returned =
        Frame::new(unsafe { pointer_gesture_rendering_overlay_update(overlay, initial, 2) });
    returned.assert_view(0, None, Some(1.0));
    let elapsed =
        Frame::new(unsafe { pointer_gesture_rendering_overlay_tick(overlay, 10_000_000) });
    elapsed.assert_view(0, None, Some(1.0));
    unsafe { pointer_gesture_rendering_trace_destroy(initial) };
    let queued =
        Frame::new(unsafe { pointer_gesture_rendering_overlay_update(overlay, next, 10_000_001) });
    queued.assert_view(1, None, Some(2.0));
    let last = trace_at(3.0);
    let queued =
        Frame::new(unsafe { pointer_gesture_rendering_overlay_update(overlay, last, 10_000_002) });
    queued.assert_view(0, Some(20_000_001), Some(2.0));
    unsafe { pointer_gesture_rendering_trace_destroy(last) };
    let final_frame = Frame::new(unsafe {
        pointer_gesture_rendering_overlay_update(overlay, null(), 10_000_003)
    });
    final_frame.assert_view(1, Some(30_000_003), Some(3.0));

    let timing = pointer_gesture_rendering_overlay_configuration(0.02, 24.0);
    let configured = Frame::new(unsafe {
        pointer_gesture_rendering_overlay_configure(overlay, timing, 10_000_004)
    });
    configured.assert_view(0, Some(50_000_003), Some(3.0));
    let early = Frame::new(unsafe { pointer_gesture_rendering_overlay_tick(overlay, 50_000_002) });
    early.assert_view(0, Some(50_000_003), Some(3.0));
    let hidden = Frame::new(unsafe { pointer_gesture_rendering_overlay_tick(overlay, 50_000_003) });
    hidden.assert_view(1, None, None);

    let shown_again =
        Frame::new(unsafe { pointer_gesture_rendering_overlay_update(overlay, next, 60_000_000) });
    shown_again.assert_view(1, None, Some(2.0));
    let newest = trace_at(4.0);
    let queued = Frame::new(unsafe {
        pointer_gesture_rendering_overlay_update(overlay, newest, 60_000_001)
    });
    queued.assert_view(0, Some(80_000_000), Some(2.0));
    unsafe { pointer_gesture_rendering_trace_destroy(newest) };
    let cleared =
        Frame::new(unsafe { pointer_gesture_rendering_overlay_clear(overlay, 60_000_002) });
    cleared.assert_view(1, None, None);
    let expired = Frame::new(unsafe { pointer_gesture_rendering_overlay_tick(overlay, u64::MAX) });
    expired.assert_view(0, None, None);
    unsafe { pointer_gesture_rendering_trace_destroy(next) };
    unsafe { pointer_gesture_rendering_overlay_destroy(overlay) };

    shown.assert_view(1, None, Some(1.0));
    final_frame.assert_view(1, Some(30_000_003), Some(3.0));
}
