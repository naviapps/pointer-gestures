//! Windows-specific resource ownership; portable geometry never creates windows.

pub const POINTER_GESTURE_RENDERING_WINDOWS_ABI_VERSION: u32 = 4u32;

#[cfg(windows)]
#[path = "windows/implementation.rs"]
mod implementation;
