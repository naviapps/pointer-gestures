//! C ABI v8. Numeric codes and borrowed views are confined to this boundary.
mod engine;
mod matcher;
mod startup;
#[cfg(windows)]
mod windows;
pub use engine::*;
pub use matcher::*;

#[repr(C)]
pub struct Movement {
    pub valid: u32,
    pub direction: u32,
    pub maximum_axis_delta: f64,
}
#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_recognition_abi_version() -> u32 {
    8
}

#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_recognition_validate_direction_limit(value: i64) -> u32 {
    u32::from(value > 0 && pointer_gesture_recognition::validate_direction_limit(value as usize))
}
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

#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_recognition_duration_nanoseconds(seconds: f64) -> u64 {
    pointer_gesture_recognition::duration_nanoseconds(seconds)
}
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
/// Sample replay points into an exclusive caller-owned output buffer.
///
/// # Safety
/// When capacity suffices, input must contain count initialized points and output
/// must hold the returned point count. Buffers may not overlap. A size query does
/// not dereference either buffer.
pub unsafe extern "C" fn pointer_gesture_recognition_replay_points(
    points: *const ForeignPoint,
    count: usize,
    maximum: usize,
    output: *mut ForeignPoint,
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

#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_recognition_elapsed_nanoseconds(
    seconds: i64,
    attoseconds: i64,
) -> u64 {
    pointer_gesture_recognition::elapsed_nanoseconds(seconds, attoseconds)
}
