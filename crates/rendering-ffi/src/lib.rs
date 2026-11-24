//! C ABI v8. Raw buffers and ownership remain within this crate.
mod geometry;
mod input;
mod overlay;
mod trace;
#[cfg(windows)]
/// cbindgen:ignore
mod windows;

pub use geometry::*;
pub use input::{pointer_gesture_rendering_finite_or_zero, pointer_gesture_rendering_non_negative};
pub use overlay::*;
pub use trace::*;

pub const POINTER_GESTURE_RENDERING_ABI_VERSION: u32 = 8u32;

#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_rendering_abi_version() -> u32 {
    POINTER_GESTURE_RENDERING_ABI_VERSION
}

/// Clamp negative elapsed components, truncate fractions and saturate overflow.
#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_rendering_elapsed_nanoseconds(
    seconds: i64,
    attoseconds: i64,
) -> u64 {
    pointer_gesture_rendering::elapsed_nanoseconds(seconds, attoseconds)
}
