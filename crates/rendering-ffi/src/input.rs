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
pub(crate) unsafe fn directions(
    data: *const u32,
    count: usize,
) -> std::result::Result<Vec<Direction>, GeometryError> {
    unsafe { buffer(data, count) }?
        .iter()
        .map(|&value| Direction::try_from(value).map_err(|_| GeometryError::InvalidDirection))
        .collect()
}

#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_rendering_non_negative(value: f64) -> f64 {
    pointer_gesture_rendering::non_negative(value)
}

#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_rendering_finite_or_zero(value: f64) -> f64 {
    pointer_gesture_rendering::finite_or_zero(value)
}
