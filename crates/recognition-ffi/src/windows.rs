//! Windows-only ownership boundary; the portable C ABI remains unchanged.
use super::{Config, Frame, MatcherHandle};
use pointer_gesture_recognition::Point;
use pointer_gesture_recognition_windows::{
    ContextReadResult, Error, Policy, PressContext, Recognizer, SessionContext,
};
use std::os::windows::io::IntoRawHandle;

#[repr(C)]
#[derive(Default)]
pub struct WindowsError {
    kind: u32,
    system: u32,
    sent: u32,
    expected: u32,
}

#[repr(C)]
pub struct WindowsPressContext {
    point: Point,
    recipient_process_id: u32,
    foreground_process_id: u32,
}

#[repr(C)]
#[derive(Default)]
pub struct WindowsEpoch {
    value: u64,
    present: u32,
}

// Stack-owned during a single call into the foreign resolver.
pub struct ContextSelection(Option<Result<SessionContext, Error>>);
type ResolveContext = unsafe extern "C" fn(usize, WindowsPressContext, *mut ContextSelection);

fn resolver(
    callback: ResolveContext,
    user: usize,
) -> impl FnMut(PressContext) -> Result<SessionContext, Error> + Send {
    move |press| {
        let mut selection = ContextSelection(None);
        unsafe {
            callback(
                user,
                WindowsPressContext {
                    point: press.point,
                    recipient_process_id: press.recipient_process_id,
                    foreground_process_id: press.foreground_process_id.unwrap_or(0),
                },
                &mut selection,
            );
        }
        selection.0.unwrap_or(Err(Error::WorkerStopped))
    }
}

/// # Safety
/// The selection belongs to the current callback. Matcher is live and immutable
/// for this call. The matcher is cloned before returning.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_windows_select_context(
    selection: *mut ContextSelection,
    matcher: *const MatcherHandle,
    enabled: u32,
    modifiers: u32,
    epoch: u64,
) -> u32 {
    let Some(selection) = (unsafe { selection.as_mut() }) else {
        return 0;
    };
    let result = if selection.0.is_some() || enabled > 1 || modifiers > 15 {
        Err(Error::InvalidConfiguration)
    } else if let Some(matcher) = unsafe { matcher.as_ref() } {
        Ok(SessionContext {
            matcher: matcher.clone(),
            policy: Policy {
                enabled: enabled != 0,
                required_modifiers: modifiers,
            },
            epoch,
        })
    } else {
        Err(Error::InvalidConfiguration)
    };
    let accepted = u32::from(result.is_ok());
    selection.0 = Some(result);
    accepted
}

fn status(result: Result<(), Error>, error: *mut WindowsError) -> u32 {
    let (ok, value) = match result {
        Ok(()) => (1, WindowsError::default()),
        Err(error) => (
            0,
            match error {
                Error::InvalidConfiguration => WindowsError {
                    kind: 1,
                    ..Default::default()
                },
                Error::AlreadyRunning => WindowsError {
                    kind: 2,
                    ..Default::default()
                },
                Error::System(system) => WindowsError {
                    kind: 3,
                    system,
                    ..Default::default()
                },
                Error::Replay {
                    system,
                    sent,
                    expected,
                } => WindowsError {
                    kind: 4,
                    system,
                    sent,
                    expected,
                },
                Error::ConsumerLagged => WindowsError {
                    kind: 5,
                    ..Default::default()
                },
                Error::ReplayTargetChanged => WindowsError {
                    kind: 7,
                    ..Default::default()
                },
                Error::WorkerStopped => WindowsError {
                    kind: 6,
                    ..Default::default()
                },
            },
        ),
    };
    // All functions require a writable error output owned by the caller.
    unsafe {
        error.write(value);
    }
    ok
}

#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_recognition_windows_abi_version() -> u32 {
    3
}

/// # Safety
/// Matcher is live and immutable during the call; error is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_windows_start(
    config: Config,
    matcher: *const MatcherHandle,
    error: *mut WindowsError,
) -> *mut Recognizer {
    let result = config
        .decode()
        .ok_or(Error::InvalidConfiguration)
        .and_then(|config| Recognizer::start(config, unsafe { &*matcher }));
    start_result(result, error)
}

/// # Safety
/// Callback and user data are live until the resolver is replaced or the
/// adapter has stopped. Error is writable. Callback must return normally.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_windows_start_with_context(
    config: Config,
    callback: Option<ResolveContext>,
    user: usize,
    error: *mut WindowsError,
) -> *mut Recognizer {
    let result = config
        .decode()
        .zip(callback)
        .ok_or(Error::InvalidConfiguration)
        .and_then(|(config, callback)| {
            Recognizer::try_start_with_context(config, resolver(callback, user))
        });
    start_result(result, error)
}

fn start_result(result: Result<Recognizer, Error>, error: *mut WindowsError) -> *mut Recognizer {
    match result {
        Ok(adapter) => {
            status(Ok(()), error);
            Box::into_raw(Box::new(adapter))
        }
        Err(value) => {
            status(Err(value), error);
            std::ptr::null_mut()
        }
    }
}
/// # Safety
/// Adapter is live and exclusively accessed. Frame/error are writable outputs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_windows_read(
    adapter: *const Recognizer,
    frame: *mut *mut Frame,
    error: *mut WindowsError,
) -> u32 {
    let mut epoch = WindowsEpoch::default();
    unsafe {
        pointer_gesture_recognition_windows_read_with_context(adapter, frame, &mut epoch, error)
    }
}

/// # Safety
/// Adapter is live and exclusively accessed. Frame/epoch/error are writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_windows_read_with_context(
    adapter: *const Recognizer,
    frame: *mut *mut Frame,
    epoch: *mut WindowsEpoch,
    error: *mut WindowsError,
) -> u32 {
    unsafe {
        frame.write(std::ptr::null_mut());
        epoch.write(WindowsEpoch::default());
    }
    match unsafe { &*adapter }.try_read_with_context() {
        Ok(value) => {
            status(Ok(()), error);
            match value {
                ContextReadResult::Frame(output) => {
                    unsafe {
                        epoch.write(WindowsEpoch {
                            value: output.epoch.unwrap_or(0),
                            present: u32::from(output.epoch.is_some()),
                        });
                        frame.write(Box::into_raw(Box::new(output.output)));
                    }
                    1
                }
                ContextReadResult::Pending => 0,
                ContextReadResult::Completed => 3,
            }
        }
        Err(value) => {
            status(Err(value), error);
            2
        }
    }
}

/// # Safety
/// Adapter is exclusively accessed and error is writable. Both callback
/// lifetimes must cover this call; on failure keep them until stop joins.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_windows_set_context_resolver(
    adapter: *mut Recognizer,
    callback: Option<ResolveContext>,
    user: usize,
    error: *mut WindowsError,
) -> u32 {
    status(
        callback
            .ok_or(Error::InvalidConfiguration)
            .and_then(|callback| {
                unsafe { &mut *adapter }.set_fallible_context_resolver(resolver(callback, user))
            }),
        error,
    )
}
/// # Safety
/// Adapter is live/serialized; error is writable. Close the owned handle once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_windows_duplicate_notification_handle(
    adapter: *const Recognizer,
    error: *mut WindowsError,
) -> usize {
    match unsafe { &*adapter }.duplicate_notification_handle() {
        Ok(handle) => {
            status(Ok(()), error);
            handle.into_raw_handle() as usize
        }
        Err(value) => {
            status(Err(value), error);
            0
        }
    }
}
/// # Safety
/// Adapter is live; all adapter calls are serialized by the caller.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_windows_now_ns(
    adapter: *const Recognizer,
) -> u64 {
    unsafe { &*adapter }.now_ns()
}
/// # Safety
/// Adapter is exclusively accessed; matcher is live/immutable; error is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_windows_use_matcher(
    adapter: *mut Recognizer,
    matcher: *const MatcherHandle,
    error: *mut WindowsError,
) -> u32 {
    status(
        unsafe { &mut *adapter }.use_matcher(unsafe { &*matcher }),
        error,
    )
}
/// # Safety
/// Adapter is exclusively accessed; error is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_windows_set_policy(
    adapter: *mut Recognizer,
    enabled: u32,
    modifiers: u32,
    error: *mut WindowsError,
) -> u32 {
    status(
        if enabled > 1 {
            Err(Error::InvalidConfiguration)
        } else {
            unsafe { &mut *adapter }.set_policy(Policy {
                enabled: enabled != 0,
                required_modifiers: modifiers,
            })
        },
        error,
    )
}
/// # Safety
/// Adapter is exclusively accessed; error is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_windows_set_recording(
    adapter: *mut Recognizer,
    enabled: u32,
    error: *mut WindowsError,
) -> u32 {
    status(
        if enabled > 1 {
            Err(Error::InvalidConfiguration)
        } else {
            unsafe { &mut *adapter }.set_recording(enabled != 0)
        },
        error,
    )
}
/// # Safety
/// Adapter is exclusively accessed; error is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_windows_update_tuning(
    adapter: *mut Recognizer,
    config: Config,
    error: *mut WindowsError,
) -> u32 {
    status(
        config
            .decode()
            .ok_or(Error::InvalidConfiguration)
            .and_then(|c| unsafe { &mut *adapter }.update_tuning(c)),
        error,
    )
}
/// # Safety
/// Adapter is exclusively accessed; error is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_windows_cancel(
    adapter: *mut Recognizer,
    error: *mut WindowsError,
) -> u32 {
    status(unsafe { &mut *adapter }.cancel(), error)
}
/// # Safety
/// Adapter is exclusively accessed; error is writable. Stop is idempotent.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_windows_stop(
    adapter: *mut Recognizer,
    error: *mut WindowsError,
) -> u32 {
    status(unsafe { &mut *adapter }.stop(), error)
}
/// # Safety
/// Adapter is null or exclusively owned and destroyed exactly once. Stop/recovery
/// and joining run before memory is released; no host callback is made.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_windows_destroy(adapter: *mut Recognizer) {
    if !adapter.is_null() {
        drop(unsafe { Box::from_raw(adapter) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pointer_gesture_recognition::Direction;

    fn press() -> PressContext {
        PressContext {
            point: Point { x: 12.0, y: 34.0 },
            recipient_process_id: 567,
            foreground_process_id: None,
        }
    }

    #[test]
    fn callback_selection_copies_matcher_policy_and_zero_epoch() {
        struct CallbackState {
            matcher: *const MatcherHandle,
            press: Option<WindowsPressContext>,
            selected: u32,
        }
        unsafe extern "C" fn select(
            user: usize,
            press: WindowsPressContext,
            selection: *mut ContextSelection,
        ) {
            let state = unsafe { &mut *(user as *mut CallbackState) };
            state.press = Some(press);
            state.selected = unsafe {
                pointer_gesture_recognition_windows_select_context(
                    selection,
                    state.matcher,
                    1,
                    10,
                    0,
                )
            };
        }
        let mut matcher = MatcherHandle::default();
        matcher.register(&[Direction::Right], 91).unwrap();
        let mut state = CallbackState {
            matcher: std::ptr::from_ref(&matcher),
            press: None,
            selected: 0,
        };
        let result = resolver(select, std::ptr::from_mut(&mut state) as usize)(press()).unwrap();
        assert_eq!(state.selected, 1);
        let captured = state.press.unwrap();
        assert_eq!(captured.point, Point { x: 12.0, y: 34.0 });
        assert_eq!(captured.recipient_process_id, 567);
        assert_eq!(captured.foreground_process_id, 0);
        matcher.clear();
        assert_eq!(result.matcher.match_pattern(&[Direction::Right]), Some(91));
        assert!(result.policy.enabled);
        assert_eq!(result.policy.required_modifiers, 10);
        assert_eq!(result.epoch, 0);
    }

    #[test]
    fn missing_or_malformed_selection_returns_a_terminal_error() {
        unsafe extern "C" fn missing(_: usize, _: WindowsPressContext, _: *mut ContextSelection) {}
        assert!(matches!(
            resolver(missing, 0)(press()),
            Err(Error::WorkerStopped)
        ));

        let matcher = MatcherHandle::default();
        let mut selection = ContextSelection(None);
        assert_eq!(
            unsafe {
                pointer_gesture_recognition_windows_select_context(
                    &mut selection,
                    &matcher,
                    1,
                    16,
                    0,
                )
            },
            0
        );
        assert!(matches!(
            selection.0,
            Some(Err(Error::InvalidConfiguration))
        ));
        let mut selection = ContextSelection(None);
        assert_eq!(
            unsafe {
                pointer_gesture_recognition_windows_select_context(
                    &mut selection,
                    &matcher,
                    1,
                    0,
                    0,
                )
            },
            1
        );
        assert_eq!(
            unsafe {
                pointer_gesture_recognition_windows_select_context(
                    &mut selection,
                    &matcher,
                    1,
                    0,
                    0,
                )
            },
            0
        );
        assert!(matches!(
            selection.0,
            Some(Err(Error::InvalidConfiguration))
        ));
    }
}
