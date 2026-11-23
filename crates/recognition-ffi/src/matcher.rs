use pointer_gesture_recognition::{Direction, Matcher, PatternError, validate_pattern};

/// Opaque foreign handle. Clones retain the safe core's copy-on-write snapshot.
pub type MatcherHandle = Matcher;

#[repr(C)]
pub struct Lookup {
    pub(crate) present: u32,
    pub(crate) value: u64,
}
impl Lookup {
    fn from_option(value: Option<u64>) -> Self {
        Self {
            present: u32::from(value.is_some()),
            value: value.unwrap_or(0),
        }
    }
}

/// Foreign validation result: zero means valid; an absent index is u32::MAX.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PatternValidation {
    pub code: u32,
    pub index: u32,
}
impl PatternValidation {
    fn error(code: u32, index: Option<usize>) -> Self {
        Self {
            code,
            index: index.map_or(u32::MAX, |i| i as u32),
        }
    }
    fn from_result(result: Result<(), PatternError>) -> Self {
        match result {
            Ok(()) => Self::error(0, None),
            Err(PatternError::Empty) => Self::error(1, None),
            Err(PatternError::TooLong) => Self::error(2, None),
            Err(PatternError::RepeatedDirection { index }) => Self::error(3, Some(index)),
        }
    }
}

unsafe fn validated_pattern<'a>(
    data: *const u32,
    count: usize,
) -> Result<&'a [Direction], PatternValidation> {
    if count == 0 {
        return Err(PatternValidation::error(1, None));
    }
    if !pointer_gesture_recognition::validate_direction_limit(count) {
        return Err(PatternValidation::error(2, None));
    }
    if data.is_null() || !data.is_aligned() {
        return Err(PatternValidation::error(5, None));
    }
    let codes = unsafe { std::slice::from_raw_parts(data, count) };
    if let Some(index) = codes.iter().position(|&d| Direction::try_from(d).is_err()) {
        return Err(PatternValidation::error(4, Some(index)));
    }
    // Direction is repr(u32), and every code has been validated.
    Ok(unsafe { std::slice::from_raw_parts(data.cast::<Direction>(), count) })
}

/// # Safety
/// A non-null aligned buffer must be readable and stable for count elements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_pattern_validate(
    data: *const u32,
    count: usize,
) -> PatternValidation {
    match unsafe { validated_pattern(data, count) } {
        Ok(pattern) => PatternValidation::from_result(validate_pattern(pattern)),
        Err(error) => error,
    }
}

// The caller keeps the buffer stable for this call; no borrowed reference escapes.
unsafe fn pattern<'a>(data: *const u32, count: usize) -> Option<&'a [Direction]> {
    unsafe { validated_pattern(data, count) }.ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_recognition_matcher_create() -> *mut MatcherHandle {
    Box::into_raw(Box::new(Matcher::default()))
}

/// # Safety
/// The matcher is live and immutable during this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_matcher_clone(
    matcher: *const MatcherHandle,
) -> *mut MatcherHandle {
    Box::into_raw(Box::new(unsafe { &*matcher }.clone()))
}

/// # Safety
/// The matcher is null or an exclusively owned live handle, destroyed once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_matcher_destroy(matcher: *mut MatcherHandle) {
    if !matcher.is_null() {
        drop(unsafe { Box::from_raw(matcher) });
    }
}

/// # Safety
/// The matcher is live and exclusively accessed. The buffer is readable for count elements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_matcher_register(
    matcher: *mut MatcherHandle,
    data: *const u32,
    count: usize,
    value: u64,
) -> PatternValidation {
    match unsafe { validated_pattern(data, count) } {
        Ok(pattern) => {
            PatternValidation::from_result(unsafe { &mut *matcher }.register(pattern, value))
        }
        Err(error) => error,
    }
}

/// # Safety
/// The matcher is live/immutable. The buffer is readable for count elements.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_matcher_match(
    matcher: *const MatcherHandle,
    data: *const u32,
    count: usize,
) -> Lookup {
    Lookup::from_option(
        unsafe { pattern(data, count) }.and_then(|p| unsafe { &*matcher }.match_pattern(p)),
    )
}

/// # Safety
/// The matcher is live and immutable during this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_matcher_is_empty(
    matcher: *const MatcherHandle,
) -> u32 {
    u32::from(unsafe { &*matcher }.is_empty())
}

/// # Safety
/// The matcher is live/exclusive. The pattern buffer is readable during this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_matcher_remove(
    matcher: *mut MatcherHandle,
    data: *const u32,
    count: usize,
) -> Lookup {
    Lookup::from_option(
        unsafe { pattern(data, count) }.and_then(|p| unsafe { &mut *matcher }.remove(p)),
    )
}

/// # Safety
/// The matcher is live and exclusively accessed. Existing snapshots remain valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_recognition_matcher_clear(matcher: *mut MatcherHandle) {
    unsafe { &mut *matcher }.clear();
}
