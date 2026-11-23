//! Event-source startup policy without tasks, callbacks or OS ownership.

use crate::duration_nanoseconds;

/// Observable event-source lifecycle.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u32)]
pub enum Lifecycle {
    /// Stopped with no requested retry.
    #[default]
    Idle = 0,
    /// A synchronous source-start attempt is in progress.
    Starting = 1,
    /// The source accepted startup.
    Ready = 2,
    /// A failed source will be retried.
    Retrying = 3,
    /// Startup failed and no retry schedule is configured.
    Failed = 4,
}

/// A negative or non-finite retry delay, identified by its zero-based index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidRetryDelay(pub usize);
impl std::fmt::Display for InvalidRetryDelay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "retry delay {} must be finite and nonnegative", self.0)
    }
}
impl std::error::Error for InvalidRetryDelay {}

/// Validate a second-based retry schedule without sorting or removing duplicates.
pub fn validate_retry_delays(delays: &[f64]) -> Result<(), InvalidRetryDelay> {
    delays
        .iter()
        .position(|d| !d.is_finite() || *d < 0.)
        .map_or(Ok(()), |index| Err(InvalidRetryDelay(index)))
}

/// Revision-guarded startup and retry state. Hosts own sleeping, source installation
/// and teardown. A stale completion or scheduled retry can never resurrect a stopped
/// or replaced source. Repeated failures reuse the schedule's last delay.
#[derive(Debug, Default)]
pub struct Startup {
    lifecycle: Lifecycle,
    requested: bool,
    revision: u64,
    attempt: usize,
    delays: Vec<u64>,
}
impl Startup {
    /// Create a stopped lifecycle with validated second-based retry delays.
    pub fn new(delays: &[f64]) -> Result<Self, InvalidRetryDelay> {
        let mut startup = Self::default();
        startup.set_delays(delays)?;
        Ok(startup)
    }
    /// Current source lifecycle.
    pub fn lifecycle(&self) -> Lifecycle {
        self.lifecycle
    }
    /// Whether the host still wants the source running.
    pub fn requested(&self) -> bool {
        self.requested
    }
    /// Current request revision, used for reentrant and scheduled work.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    fn advance(&mut self) {
        self.revision = self.revision.wrapping_add(1).max(1);
        self.attempt = 0;
    }
    /// Request immediate startup. Ready/starting requests are idempotent.
    pub fn start(&mut self) -> bool {
        if matches!(self.lifecycle, Lifecycle::Ready | Lifecycle::Starting) {
            return false;
        }
        self.advance();
        self.requested = true;
        self.lifecycle = Lifecycle::Starting;
        true
    }
    /// Stop and invalidate all pending attempts. Returns whether teardown is needed.
    pub fn stop(&mut self) -> bool {
        if !self.requested && self.lifecycle == Lifecycle::Idle {
            return false;
        }
        self.advance();
        self.requested = false;
        self.lifecycle = Lifecycle::Idle;
        true
    }
    /// Retry immediately only after an earlier start request.
    pub fn retry_now(&mut self) -> bool {
        self.requested && self.start()
    }
    /// Accept a scheduled attempt only for the current retrying request.
    pub fn retry(&mut self, revision: u64) -> bool {
        if revision != self.revision || !self.requested || self.lifecycle != Lifecycle::Retrying {
            return false;
        }
        self.lifecycle = Lifecycle::Starting;
        true
    }
    /// Record startup completion only if its attempt is still current.
    pub fn complete(&mut self, revision: u64, success: bool) -> bool {
        if revision != self.revision || !self.requested || self.lifecycle != Lifecycle::Starting {
            return false;
        }
        self.lifecycle = if success {
            Lifecycle::Ready
        } else {
            self.failure_state()
        };
        true
    }
    fn failure_state(&self) -> Lifecycle {
        if self.delays.is_empty() {
            Lifecycle::Failed
        } else {
            Lifecycle::Retrying
        }
    }
    /// Next retry delay in nanoseconds. Zero requires a host task yield. Only a
    /// retrying lifecycle advances the schedule; counters saturate without overflow.
    pub fn next_delay(&mut self) -> Option<u64> {
        if self.lifecycle != Lifecycle::Retrying {
            return None;
        }
        let delay = self.delays[self.attempt.min(self.delays.len() - 1)];
        self.attempt = self.attempt.saturating_add(1);
        Some(delay)
    }
    /// Replace retry policy atomically. Pending/failed requests begin the new schedule;
    /// a ready or currently starting source is not restarted. Returns whether pending
    /// task work must be replaced. Invalid input leaves the existing policy intact.
    pub fn set_delays(&mut self, delays: &[f64]) -> Result<bool, InvalidRetryDelay> {
        validate_retry_delays(delays)?;
        self.delays = delays.iter().copied().map(duration_nanoseconds).collect();
        if self.requested && !matches!(self.lifecycle, Lifecycle::Ready | Lifecycle::Starting) {
            self.advance();
            self.lifecycle = self.failure_state();
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_attempts_and_retries_cannot_restart_a_stopped_source() {
        let mut s = Startup::new(&[0., 0.5, 1.]).unwrap();
        assert!(!s.retry_now());
        assert!(s.start());
        let old = s.revision();
        assert!(!s.start());
        assert!(s.stop());
        assert!(!s.stop());
        assert!(!s.complete(old, true));
        assert!(!s.retry(old));
        assert!(s.start());
        assert!(!s.complete(old, false));
        assert_eq!(s.lifecycle(), Lifecycle::Starting);
        assert!(s.complete(s.revision(), true));
        assert!(!s.retry_now());
    }
    #[test]
    fn retries_repeat_last_delay_and_reconfiguration_invalidates_pending_work() {
        let mut s = Startup::new(&[0., 0.5, 0.5]).unwrap();
        s.start();
        s.complete(s.revision(), false);
        assert_eq!(s.next_delay(), Some(0));
        for _ in 0..4 {
            assert!(s.retry(s.revision()));
            assert!(s.complete(s.revision(), false));
            assert_eq!(s.next_delay(), Some(500_000_000));
        }
        let old = s.revision();
        assert!(s.set_delays(&[]).unwrap());
        assert_eq!(s.lifecycle(), Lifecycle::Failed);
        assert_eq!(s.next_delay(), None);
        assert!(!s.retry(old));
        assert!(s.set_delays(&[f64::MAX]).unwrap());
        assert_eq!(s.next_delay(), Some(u64::MAX));
        assert!(s.retry_now());
        assert!(s.complete(s.revision(), true));
        assert!(!s.set_delays(&[]).unwrap());
        assert_eq!(s.lifecycle(), Lifecycle::Ready);
    }
    #[test]
    fn invalid_policy_preserves_active_schedule() {
        let mut s = Startup::new(&[1.]).unwrap();
        s.start();
        s.complete(s.revision(), false);
        assert_eq!(s.set_delays(&[0., f64::NAN]), Err(InvalidRetryDelay(1)));
        assert_eq!(s.next_delay(), Some(1_000_000_000));
        assert_eq!(validate_retry_delays(&[-1.]), Err(InvalidRetryDelay(0)));
    }
}
