use pointer_gesture_recognition::{Startup, validate_retry_delays};

/// Lifecycle values: idle=0, starting=1, ready=2, retrying=3, failed=4.
#[repr(C)]
pub struct StartupView {
    pub revision: u64,
    pub lifecycle: u32,
    pub requested: u32,
}
/// Return SIZE_MAX on success or the first invalid zero-based index.
/// Null delays are allowed only when count is zero.
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
/// Allocate a host-neutral source-start lifecycle. It owns policy, revisions and
/// retry selection. The host owns OS resources, sleeps and callbacks.
/// Serialize access to the handle.
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
/// Kinds: start=1, stop=2, retry-now=3, scheduled-retry=4, success=5, failure=6.
/// Kinds 4..6 require the attempt's revision. Return 1 when accepted and 0 for
/// stale or unknown requests.
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
/// Return 0 for invalid delays, 1 when accepted, or 2 when accepted and pending
/// work was replaced. Invalid values leave the current delays intact.
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
/// Return 1 and write a delay while retrying, or return 0 otherwise.
/// Zero delay means yield. Each call advances the schedule, which repeats its
/// last delay for further failures.
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
