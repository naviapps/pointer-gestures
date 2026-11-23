//! Windows desktop input adaptation for the deterministic recognition engine.
//! An ordered injector delivers recovery while the input owner pumps native messages.
//! Hosts drain observations and execute their own commands. No application UI or
//! permission policy is installed.
#![warn(missing_docs)]

#[cfg(windows)]
mod desktop;
#[cfg(windows)]
mod notification;
#[cfg(any(windows, test))]
mod observation;
#[cfg(windows)]
pub use observation::ReadResult;
mod context;
pub use context::{ContextOutput, ContextReadResult, PressContext, SessionContext};
mod diagnostics;
pub use diagnostics::{
    DiagnosticPhase, DiagnosticReader, DiagnosticRecord, DiagnosticSnapshot, DiagnosticState,
};
#[cfg(any(windows, test))]
mod replay;
#[cfg(any(windows, test))]
mod session;
#[cfg(test)]
mod session_tests;
#[cfg(windows)]
pub use desktop::Recognizer;

/// Modifier requirements evaluated at the beginning of each native press.
/// Values are Shift=1, Control=2, Alt=4 and Windows=8; combinations are inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    /// Whether future presses may start recognition.
    pub enabled: bool,
    /// Required modifier groups. Additional held modifiers are allowed.
    pub required_modifiers: u32,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            enabled: true,
            required_modifiers: 0,
        }
    }
}

/// A native adapter failure. Failure stops capture and attempts input recovery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Unsupported button, modifier flags, tuning, or excessive replay buffer.
    InvalidConfiguration,
    /// One adapter already owns desktop input in this process.
    AlreadyRunning,
    /// A Windows operation failed with its system error code.
    System(u32),
    /// Windows accepted fewer injected events than requested.
    Replay {
        /// Last Windows error; zero does not imply successful injection.
        system: u32,
        /// Number of events accepted from the ordered batch.
        sent: u32,
        /// Number of events in the ordered batch.
        expected: u32,
    },
    /// The bounded input or observation backlog overflowed. Capture stopped.
    ConsumerLagged,
    /// The original input recipient changed before a retained press could be
    /// replayed. No new press is injected into the replacement window.
    ReplayTargetChanged,
    /// The native message loop could not complete a request.
    WorkerStopped,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Windows pointer adapter: {self:?}")
    }
}
impl std::error::Error for Error {}
