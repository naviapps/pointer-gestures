use super::matcher::MatcherHandle;
pub use pointer_gesture_recognition::Output as Frame;
pub use pointer_gesture_recognition::Point as ForeignPoint;
use pointer_gesture_recognition::{
    Configuration, Direction, Effect, Engine, Event as InputEvent, Point, Status, Trace,
};

#[repr(C)]
pub struct Config {
    pub start_distance: f64,
    pub change_distance: f64,
    pub total_ns: u64,
    pub idle_ns: u64,
    pub maximum_points: usize,
    pub button: u32,
    pub pass_empty: u32,
    pub maximum_directions: usize,
}
impl Config {
    pub(crate) fn decode(self) -> Option<Configuration> {
        (self.pass_empty <= 1).then_some(Configuration {
            start_distance: self.start_distance,
            change_distance: self.change_distance,
            total_duration_ns: self.total_ns,
            idle_duration_ns: self.idle_ns,
            maximum_points: self.maximum_points,
            maximum_directions: self.maximum_directions,
            button: self.button,
            passes_through_empty_matcher: self.pass_empty != 0,
        })
    }
}
#[repr(C)]
pub struct Event {
    pub kind: u32,
    pub button: u32,
    pub x: f64,
    pub y: f64,
    pub now_ns: u64,
    pub value: u32,
}
impl Event {
    fn decode(&self) -> Option<InputEvent> {
        let point = Point {
            x: self.x,
            y: self.y,
        };
        match self.kind {
            1 => Some(InputEvent::Down {
                button: self.button,
                point,
            }),
            2 => Some(InputEvent::Move {
                button: self.button,
                point,
            }),
            3 => Some(InputEvent::Up {
                button: self.button,
                point,
            }),
            4 => Some(InputEvent::Cancel),
            5 => Some(InputEvent::Stop),
            6 => Some(InputEvent::Start),
            7 => Some(InputEvent::Recording(self.value != 0)),
            8 => Some(InputEvent::Policy {
                enabled: self.value & 1 != 0,
                modifiers_satisfied: self.value & 2 != 0,
            }),
            9 => Some(InputEvent::Snapshot),
            10 => Some(InputEvent::Tick),
            _ => None,
        }
    }
}
#[repr(C)]
pub struct DirectionsView {
    data: *const u32,
    count: usize,
}
#[repr(C)]
pub struct PointsView {
    data: *const Point,
    count: usize,
}
#[repr(C)]
pub struct SessionReportView {
    present: u32,
    outcome: u32,
    captured: u32,
    recording: u32,
    rejected_direction: u32,
    generation: u64,
    elapsed_ns: u64,
    idle_ns: u64,
    point_count: u64,
    maximum_axis_distance: f64,
    directions: DirectionsView,
}

/// # Safety
/// A non-null frame is live. Returned direction bytes borrow that frame.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_frame_session_report(
    frame: *const Frame,
) -> SessionReportView {
    let report = unsafe { frame.as_ref() }.and_then(|frame| frame.last_session.as_ref());
    match report {
        Some(report) => SessionReportView {
            present: 1,
            outcome: report.outcome as u32,
            captured: u32::from(report.captured),
            recording: u32::from(report.recording),
            rejected_direction: report.rejected_direction.map_or(0, |d| d as u32),
            generation: report.generation,
            elapsed_ns: report.elapsed_ns,
            idle_ns: report.idle_ns,
            point_count: report.point_count,
            maximum_axis_distance: report.maximum_axis_distance,
            directions: directions(&report.directions),
        },
        None => SessionReportView {
            present: 0,
            outcome: 0,
            captured: 0,
            recording: 0,
            rejected_direction: 0,
            generation: 0,
            elapsed_ns: 0,
            idle_ns: 0,
            point_count: 0,
            maximum_axis_distance: 0.0,
            directions: directions(&[]),
        },
    }
}
#[repr(C)]
pub struct FrameView {
    disposition: u32,
    ready: u32,
    stage: u32,
    capturing: u32,
    recording: u32,
    visible: u32,
    failure: u32,
    input_generation: u64,
    discard_visible: u32,
    directions: DirectionsView,
    endpoints: PointsView,
    raw: PointsView,
    recorded: DirectionsView,
    tail_present: u32,
    tail: Point,
    effect_count: usize,
    deadline_present: u32,
    deadline_ns: u64,
}
#[repr(C)]
pub struct TraceView {
    visible: u32,
    directions: DirectionsView,
    endpoints: PointsView,
    raw: PointsView,
    tail_present: u32,
    tail: Point,
}
#[repr(C)]
pub struct EffectView {
    kind: u32,
    button: u32,
    match_value: u64,
    points: PointsView,
    trace: TraceView,
    ready: u32,
    capturing: u32,
    recording: u32,
    failure: u32,
    recorded: DirectionsView,
}
const ZERO: Point = Point { x: 0.0, y: 0.0 };
fn points(value: &[Point]) -> PointsView {
    PointsView {
        data: if value.is_empty() {
            std::ptr::null()
        } else {
            value.as_ptr()
        },
        count: value.len(),
    }
}
fn directions(value: &[Direction]) -> DirectionsView {
    DirectionsView {
        data: if value.is_empty() {
            std::ptr::null()
        } else {
            value.as_ptr().cast()
        },
        count: value.len(),
    }
}
fn failure(status: &Status) -> u32 {
    status.failure.map_or(0, |f| f as u32)
}
fn trace_view(trace: &Trace) -> TraceView {
    TraceView {
        visible: u32::from(trace.visible),
        directions: directions(&trace.directions),
        endpoints: points(&trace.endpoints),
        raw: points(&trace.raw_points),
        tail_present: u32::from(trace.tail.is_some()),
        tail: trace.tail.unwrap_or(ZERO),
    }
}
impl TraceView {
    fn empty() -> Self {
        Self {
            visible: 0,
            directions: directions(&[]),
            endpoints: points(&[]),
            raw: points(&[]),
            tail_present: 0,
            tail: ZERO,
        }
    }
}

impl EffectView {
    fn empty() -> Self {
        Self {
            kind: 0,
            button: 0,
            match_value: 0,
            points: points(&[]),
            trace: TraceView::empty(),
            ready: 0,
            capturing: 0,
            recording: 0,
            failure: 0,
            recorded: directions(&[]),
        }
    }
    fn from_effect(effect: &Effect) -> Self {
        let mut view = Self::empty();
        match effect {
            Effect::Click { button, point } => {
                view.kind = 1;
                view.button = *button;
                view.points = points(std::slice::from_ref(point));
            }
            Effect::Drag {
                button,
                points: value,
            } => {
                view.kind = 2;
                view.button = *button;
                view.points = points(value);
            }
            Effect::DragStart {
                button,
                points: value,
            } => {
                view.kind = 3;
                view.button = *button;
                view.points = points(value);
            }
            Effect::Release { button, point } => {
                view.kind = 4;
                view.button = *button;
                view.points = points(std::slice::from_ref(point));
            }
            Effect::Match(value) => {
                view.kind = 5;
                view.match_value = *value;
            }
            Effect::Publish { status, trace } => {
                view.kind = 6;
                view.trace = trace_view(trace);
                view.ready = u32::from(status.ready);
                view.capturing = u32::from(status.capturing);
                view.recording = u32::from(status.recording);
                view.failure = failure(status);
                view.recorded = directions(&status.recorded_directions);
            }
        }
        view
    }
}
fn frame_view(frame: &Frame) -> FrameView {
    FrameView {
        disposition: frame.disposition as u32,
        ready: u32::from(frame.status.ready),
        stage: frame.stage as u32,
        capturing: u32::from(frame.status.capturing),
        recording: u32::from(frame.status.recording),
        visible: u32::from(frame.trace.visible),
        failure: failure(&frame.status),
        input_generation: frame.input_generation,
        discard_visible: u32::from(frame.discard_pending_visible_trace),
        directions: directions(&frame.trace.directions),
        endpoints: points(&frame.trace.endpoints),
        raw: points(&frame.trace.raw_points),
        recorded: directions(&frame.status.recorded_directions),
        tail_present: u32::from(frame.trace.tail.is_some()),
        tail: frame.trace.tail.unwrap_or(ZERO),
        effect_count: frame.effects.len(),
        deadline_present: u32::from(frame.deadline_ns.is_some()),
        deadline_ns: frame.deadline_ns.unwrap_or(0),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_recognition_engine_create(config: Config) -> *mut Engine {
    config
        .decode()
        .and_then(|c| Engine::new(c).ok())
        .map_or(std::ptr::null_mut(), |e| Box::into_raw(Box::new(e)))
}
/// # Safety
/// The engine is live and exclusively accessed; button and empty-matcher policy are immutable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_engine_update_tuning(
    engine: *mut Engine,
    config: Config,
    now_ns: u64,
    reset_unchanged: u32,
) -> u32 {
    match config.decode().and_then(|c| {
        unsafe { &mut *engine }
            .update_tuning(c, now_ns, reset_unchanged != 0)
            .ok()
    }) {
        Some(false) => 0,
        Some(true) => 1,
        None => 2,
    }
}
/// # Safety
/// Destroy a null or live exclusive engine exactly once. This only frees memory.
/// Obtain Stop output and deliver recovery before tearing down a host input adapter.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_engine_destroy(engine: *mut Engine) {
    if !engine.is_null() {
        drop(unsafe { Box::from_raw(engine) });
    }
}
/// # Safety
/// The engine is live/exclusive. The caller owns and must free the returned frame.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_engine_step(
    engine: *mut Engine,
    event: Event,
) -> *mut Frame {
    let output = if event.kind == 11 {
        unsafe { &mut *engine }.prepare_replacement_press(event.now_ns)
    } else {
        let Some(input) = event.decode() else {
            return std::ptr::null_mut();
        };
        unsafe { &mut *engine }.step(input, event.now_ns)
    };
    Box::into_raw(Box::new(output))
}
/// # Safety
/// The engine is live/exclusive; the matcher is live and immutable for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_engine_use_matcher(
    engine: *mut Engine,
    matcher: *const MatcherHandle,
) {
    unsafe { &mut *engine }.use_matcher(unsafe { &*matcher });
}
/// # Safety
/// The frame is live. Returned immutable views borrow it until destruction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_frame_view(frame: *const Frame) -> FrameView {
    frame_view(unsafe { &*frame })
}
/// # Safety
/// The frame is live; effect views borrow the same frame, including single points.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_frame_effect(
    frame: *const Frame,
    index: usize,
) -> EffectView {
    unsafe { &*frame }
        .effects
        .get(index)
        .map_or_else(EffectView::empty, EffectView::from_effect)
}
/// # Safety
/// Destroy a null or live owned frame exactly once, after all readers finish.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_frame_destroy(frame: *mut Frame) {
    if !frame.is_null() {
        drop(unsafe { Box::from_raw(frame) });
    }
}

#[cfg(test)]
mod destruction_tests {
    use super::*;
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    struct CountingAllocator;
    thread_local! {
        static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
    }
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let _ = ALLOCATIONS.try_with(|count| {
                if let Some(value) = count.get() {
                    count.set(Some(value + 1));
                }
            });
            unsafe { System.alloc(layout) }
        }
        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            unsafe { System.dealloc(pointer, layout) }
        }
    }
    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;

    #[test]
    fn memory_only_engine_destruction_allocates_no_frames() {
        let mut measured = [0; 3];
        for (stage, measured_allocations) in measured.iter_mut().enumerate() {
            let engine = pointer_gesture_recognition_engine_create(Config {
                start_distance: 10.0,
                change_distance: 5.0,
                total_ns: 0,
                idle_ns: 0,
                maximum_points: 256,
                maximum_directions: 64,
                button: 1,
                pass_empty: 0,
            });
            assert!(!engine.is_null());
            unsafe {
                let event = |kind, x, value| Event {
                    kind,
                    button: 1,
                    x,
                    y: 0.0,
                    now_ns: 0,
                    value,
                };
                if stage > 0 {
                    pointer_gesture_recognition_frame_destroy(
                        pointer_gesture_recognition_engine_step(engine, event(6, 0.0, 0)),
                    );
                    pointer_gesture_recognition_frame_destroy(
                        pointer_gesture_recognition_engine_step(engine, event(7, 0.0, 1)),
                    );
                    pointer_gesture_recognition_frame_destroy(
                        pointer_gesture_recognition_engine_step(engine, event(1, 0.0, 0)),
                    );
                }
                if stage > 1 {
                    pointer_gesture_recognition_frame_destroy(
                        pointer_gesture_recognition_engine_step(engine, event(2, 40.0, 0)),
                    );
                }
                let saved = pointer_gesture_recognition_engine_step(engine, event(9, 0.0, 0));
                let view = pointer_gesture_recognition_frame_view(saved);
                ALLOCATIONS.with(|count| count.set(Some(0)));
                pointer_gesture_recognition_engine_destroy(engine);
                let allocations = ALLOCATIONS.with(|count| count.replace(None).unwrap());
                *measured_allocations = allocations;
                assert_eq!(view.stage as usize, stage);
                if stage == 2 {
                    assert_eq!(*view.directions.data, Direction::Right as u32);
                    assert_eq!((*view.endpoints.data.add(1)).x, 40.0);
                }
                pointer_gesture_recognition_frame_destroy(saved);
            }
        }
        assert_eq!(measured, [0; 3], "stopped/pending/capturing");
        unsafe { pointer_gesture_recognition_engine_destroy(std::ptr::null_mut()) };
    }
}
