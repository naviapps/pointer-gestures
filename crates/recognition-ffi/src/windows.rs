//! Windows-only ownership boundary; the portable C ABI remains unchanged.

pub const POINTER_GESTURE_RECOGNITION_WINDOWS_ABI_VERSION: u32 = 3u32;

#[cfg(windows)]
#[path = "windows/implementation.rs"]
mod implementation;
