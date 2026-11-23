use pointer_gesture_recognition::{Startup, validate_retry_delays};

#[repr(C)]
pub struct StartupView {
    pub revision: u64,
    pub lifecycle: u32,
    pub requested: u32,
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_validate_retry_delays(
    delays: *const f64,
    count: usize,
) -> usize {
    let delays = if count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(delays, count) }
    };
    validate_retry_delays(delays).map_or_else(|error| error.0, |()| usize::MAX)
}
#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_recognition_startup_create() -> *mut Startup {
    Box::into_raw(Box::default())
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_startup_destroy(handle: *mut Startup) {
    if !handle.is_null() {
        unsafe {
            drop(Box::from_raw(handle));
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_startup_view(
    handle: *const Startup,
) -> StartupView {
    let state = unsafe { &*handle };
    StartupView {
        revision: state.revision(),
        lifecycle: state.lifecycle() as u32,
        requested: u32::from(state.requested()),
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_startup_request(
    handle: *mut Startup,
    kind: u32,
    revision: u64,
) -> u32 {
    let state = unsafe { &mut *handle };
    u32::from(match kind {
        1 => state.start(),
        2 => state.stop(),
        3 => state.retry_now(),
        4 => state.retry(revision),
        5 => state.complete(revision, true),
        6 => state.complete(revision, false),
        _ => false,
    })
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_startup_set_delays(
    handle: *mut Startup,
    delays: *const f64,
    count: usize,
) -> u32 {
    let delays = if count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(delays, count) }
    };
    unsafe { &mut *handle }
        .set_delays(delays)
        .map_or(0, |changed| if changed { 2 } else { 1 })
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_startup_next_delay(
    handle: *mut Startup,
    delay_ns: *mut u64,
) -> u32 {
    let Some(delay) = unsafe { &mut *handle }.next_delay() else {
        return 0;
    };
    unsafe {
        *delay_ns = delay;
    }
    1
}
