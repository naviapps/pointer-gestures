use crate::{Direction, Point, SessionReport};
use std::sync::Arc;

/// How the host should handle the original physical input associated with an output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Disposition {
    /// Deliver the original input normally.
    PassThrough = 0,
    /// Suppress the original input and apply the returned effects instead.
    Consume = 1,
    /// A control operation with no original physical input to suppress or deliver.
    Control = 2,
}
/// Recognition progress for the currently retained button session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum InputStage {
    /// No consumed button session remains.
    Idle = 0,
    /// Button-down is retained but movement has not started recognition.
    Pending = 1,
    /// Movement has started a recognized direction sequence.
    Active = 2,
}
/// Host policy rejection or deadline expiration reported in the current status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Failure {
    /// Host policy disabled the attempted session.
    Disabled = 1,
    /// The attempted non-recording session did not satisfy modifier policy.
    ModifiersNotSatisfied = 2,
    /// A total or idle deadline expired.
    Expired = 3,
}
/// Current lifecycle, capture, recording and failure snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    /// Whether the engine has been started and not subsequently stopped.
    pub ready: bool,
    /// Whether a button session has passed the recognition start threshold.
    pub capturing: bool,
    /// Recording mode configured for future sessions.
    pub recording: bool,
    /// Most recent policy or expiry failure, cleared by applicable lifecycle/input transitions.
    pub failure: Option<Failure>,
    /// Last completed captured direction sequence; empty when no sequence is available.
    pub recorded_directions: Arc<[Direction]>,
}
/// Immutable snapshots share buffers; cloning a trace never copies its point arrays.
/// Successive moving snapshots also share unchanged direction sequences while
/// retaining independent endpoint values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Trace {
    /// Whether the snapshot contains a currently visible captured trace.
    pub visible: bool,
    /// Cardinal runs in recognition order.
    pub directions: Arc<[Direction]>,
    /// Original stroke origin followed by one endpoint per cardinal run.
    pub endpoints: Arc<[Point]>,
    /// Bounded raw samples, published in batches and flushed on completion.
    pub raw_points: Arc<[Point]>,
    /// Latest pointer location, independent of raw-sample publication cadence.
    pub tail: Option<Point>,
}
/// Hosts deliver effects in order; the engine does not inject input or run commands.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Replay a complete click to recover consumed input.
    Click {
        /// Host-defined button identifier.
        button: u32,
        /// Original press location.
        point: Point,
    },
    /// Replay a complete drag, including its release.
    Drag {
        /// Host-defined button identifier.
        button: u32,
        /// Ordered replay samples, retaining the original press location.
        points: Arc<[Point]>,
    },
    /// Restore a held drag when a prefix cannot match; future physical input passes through.
    DragStart {
        /// Host-defined button identifier.
        button: u32,
        /// Ordered replay samples through the current held location.
        points: Arc<[Point]>,
    },
    /// Release the consumed button after recognition or interruption.
    Release {
        /// Host-defined button identifier.
        button: u32,
        /// Release location in the same coordinate space as the original input.
        point: Point,
    },
    /// Resolve and execute the host-owned command identified by this value.
    Match(u64),
    /// Publish an intermediate trace transition in sequence with recovery and commands.
    Publish {
        /// Status at this transition, which can differ from the final output status.
        status: Status,
        /// Visible or hidden trace at this transition.
        trace: Trace,
    },
}
/// Owned result of one engine call, valid after subsequent calls or engine destruction.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    /// Last completed retained session. Persists across snapshots and new input;
    /// compare its generation to distinguish a new completion. Not an event history.
    pub last_session: Option<SessionReport>,
    /// Handling required for the original physical input, if any.
    pub disposition: Disposition,
    /// Stage after the call completes.
    pub stage: InputStage,
    /// Final lifecycle and recognition status after all intermediate effects.
    pub status: Status,
    /// Final trace after all intermediate effects.
    pub trace: Trace,
    /// Nonzero identity of retained input; zero when idle. Wraps without producing zero.
    pub input_generation: u64,
    /// Discard any host-coalesced visible trace after interruption or prefix rejection.
    pub discard_pending_visible_trace: bool,
    /// Next absolute monotonic nanosecond deadline, or none when no tick is needed.
    pub deadline_ns: Option<u64>,
    /// Host requests in delivery order; intermediate publications must not be skipped.
    pub effects: Vec<Effect>,
}
