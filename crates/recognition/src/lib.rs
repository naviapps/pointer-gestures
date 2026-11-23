//! Pointer gesture recognition without OS capture, clocks or foreign exports.
//! Deliver returned effects in order and close engines explicitly to recover input.
#![warn(missing_docs)]
mod configuration;
mod engine;
mod event;
mod geometry;
mod matcher;
mod output;
mod replay;
mod session;
mod startup;

pub use configuration::*;
pub use engine::Engine;
pub use event::Event;
pub use geometry::{Direction, InvalidDirection, Movement, Point, resolve_movement};
pub use matcher::{Matcher, PatternError, validate_pattern};
pub use output::*;
pub use session::{SessionOutcome, SessionReport};
pub use startup::{InvalidRetryDelay, Lifecycle, Startup, validate_retry_delays};

pub use replay::{replay_point_count, replay_points};
