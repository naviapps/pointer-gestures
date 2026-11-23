//! Portable geometry, presentation and optional CPU rasterization. Hosts own input,
//! coordinate transforms and opt into native window resources separately.
#![warn(missing_docs)]
mod overlay;
mod path;
mod pattern;
#[cfg(feature = "raster")]
pub mod raster;
mod trace;

pub use overlay::{OverlayState, OverlayTiming, OverlayUpdate};
pub use path::{
    Bounds, Command, CornerStyle, Geometry, GeometryError, Path, finite_or_zero, live_trace,
    non_negative,
};
pub use pattern::pattern;
pub use pointer_gesture_recognition::{Direction, Point, elapsed_nanoseconds};
pub use trace::OverlayTrace;
