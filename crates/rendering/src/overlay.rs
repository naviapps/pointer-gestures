use crate::OverlayTrace;

/// Presentation cadence inputs in seconds and the host's coordinate units.
/// States normalize supplied values; nanoseconds are derived internally.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayTiming {
    /// Animation interval in seconds, defaulting to 1/60 second.
    pub interval_seconds: f64,
    /// Coordinate distance allowing a rate-limited immediate update, defaulting to 24.
    pub maximum_point_lag: f64,
}
impl OverlayTiming {
    /// Normalize invalid/non-positive/oversized intervals to 1/60 second and
    /// negative or non-finite lag distances to 24. Zero lag remains valid.
    pub fn new(interval_seconds: f64, maximum_point_lag: f64) -> Self {
        let interval_seconds = if interval_seconds.is_finite()
            && interval_seconds > 0.0
            && interval_seconds <= (i64::MAX / 4) as f64 / 1_000_000_000.0
        {
            interval_seconds
        } else {
            1.0 / 60.0
        };
        let maximum_point_lag = if maximum_point_lag.is_finite() && maximum_point_lag >= 0.0 {
            maximum_point_lag
        } else {
            24.0
        };
        Self {
            interval_seconds,
            maximum_point_lag,
        }
    }
}
impl Default for OverlayTiming {
    fn default() -> Self {
        Self::new(1.0 / 60.0, 24.0)
    }
}

/// A value snapshot, valid independently of later updates and reducer destruction.
#[derive(Debug, Clone)]
pub struct OverlayUpdate {
    /// Whether the presented trace changed during this call.
    pub changed: bool,
    /// Current presented trace; none means hidden. Retain it when `changed` is false.
    pub trace: Option<OverlayTrace>,
    /// Next absolute monotonic nanosecond deadline, or none when no tick is needed.
    pub deadline_ns: Option<u64>,
}

/// Serial presentation reducer. Hosts supply monotonic time and schedule `tick`;
/// it owns no timer, observer, callback, graphics API or window.
pub struct OverlayState {
    configuration: OverlayTiming,
    interval_ns: u64,
    trace: Option<OverlayTrace>,
    last_visible: Option<u64>,
    last_lag_bypass: Option<u64>,
    pending: Option<Option<OverlayTrace>>,
    deadline: Option<u64>,
    now: u64,
}
impl OverlayState {
    /// Create a state with an optional initially presented trace and normalized timing.
    /// `now_ns` establishes the host's monotonic time origin; later clock regressions clamp.
    pub fn new(configuration: OverlayTiming, trace: Option<OverlayTrace>, now_ns: u64) -> Self {
        let configuration = OverlayTiming::new(
            configuration.interval_seconds,
            configuration.maximum_point_lag,
        );
        Self {
            configuration,
            interval_ns: pointer_gesture_recognition::duration_nanoseconds(
                configuration.interval_seconds,
            ),
            last_visible: trace.as_ref().map(|_| now_ns),
            trace,
            last_lag_bypass: None,
            pending: None,
            deadline: None,
            now: now_ns,
        }
    }
    /// Submit new visible geometry or none to hide, at monotonic nanosecond time.
    /// Ordinary movement coalesces; direction changes and rate-limited large
    /// movement publish immediately. Hiding first presents any pending final trace.
    /// Returning to presented geometry cancels older pending movement or hiding.
    /// Schedule [`Self::tick`] at the returned deadline, even when `changed` is false.
    pub fn update(&mut self, trace: Option<OverlayTrace>, now_ns: u64) -> OverlayUpdate {
        self.now = self.now.max(now_ns);
        let before = self.trace.clone();
        if self.trace == trace {
            self.cancel();
        } else if self.pending.as_ref() != Some(&trace) {
            match trace {
                None => {
                    if let Some(Some(final_trace)) = self.pending.take() {
                        self.cancel();
                        self.publish(Some(final_trace));
                    }
                    let deadline = self
                        .last_visible
                        .map(|t| t.saturating_add(self.interval_ns.saturating_mul(2)));
                    if let Some(deadline) = deadline.filter(|&t| t > self.now) {
                        self.pending = Some(None);
                        self.deadline = Some(deadline);
                    } else {
                        self.cancel();
                        self.publish(None);
                    }
                }
                Some(next) => {
                    let direction_change = self
                        .trace
                        .as_ref()
                        .is_some_and(|t| t.directions() != next.directions());
                    let lag = self.trace.as_ref().is_some_and(|t| {
                        let a = t.leading_point();
                        let b = next.leading_point();
                        (a.x - b.x).hypot(a.y - b.y) > self.configuration.maximum_point_lag
                    });
                    let bypass = lag
                        && self
                            .last_lag_bypass
                            .is_none_or(|t| self.now.saturating_sub(t) >= self.interval_ns);
                    let resumed = self.pending == Some(None);
                    if resumed
                        || self.last_visible.is_none()
                        || direction_change
                        || bypass
                        || self
                            .last_visible
                            .is_some_and(|t| self.now.saturating_sub(t) >= self.interval_ns)
                    {
                        self.cancel();
                        self.publish(Some(next));
                        if resumed {
                            self.last_lag_bypass = None;
                        } else if bypass && !direction_change {
                            self.last_lag_bypass = Some(self.now);
                        }
                    } else {
                        self.pending = Some(Some(next));
                        // Replacing pending data never restarts its timer.
                        self.deadline = self
                            .last_visible
                            .map(|t| t.saturating_add(self.interval_ns));
                    }
                }
            }
        }
        self.snapshot(before != self.trace)
    }
    /// Deliver monotonic nanosecond time to publish an elapsed pending update or hide.
    /// Clock regressions clamp; absolute deadlines saturate at [`u64::MAX`].
    pub fn tick(&mut self, now_ns: u64) -> OverlayUpdate {
        self.now = self.now.max(now_ns);
        let before = self.trace.clone();
        if self.deadline.is_some_and(|t| t <= self.now) {
            let pending = self.pending.take();
            self.deadline = None;
            if let Some(trace) = pending {
                self.publish(trace);
            }
        }
        self.snapshot(before != self.trace)
    }
    /// Replace cadence without resetting visible geometry or pending final hiding.
    /// Deadlines are recomputed from the last actual presentation, never from the
    /// configuration change. A newly due update is delivered during this call.
    pub fn reconfigure(&mut self, configuration: OverlayTiming, now_ns: u64) -> OverlayUpdate {
        self.configuration = OverlayTiming::new(
            configuration.interval_seconds,
            configuration.maximum_point_lag,
        );
        self.interval_ns =
            pointer_gesture_recognition::duration_nanoseconds(self.configuration.interval_seconds);
        if let Some(pending) = self.pending.take() {
            self.deadline = None;
            self.update(pending, now_ns)
        } else {
            self.now = self.now.max(now_ns);
            self.snapshot(false)
        }
    }
    /// Immediately discard visible and queued geometry, retaining configuration.
    /// Unlike `update(None, ...)`, this never presents a pending final frame.
    /// No deadline remains; later visible input starts a fresh presentation cadence.
    pub fn clear(&mut self, now_ns: u64) -> OverlayUpdate {
        self.now = self.now.max(now_ns);
        let changed = self.trace.is_some();
        self.cancel();
        self.trace = None;
        self.last_visible = None;
        self.last_lag_bypass = None;
        self.snapshot(changed)
    }
    fn cancel(&mut self) {
        self.pending = None;
        self.deadline = None;
    }
    fn publish(&mut self, trace: Option<OverlayTrace>) {
        if self.trace == trace {
            return;
        }
        self.trace = trace;
        self.last_visible = self.trace.as_ref().map(|_| self.now);
        if self.trace.is_none() {
            self.last_lag_bypass = None;
        }
    }
    fn snapshot(&self, changed: bool) -> OverlayUpdate {
        OverlayUpdate {
            changed,
            trace: self.trace.clone(),
            deadline_ns: self.deadline,
        }
    }
}
