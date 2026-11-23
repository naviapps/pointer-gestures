//! Host context values captured before a native button-down enters recognition.
use crate::Policy;
use pointer_gesture_recognition::{Matcher, Output, Point};

/// Native identities observed synchronously for an eligible pointer press.
/// Process IDs are lookup hints, not durable process identities or permissions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PressContext {
    /// Physical virtual-desktop coordinates of the original press.
    pub point: Point,
    /// Process owning the original input recipient, including native capture.
    pub recipient_process_id: u32,
    /// Process owning the foreground window at this press, when available.
    pub foreground_process_id: Option<u32>,
}

/// Host-owned matching and policy snapshot selected for one physical press.
/// Cloning a matcher retains its immutable trie in O(1) time.
#[derive(Clone)]
pub struct SessionContext {
    /// Future matching rules, captured by the engine at button-down.
    pub matcher: Matcher,
    /// Enablement and modifier requirements for this press.
    pub policy: Policy,
    /// Opaque host snapshot identity, copied onto every associated observation.
    /// Hosts must retain its command table until those observations are drained.
    pub epoch: u64,
}

/// Owned observation annotated with the host snapshot captured for its press.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextOutput {
    /// Ordered recognition output; native recovery effects are already delivered.
    pub output: Output,
    /// Captured host identity, absent before the first context-resolved press.
    /// It remains associated through release, interruption and shutdown.
    pub epoch: Option<u64>,
}

/// Context-aware nonblocking read, distinguishing silence from completion.
#[derive(Debug)]
#[expect(
    clippy::large_enum_variant,
    reason = "Transient reads keep frames inline to avoid per-input allocation"
)]
pub enum ContextReadResult {
    /// Deliver this owned frame's effects before reading another.
    Frame(ContextOutput),
    /// No observation yet; wait for notification readiness.
    Pending,
    /// Normal completion after final observations have drained.
    Completed,
}

#[cfg(any(windows, test))]
pub(crate) fn resolve(
    resolver: &mut dyn FnMut(PressContext) -> Result<SessionContext, crate::Error>,
    press: PressContext,
) -> Result<SessionContext, crate::Error> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| resolver(press)))
        .unwrap_or(Err(crate::Error::WorkerStopped))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolver_failure_is_reported_without_leaving_the_hook_boundary() {
        let press = PressContext {
            point: Point { x: 1., y: 2. },
            recipient_process_id: 42,
            foreground_process_id: None,
        };
        let result = resolve(&mut |_| panic!("Host resolver failed"), press);
        assert!(matches!(result, Err(crate::Error::WorkerStopped)));
        let result = resolve(&mut |_| Err(crate::Error::InvalidConfiguration), press);
        assert!(matches!(result, Err(crate::Error::InvalidConfiguration)));
    }
}
