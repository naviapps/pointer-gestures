//! Bounded, coordinate-free summaries of completed retained input.
use crate::Direction;
use std::sync::Arc;

/// Why a retained button session ended. Ordinary clicks are not recognition errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum SessionOutcome {
    /// An exact registered pattern produced its host-owned match effect.
    Matched = 1,
    /// Recording completed a nonempty direction pattern.
    Recorded = 2,
    /// Movement did not start recognition; ordinary input recovery was requested.
    BelowThreshold = 3,
    /// The completed pattern was a registered prefix without an exact match.
    Unmatched = 4,
    /// A new direction could not extend any registered pattern.
    UnmatchedPrefix = 5,
    /// The host explicitly cancelled retained input.
    Cancelled = 6,
    /// Recognition stopped or closed while retaining input.
    Stopped = 7,
    /// A replacement button-down superseded retained input.
    Superseded = 8,
    /// The total session deadline was the earliest expired deadline.
    TotalDeadline = 9,
    /// The inactivity deadline was the earliest expired deadline.
    IdleDeadline = 10,
    /// A matching release had a non-finite location and could not match.
    InvalidRelease = 11,
    /// Another direction would exceed the session's captured direction budget.
    DirectionLimit = 12,
}

/// Last completed retained session, independent of later input and engine lifetime.
///
/// This describes the recognition decision and requested effects, not successful OS
/// input injection or host command execution. It retains no pointer coordinates.
/// Times use the host's monotonic nanoseconds; distances use its coordinate units.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionReport {
    /// Nonzero wrapping identity of the completed session, local to its engine.
    pub generation: u64,
    /// Terminal recognition decision.
    pub outcome: SessionOutcome,
    /// Time since the original press, unaffected by deadline tuning resets.
    pub elapsed_ns: u64,
    /// Time since the last changed finite pointer location.
    pub idle_ns: u64,
    /// Number of admitted finite samples, including press, release and stationary moves.
    pub point_count: u64,
    /// Largest absolute horizontal or vertical displacement from the press, saturated finite.
    pub maximum_axis_distance: f64,
    /// Whether this session crossed the recognition start threshold.
    pub captured: bool,
    /// Recording mode captured at the press.
    pub recording: bool,
    /// Accepted direction runs, bounded by the session's captured budget.
    pub directions: Arc<[Direction]>,
    /// A direction rejected by prefix validation or the direction budget.
    pub rejected_direction: Option<Direction>,
}
