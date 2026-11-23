use crate::Point;

/// Host input or lifecycle control. Every event is paired with monotonic time in [`crate::Engine::step`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// A physical pointer-button press.
    Down {
        /// Host-defined button identifier.
        button: u32,
        /// Global location in the host's consistent coordinate space.
        point: Point,
    },
    /// Pointer motion while the named button is held.
    Move {
        /// Host-defined button identifier.
        button: u32,
        /// Current global location.
        point: Point,
    },
    /// A physical pointer-button release, including its final location. A non-finite
    /// location interrupts retained input for this button with finite recovery and
    /// a consume disposition, never a match. Unretained input passes through.
    Up {
        /// Host-defined button identifier.
        button: u32,
        /// Final global location.
        point: Point,
    },
    /// Interrupt active input and return the required recovery effects.
    Cancel,
    /// Stop recognition and recover input; the engine remains reusable.
    Stop,
    /// Enable recognition on a stopped engine.
    Start,
    /// Set recording mode for future sessions; active input keeps its captured mode.
    Recording(bool),
    /// Set host eligibility for future button-down events.
    Policy {
        /// Whether the current host context permits recognition.
        enabled: bool,
        /// Whether the host's modifier policy is satisfied outside recording.
        modifiers_satisfied: bool,
    },
    /// Expire an elapsed total or idle deadline without physical input.
    Tick,
    /// Read the current state after updating host policy, matcher or tuning.
    Snapshot,
}
