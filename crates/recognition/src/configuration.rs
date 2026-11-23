/// Engine tuning in the host's coordinate units and monotonic nanoseconds.
/// Distances must be finite and nonnegative; the point limit must be positive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Configuration {
    /// Dominant-axis distance from button-down required to begin recognition.
    pub start_distance: f64,
    /// Dominant-axis distance from the previous endpoint required to add a turn.
    pub change_distance: f64,
    /// Maximum session duration in nanoseconds; zero disables this deadline.
    pub total_duration_ns: u64,
    /// Maximum time without changed coordinates, in nanoseconds; zero disables it.
    pub idle_duration_ns: u64,
    /// Raw trace limit. Replay retains at least the origin and latest point.
    pub maximum_points: usize,
    /// Maximum accepted direction runs per session, from 1 through 4096.
    /// Captured at button-down; exceeding the budget requests input recovery.
    pub maximum_directions: usize,
    /// Host-defined pointer button identifier, immutable after construction.
    pub button: u32,
    /// Pass empty-matcher input through outside recording; immutable after construction.
    pub passes_through_empty_matcher: bool,
}
impl Configuration {
    pub(crate) fn valid(self) -> bool {
        validate_tuning(
            self.start_distance,
            self.change_distance,
            None,
            None,
            self.maximum_points,
        )
        .is_ok()
            && validate_direction_limit(self.maximum_directions)
    }
}
/// Whether a direction budget is positive and bounded to at most 4096 runs.
pub fn validate_direction_limit(value: usize) -> bool {
    (1..=4096).contains(&value)
}
/// Invalid distances/point limit, or an attempted immutable policy change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidConfiguration;
impl std::fmt::Display for InvalidConfiguration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid distances, point/direction limits, or immutable engine policy")
    }
}
impl std::error::Error for InvalidConfiguration {}

/// Convert a positive finite duration to nanoseconds, rounding up and saturating.
/// Zero, negative and non-finite durations request no delay.
pub fn duration_nanoseconds(seconds: f64) -> u64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0;
    }
    (seconds * 1_000_000_000.0).ceil().max(1.0) as u64
}

/// The first invalid field in host-facing second-based tuning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum InvalidTuning {
    /// Start distance is negative or non-finite.
    StartDistance = 1,
    /// Turn distance is negative or non-finite.
    ChangeDistance = 2,
    /// An enabled total duration is non-positive or non-finite.
    TotalDuration = 3,
    /// An enabled idle duration is non-positive or non-finite.
    IdleDuration = 4,
    /// The raw point limit is zero.
    MaximumPoints = 5,
}
impl std::fmt::Display for InvalidTuning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::StartDistance => "start distance must be finite and nonnegative",
            Self::ChangeDistance => "change distance must be finite and nonnegative",
            Self::TotalDuration => "enabled total duration must be finite and positive",
            Self::IdleDuration => "enabled idle duration must be finite and positive",
            Self::MaximumPoints => "point limit must be positive",
        })
    }
}
impl std::error::Error for InvalidTuning {}

/// Validate distances, optional durations and the point limit in field order.
/// Native engine configuration and language adapters share this validation.
pub fn validate_tuning(
    start: f64,
    change: f64,
    total: Option<f64>,
    idle: Option<f64>,
    points: usize,
) -> Result<(), InvalidTuning> {
    if !start.is_finite() || start < 0.0 {
        Err(InvalidTuning::StartDistance)
    } else if !change.is_finite() || change < 0.0 {
        Err(InvalidTuning::ChangeDistance)
    } else if total.is_some_and(|v| !v.is_finite() || v <= 0.0) {
        Err(InvalidTuning::TotalDuration)
    } else if idle.is_some_and(|v| !v.is_finite() || v <= 0.0) {
        Err(InvalidTuning::IdleDuration)
    } else if points == 0 {
        Err(InvalidTuning::MaximumPoints)
    } else {
        Ok(())
    }
}

/// Convert signed whole seconds and fractional attoseconds to elapsed nanoseconds.
/// Clamp negative components to zero, truncate sub-nanosecond fractions and saturate
/// overflow. Native hosts retain their own monotonic clock/instant ownership.
pub fn elapsed_nanoseconds(seconds: i64, attoseconds: i64) -> u64 {
    (seconds.max(0) as u64)
        .saturating_mul(1_000_000_000)
        .saturating_add(attoseconds.max(0) as u64 / 1_000_000_000)
}

#[cfg(test)]
mod elapsed_tests {
    use super::elapsed_nanoseconds;
    #[test]
    fn elapsed_components_truncate_clamp_and_saturate() {
        assert_eq!(
            elapsed_nanoseconds(2, 123_456_789_999_999_999),
            2_123_456_789
        );
        assert_eq!(elapsed_nanoseconds(-1, -1), 0);
        assert_eq!(elapsed_nanoseconds(0, 999_999_999), 0);
        assert_eq!(elapsed_nanoseconds(i64::MAX, i64::MAX), u64::MAX);
        assert_eq!(
            elapsed_nanoseconds(18_446_744_073, 999_999_999_999_999_999),
            u64::MAX
        );
    }
}
