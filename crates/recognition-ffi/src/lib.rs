//! C ABI v8. Numeric codes and borrowed views are confined to this boundary.
mod engine;
mod matcher;
mod startup;
#[cfg(windows)]
/// cbindgen:ignore
mod windows;
pub use engine::*;
pub use matcher::*;
use pointer_gesture_recognition::Point;

pub const POINTER_GESTURE_RECOGNITION_ABI_VERSION: u32 = 8u32;

/// Direction zero means equal axis deltas. Valid zero means non-finite input.
#[repr(C)]
pub struct Movement {
    pub valid: u32,
    pub direction: u32,
    pub maximum_axis_delta: f64,
}
#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_recognition_abi_version() -> u32 {
    POINTER_GESTURE_RECOGNITION_ABI_VERSION
}

/// Direction runs are bounded to 1..4096 and captured at button-down.
#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_recognition_validate_direction_limit(value: i64) -> u32 {
    u32::from(value > 0 && pointer_gesture_recognition::validate_direction_limit(value as usize))
}
/// Finite endpoints can yield an infinite maximum axis delta and a cardinal direction.
#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_recognition_movement(
    sx: f64,
    sy: f64,
    ex: f64,
    ey: f64,
) -> Movement {
    pointer_gesture_recognition::resolve_movement(
        pointer_gesture_recognition::Point { x: sx, y: sy },
        pointer_gesture_recognition::Point { x: ex, y: ey },
    )
    .map_or(
        Movement {
            valid: 0,
            direction: 0,
            maximum_axis_delta: 0.0,
        },
        |m| Movement {
            valid: 1,
            direction: m.direction.map_or(0, |d| d as u32),
            maximum_axis_delta: m.maximum_axis_delta,
        },
    )
}

/// Saturating ceil to nanoseconds. Non-positive or non-finite seconds return zero.
#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_recognition_duration_nanoseconds(seconds: f64) -> u64 {
    pointer_gesture_recognition::duration_nanoseconds(seconds)
}
/// First invalid field: start=1, change=2, total=3, idle=4, points=5, flags=6.
/// Duration flags are total=1 and idle=2. Absent durations are ignored.
#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_recognition_validate_tuning(
    start: f64,
    change: f64,
    total: f64,
    idle: f64,
    durations_present: u32,
    points: i64,
) -> u32 {
    if durations_present > 3 {
        return 6;
    }
    pointer_gesture_recognition::validate_tuning(
        start,
        change,
        (durations_present & 1 != 0).then_some(total),
        (durations_present & 2 != 0).then_some(idle),
        points.max(0) as usize,
    )
    .map_or_else(|error| error as u32, |()| 0)
}

#[unsafe(no_mangle)]
/// Sample a replay path uniformly with both endpoints and a minimum limit of two.
/// Returns the required count. A null output with zero capacity queries that count.
///
/// # Safety
/// When capacity suffices, input must contain count initialized points and output
/// must hold the returned point count. Buffers may not overlap. A size query
/// does not dereference either buffer, and empty input permits null points.
pub unsafe extern "C" fn pointer_gesture_recognition_replay_points(
    points: *const Point,
    count: usize,
    maximum: usize,
    output: *mut Point,
    capacity: usize,
) -> usize {
    let required = pointer_gesture_recognition::replay_point_count(count, maximum);
    if capacity < required || required == 0 {
        return required;
    }
    let points = unsafe { std::slice::from_raw_parts(points, count) };
    let sampled = pointer_gesture_recognition::replay_points(points, maximum);
    unsafe {
        std::ptr::copy_nonoverlapping(sampled.as_ptr(), output, required);
    }
    required
}

/// Clamp negative elapsed components, truncate fractions and saturate overflow.
#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_recognition_elapsed_nanoseconds(
    seconds: i64,
    attoseconds: i64,
) -> u64 {
    pointer_gesture_recognition::elapsed_nanoseconds(seconds, attoseconds)
}
