//! C ABI v8. Raw buffers and ownership remain within this crate.
mod geometry;
mod input;
mod overlay;
mod trace;
#[cfg(windows)]
mod windows;

pub use geometry::*;
pub use input::{pointer_gesture_rendering_finite_or_zero, pointer_gesture_rendering_non_negative};
pub use overlay::*;
pub use trace::*;

#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_rendering_abi_version() -> u32 {
    8
}

#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_rendering_elapsed_nanoseconds(
    seconds: i64,
    attoseconds: i64,
) -> u64 {
    pointer_gesture_rendering::elapsed_nanoseconds(seconds, attoseconds)
}
