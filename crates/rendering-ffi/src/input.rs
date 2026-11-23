use pointer_gesture_rendering::{Direction, GeometryError};

// The caller guarantees that a non-null input describes a readable allocation for
// this call. Count, null and alignment checks prevent structural buffer mistakes.
pub(crate) unsafe fn buffer<'a, T>(
    data: *const T,
    count: usize,
) -> std::result::Result<&'a [T], GeometryError> {
    if count == 0 {
        return Ok(&[]);
    }
    if data.is_null() || !data.is_aligned() || count > isize::MAX as usize / size_of::<T>() {
        return Err(GeometryError::InvalidBuffer);
    }
    Ok(unsafe { std::slice::from_raw_parts(data, count) })
}
pub(crate) unsafe fn directions<'a>(
    data: *const u32,
    count: usize,
) -> Result<&'a [Direction], GeometryError> {
    let codes = unsafe { buffer(data, count) }?;
    if codes
        .iter()
        .any(|&value| Direction::try_from(value).is_err())
    {
        return Err(GeometryError::InvalidDirection);
    }
    // Direction is repr(u32). The caller keeps the validated codes stable for
    // this call. The slice uses the non-null pointer from `codes` for empty input.
    Ok(unsafe { std::slice::from_raw_parts(codes.as_ptr().cast::<Direction>(), count) })
}

#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_rendering_non_negative(value: f64) -> f64 {
    pointer_gesture_rendering::non_negative(value)
}

#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_rendering_finite_or_zero(value: f64) -> f64 {
    pointer_gesture_rendering::finite_or_zero(value)
}
