//! Browser adaptation of the existing safe recognition and geometry libraries.
//! No DOM, input injection, clocks, rendering resources or product commands live here.
#![warn(missing_docs)]

use pointer_gesture_recognition::{
    Configuration, Direction, Effect, Engine, Event, Failure, InputStage, Matcher, Output,
    PatternError, Point, SessionReport, Trace, validate_direction_limit,
    validate_pattern as validate_runs,
};
use pointer_gesture_rendering::{Command, CornerStyle, Path, live_trace, pattern};
#[cfg(feature = "web")]
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone, Copy)]
struct PatternValidation(u32, u32);
impl From<PatternError> for PatternValidation {
    fn from(error: PatternError) -> Self {
        match error {
            PatternError::Empty => Self(1, u32::MAX),
            PatternError::TooLong => Self(2, u32::MAX),
            PatternError::RepeatedDirection { index } => Self(3, index as u32),
        }
    }
}
fn pattern_input(values: &[u8]) -> Result<Vec<Direction>, PatternValidation> {
    if values.is_empty() {
        return Err(PatternError::Empty.into());
    }
    if !validate_direction_limit(values.len()) {
        return Err(PatternError::TooLong.into());
    }
    values
        .iter()
        .enumerate()
        .map(|(index, &value)| {
            Direction::try_from(u32::from(value)).map_err(|_| PatternValidation(4, index as u32))
        })
        .collect()
}
fn pattern_result(result: Result<(), PatternValidation>) -> Vec<u32> {
    let PatternValidation(code, index) = result.err().unwrap_or(PatternValidation(0, u32::MAX));
    vec![code, index]
}
/// Structured validation: code and zero-based index, u32::MAX when absent.
#[cfg_attr(feature = "web", wasm_bindgen(js_name = rawValidatePattern))]
pub fn validate_pattern(values: &[u8]) -> Vec<u32> {
    pattern_result(
        pattern_input(values).and_then(|pattern| validate_runs(&pattern).map_err(Into::into)),
    )
}

fn directions(values: &[u8]) -> Result<Vec<Direction>, String> {
    values
        .iter()
        .map(|&value| Direction::try_from(u32::from(value)).map_err(|error| error.to_string()))
        .collect()
}

fn commands(path: &Path) -> Vec<f64> {
    let mut values = Vec::new();
    for command in &path.commands {
        match command {
            Command::Move(point) => values.extend([1., point.x, point.y]),
            Command::Line(point) => values.extend([2., point.x, point.y]),
            Command::Cubic {
                control1,
                control2,
                to,
            } => values.extend([
                3., control1.x, control1.y, control2.x, control2.y, to.x, to.y,
            ]),
            Command::Close => values.push(4.),
        }
    }
    values
}

fn duration_ms(value: f64) -> Result<u64, String> {
    if !value.is_finite() || value < 0. {
        return Err("Time must be finite and nonnegative milliseconds".into());
    }
    Ok(pointer_gesture_recognition::duration_nanoseconds(
        value / 1_000.,
    ))
}

fn configuration(
    start_distance: f64,
    change_distance: f64,
    maximum_points: u32,
    maximum_directions: u32,
    total_ms: f64,
    idle_ms: f64,
) -> Result<Configuration, String> {
    Ok(Configuration {
        start_distance,
        change_distance,
        total_duration_ns: duration_ms(total_ms)?,
        idle_duration_ns: duration_ms(idle_ms)?,
        maximum_points: maximum_points as usize,
        maximum_directions: maximum_directions as usize,
        button: 0,
        passes_through_empty_matcher: true,
    })
}

/// Owned, copied result. The JavaScript adapter frees this after decoding its values.
#[cfg_attr(feature = "web", wasm_bindgen)]
pub struct RawUpdate {
    matched: Option<u32>,
    trace: Option<Trace>,
    active: bool,
    deadline_ms: Option<f64>,
    discard_pending_visible_trace: bool,
    expired: bool,
    last_session: Option<SessionReport>,
}

/// Owned recognition geometry. No path planning occurs while taking a snapshot.
#[cfg_attr(feature = "web", wasm_bindgen)]
pub struct RawTrace {
    trace: Trace,
}
#[cfg_attr(feature = "web", wasm_bindgen)]
impl RawTrace {
    /// Copy stable cardinal direction codes.
    pub fn directions(&self) -> Vec<u8> {
        self.trace.directions.iter().map(|d| *d as u8).collect()
    }
    /// Copy origin and direction endpoints as interleaved x/y coordinates.
    pub fn points(&self) -> Vec<f64> {
        self.trace
            .endpoints
            .iter()
            .flat_map(|p| [p.x, p.y])
            .collect()
    }
    /// Copy the optional latest pointer position as zero or two coordinates.
    pub fn tail(&self) -> Vec<f64> {
        self.trace.tail.map(|p| vec![p.x, p.y]).unwrap_or_default()
    }
}

/// Owned coordinate-free summary; all decisions and measurements come from Rust recognition.
#[cfg_attr(feature = "web", wasm_bindgen)]
pub struct RawSessionReport {
    report: SessionReport,
}
#[cfg_attr(feature = "web", wasm_bindgen)]
impl RawSessionReport {
    /// Engine-local exact nonzero wrapping identity (BigInt in JavaScript).
    #[cfg_attr(feature = "web", wasm_bindgen(getter))]
    pub fn generation(&self) -> u64 {
        self.report.generation
    }
    /// Terminal decision code, as declared by the recognition SessionOutcome.
    #[cfg_attr(feature = "web", wasm_bindgen(getter))]
    pub fn outcome(&self) -> u32 {
        self.report.outcome as u32
    }
    /// Milliseconds since the original press, independent of deadline resets.
    #[cfg_attr(feature = "web", wasm_bindgen(getter, js_name = elapsedMs))]
    pub fn elapsed_ms(&self) -> f64 {
        self.report.elapsed_ns as f64 / 1_000_000.
    }
    /// Milliseconds since the last changed finite location.
    #[cfg_attr(feature = "web", wasm_bindgen(getter, js_name = idleMs))]
    pub fn idle_ms(&self) -> f64 {
        self.report.idle_ns as f64 / 1_000_000.
    }
    /// Exact admitted finite sample count (BigInt in JavaScript).
    #[cfg_attr(feature = "web", wasm_bindgen(getter, js_name = pointCount))]
    pub fn point_count(&self) -> u64 {
        self.report.point_count
    }
    /// Maximum axis displacement in caller coordinate units.
    #[cfg_attr(feature = "web", wasm_bindgen(getter, js_name = maximumAxisDistance))]
    pub fn maximum_axis_distance(&self) -> f64 {
        self.report.maximum_axis_distance
    }
    /// Whether the input crossed the recognition start threshold.
    #[cfg_attr(feature = "web", wasm_bindgen(getter))]
    pub fn captured(&self) -> bool {
        self.report.captured
    }
    /// Recording mode captured at button-down.
    #[cfg_attr(feature = "web", wasm_bindgen(getter))]
    pub fn recording(&self) -> bool {
        self.report.recording
    }
    /// Optional rejected direction code; accepted directions remain bounded.
    #[cfg_attr(feature = "web", wasm_bindgen(getter, js_name = rejectedDirection))]
    pub fn rejected_direction(&self) -> Option<u8> {
        self.report.rejected_direction.map(|d| d as u8)
    }
    /// Copy the accepted direction runs.
    pub fn directions(&self) -> Vec<u8> {
        self.report.directions.iter().map(|d| *d as u8).collect()
    }
}

#[cfg_attr(feature = "web", wasm_bindgen)]
impl RawUpdate {
    /// Transfer the report for the JavaScript adapter to copy and free.
    #[cfg_attr(feature = "web", wasm_bindgen(js_name = takeLastSession))]
    pub fn take_last_session(&mut self) -> Option<RawSessionReport> {
        self.last_session
            .take()
            .map(|report| RawSessionReport { report })
    }
    /// Host-registered identifier, present only on a successfully completed match.
    #[cfg_attr(feature = "web", wasm_bindgen(getter))]
    pub fn matched(&self) -> Option<u32> {
        self.matched
    }
    /// Whether a pending or capturing pointer session remains.
    #[cfg_attr(feature = "web", wasm_bindgen(getter))]
    pub fn active(&self) -> bool {
        self.active
    }
    /// Absolute next deadline on the same millisecond clock, if one exists.
    #[cfg_attr(feature = "web", wasm_bindgen(getter, js_name = deadlineMs))]
    pub fn deadline_ms(&self) -> Option<f64> {
        self.deadline_ms
    }
    /// Clear any host-coalesced visible path before applying this result.
    #[cfg_attr(feature = "web", wasm_bindgen(getter, js_name = discardPendingVisibleTrace))]
    pub fn discard_pending_visible_trace(&self) -> bool {
        self.discard_pending_visible_trace
    }
    /// The retained input expired under the shared total or idle deadline.
    #[cfg_attr(feature = "web", wasm_bindgen(getter))]
    pub fn expired(&self) -> bool {
        self.expired
    }
    /// Transfer an independent trace for the JavaScript adapter to copy and free.
    #[cfg_attr(feature = "web", wasm_bindgen(js_name = takeTrace))]
    pub fn take_trace(&mut self) -> Option<RawTrace> {
        self.trace.take().map(|trace| RawTrace { trace })
    }
}

/// One serial browser pointer session using the shared native recognition engine.
#[cfg_attr(feature = "web", wasm_bindgen)]
pub struct RawSession {
    engine: Engine,
    matcher: Matcher,
    last_time: u64,
    last_visible: Option<Trace>,
}

#[cfg_attr(feature = "web", wasm_bindgen)]
impl RawSession {
    /// Construct with caller-owned coordinate units and millisecond durations.
    #[cfg_attr(feature = "web", wasm_bindgen(constructor))]
    pub fn new(
        start: f64,
        change: f64,
        maximum_points: u32,
        maximum_directions: u32,
        total_ms: f64,
        idle_ms: f64,
        recording: bool,
    ) -> Result<Self, String> {
        let mut engine = Engine::new(configuration(
            start,
            change,
            maximum_points,
            maximum_directions,
            total_ms,
            idle_ms,
        )?)
        .map_err(|error| error.to_string())?;
        engine.step(Event::Start, 0);
        engine.step(Event::Recording(recording), 0);
        Ok(Self {
            engine,
            matcher: Matcher::default(),
            last_time: 0,
            last_visible: None,
        })
    }

    /// Register or replace an exact future pattern, returning structured validation.
    /// Failed registration leaves both matcher and active session snapshots intact.
    pub fn register(&mut self, values: &[u8], identifier: u32) -> Vec<u32> {
        let result = pattern_input(values).and_then(|pattern| {
            self.matcher
                .register(&pattern, u64::from(identifier))
                .map_err(Into::into)
        });
        if result.is_ok() {
            self.engine.use_matcher(&self.matcher);
        }
        pattern_result(result)
    }

    /// Remove one exact registration from future input, preserving active snapshots.
    pub fn remove(&mut self, pattern: &[u8]) -> Result<Option<u32>, String> {
        let removed = self
            .matcher
            .remove(&directions(pattern)?)
            .map(|id| id as u32);
        self.engine.use_matcher(&self.matcher);
        Ok(removed)
    }

    /// Clear future registrations without changing an active stroke's snapshot.
    pub fn clear(&mut self) {
        self.matcher.clear();
        self.engine.use_matcher(&self.matcher);
    }

    /// Record arbitrary patterns for future strokes without requiring matcher prefixes.
    /// Active input retains its captured mode, just as in the native engine.
    pub fn set_recording(&mut self, enabled: bool, now_ms: f64) -> Result<RawUpdate, String> {
        self.process(Event::Recording(enabled), now_ms)
    }

    /// Apply shared mutable tuning atomically and return the next deadline.
    /// Active input retains its mode, matcher and captured direction budget.
    #[allow(
        clippy::too_many_arguments,
        reason = "Flat Wasm boundary mirrors named tuning fields"
    )]
    pub fn update_tuning(
        &mut self,
        start: f64,
        change: f64,
        maximum_points: u32,
        maximum_directions: u32,
        total_ms: f64,
        idle_ms: f64,
        now_ms: f64,
        reset_unchanged: bool,
    ) -> Result<RawUpdate, String> {
        let now = self.validated_time(now_ms)?;
        let config = configuration(
            start,
            change,
            maximum_points,
            maximum_directions,
            total_ms,
            idle_ms,
        )?;
        self.engine
            .update_tuning(config, now, reset_unchanged)
            .map_err(|e| e.to_string())?;
        self.last_time = now;
        let output = self.engine.step(Event::Snapshot, now);
        Ok(self.project(output, false))
    }

    /// Begin pointer input. Pointer capture and browser event suppression belong to the host.
    pub fn begin(&mut self, x: f64, y: f64, now_ms: f64) -> Result<RawUpdate, String> {
        let point = finite_point(x, y)?;
        self.process(Event::Down { button: 0, point }, now_ms)
    }
    /// Advance held input and return the current shared geometry.
    pub fn moved(&mut self, x: f64, y: f64, now_ms: f64) -> Result<RawUpdate, String> {
        self.process(
            Event::Move {
                button: 0,
                point: finite_point(x, y)?,
            },
            now_ms,
        )
    }
    /// Complete input while preserving the final visible publication before native hiding.
    /// A non-finite release location interrupts input without matching, using shared recovery.
    pub fn end(&mut self, x: f64, y: f64, now_ms: f64) -> Result<RawUpdate, String> {
        self.process(
            Event::Up {
                button: 0,
                point: Point { x, y },
            },
            now_ms,
        )
    }
    /// Cancel without returning a match or reviving discarded geometry.
    pub fn cancel(&mut self, now_ms: f64) -> Result<RawUpdate, String> {
        self.process(Event::Cancel, now_ms)
    }
    /// Expire shared total/idle deadlines without physical input.
    pub fn tick(&mut self, now_ms: f64) -> Result<RawUpdate, String> {
        self.process(Event::Tick, now_ms)
    }
}

impl RawSession {
    fn validated_time(&self, now_ms: f64) -> Result<u64, String> {
        let now = duration_ms(now_ms)?;
        if now < self.last_time {
            return Err("Pointer timestamps must be monotonic".into());
        }
        Ok(now)
    }

    fn process(&mut self, event: Event, now_ms: f64) -> Result<RawUpdate, String> {
        let now = self.validated_time(now_ms)?;
        let released = matches!(event, Event::Up { .. });
        self.last_time = now;
        let output = self.engine.step(event, now);
        Ok(self.project(output, released))
    }

    fn project(&mut self, output: Output, released: bool) -> RawUpdate {
        let completed = released && !output.status.recorded_directions.is_empty();
        let mut matched = None;
        let mut final_trace = None;
        for effect in output.effects {
            match effect {
                Effect::Publish { trace, .. } if completed && trace.visible => {
                    final_trace = Some(trace)
                }
                Effect::Match(identifier) => matched = Some(identifier as u32),
                _ => {}
            }
        }
        let trace = if output.trace.visible {
            self.last_visible = Some(output.trace.clone());
            Some(output.trace)
        } else {
            // An unchanged release can hide the trace without publishing it again.
            let previous = self.last_visible.take();
            final_trace.or(previous.filter(|_| completed && !output.discard_pending_visible_trace))
        };
        RawUpdate {
            matched,
            trace: trace.filter(|trace| !trace.directions.is_empty()),
            active: output.stage != InputStage::Idle,
            deadline_ms: output.deadline_ns.map(|time| time as f64 / 1_000_000.),
            discard_pending_visible_trace: output.discard_pending_visible_trace,
            expired: output.status.failure == Some(Failure::Expired),
            last_session: output.last_session,
        }
    }
}

/// Plan a copied live trace only when the host chooses to present it.
#[cfg_attr(feature = "web", wasm_bindgen(js_name = rawLiveTrace))]
pub fn raw_live_trace(
    values: &[u8],
    points: &[f64],
    tail: &[f64],
    corner_radius_scale: f64,
) -> Result<Vec<f64>, String> {
    if !points.len().is_multiple_of(2) || !matches!(tail.len(), 0 | 2) {
        return Err("Trace coordinates must be x/y pairs with at most one tail".into());
    }
    let points: Vec<_> = points
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| Point { x: p[0], y: p[1] })
        .collect();
    let tail = tail.first().map(|&x| Point { x, y: tail[1] });
    live_trace(
        &directions(values)?,
        &points,
        tail,
        CornerStyle {
            radius_scale: corner_radius_scale,
        },
    )
    .map(|path| commands(&path))
    .map_err(|e| e.to_string())
}

fn finite_point(x: f64, y: f64) -> Result<Point, String> {
    let point = Point { x, y };
    if point.is_finite() {
        Ok(point)
    } else {
        Err("Pointer coordinates must be finite".into())
    }
}

/// Copied compact preview geometry using the existing shared pattern planner.
#[cfg_attr(feature = "web", wasm_bindgen)]
pub struct RawPattern {
    path: Vec<f64>,
    arrowhead: Vec<f64>,
}

#[cfg_attr(feature = "web", wasm_bindgen)]
impl RawPattern {
    /// Transfer encoded preview shaft commands to the JavaScript adapter.
    #[cfg_attr(feature = "web", wasm_bindgen(js_name = takeCommands))]
    pub fn take_commands(&mut self) -> Vec<f64> {
        std::mem::take(&mut self.path)
    }
    /// Transfer encoded closed arrowhead commands, if the preview has one.
    #[cfg_attr(feature = "web", wasm_bindgen(js_name = takeArrowhead))]
    pub fn take_arrowhead(&mut self) -> Vec<f64> {
        std::mem::take(&mut self.arrowhead)
    }
}

/// Plan arbitrary direction previews without a recognizer, DOM or graphics API.
#[cfg_attr(feature = "web", wasm_bindgen(js_name = rawPattern))]
pub fn raw_pattern(
    values: &[u8],
    width: f64,
    height: f64,
    padding: f64,
    line_width: f64,
    shows_arrowhead: bool,
    corner_radius_scale: f64,
) -> Result<RawPattern, String> {
    let geometry = pattern(
        &directions(values)?,
        width,
        height,
        padding,
        line_width,
        shows_arrowhead,
        corner_radius_scale,
    )
    .map_err(|error| error.to_string())?;
    Ok(RawPattern {
        path: commands(&geometry.path),
        arrowhead: geometry
            .arrowhead
            .as_ref()
            .map(commands)
            .unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> RawSession {
        RawSession::new(10., 25., 256, 64, 1_000., 0., false).unwrap()
    }

    #[test]
    fn final_match_keeps_the_final_geometry_after_native_hide() {
        let expected = live_trace(
            &[Direction::Down, Direction::Right],
            &[
                Point { x: 30., y: 35. },
                Point { x: 30., y: 100. },
                Point { x: 150., y: 100. },
            ],
            Some(Point { x: 150., y: 100. }),
            CornerStyle::default(),
        )
        .unwrap();
        // Aligned moves leave the final release unchanged when enough samples
        // have already been published. Both paths must return terminal geometry.
        for previous_x in [100., 150.] {
            let mut session = session();
            assert_eq!(session.register(&[2, 4], 7)[0], 0);
            session.begin(30., 35., 0.).unwrap();
            session.moved(30., 100., 10.).unwrap();
            session.moved(previous_x, 100., 20.).unwrap();
            let mut result = session.end(150., 100., 30.).unwrap();
            assert_eq!(result.matched(), Some(7));
            assert!(!result.active());
            let trace = result.take_trace().unwrap();
            assert_eq!(
                raw_live_trace(&trace.directions(), &trace.points(), &trace.tail(), 1.).unwrap(),
                commands(&expected),
            );
        }
    }

    #[test]
    fn cancellation_invalid_prefix_and_expiry_never_resurrect_a_match() {
        for event in [
            Event::Cancel,
            Event::Tick,
            Event::Move {
                button: 0,
                point: Point { x: 80., y: 120. },
            },
        ] {
            let mut session = session();
            assert_eq!(session.register(&[2], 0)[0], 0);
            session.begin(0., 0., 0.).unwrap();
            session.moved(0., 80., 20.).unwrap();
            let time = if matches!(event, Event::Tick) {
                1_001.
            } else {
                30.
            };
            let result = session.process(event, time).unwrap();
            assert_eq!(result.matched(), None);
            assert!(result.trace.is_none());
            assert!(session.end(80., 120., time + 1.).unwrap().trace.is_none());
        }
    }

    #[test]
    fn identifiers_replacement_snapshots_and_zero_are_preserved() {
        let mut session = session();
        assert_eq!(session.register(&[2], 0)[0], 0);
        session.begin(0., 0., 0.).unwrap();
        assert_eq!(session.register(&[2], u32::MAX)[0], 0);
        session.moved(0., 80., 20.).unwrap();
        assert_eq!(session.end(0., 100., 30.).unwrap().matched(), Some(0));
        session.begin(0., 0., 40.).unwrap();
        session.moved(0., 80., 50.).unwrap();
        assert!(session.begin(0., 0., 49.).is_err());
        let completed = session.end(0., 80., 60.).unwrap();
        assert_eq!(completed.matched(), Some(u32::MAX));
        assert!(completed.trace.is_some());
    }

    #[test]
    fn timestamps_and_configuration_are_validated_and_idle_deadlines_are_shared() {
        assert!(RawSession::new(-1., 25., 256, 64, 1_000., 0., false).is_err());
        assert!(RawSession::new(10., 25., 256, 64, f64::NAN, 0., false).is_err());
        let mut session = RawSession::new(10., 25., 256, 64, 0., 100., false).unwrap();
        assert_eq!(session.register(&[2], 1)[0], 0);
        assert_ne!(session.register(&[], 2)[0], 0);
        assert_ne!(session.register(&[5], 2)[0], 0);
        assert!(session.begin(f64::NAN, 0., 0.).is_err());
        assert_eq!(
            session.begin(0., 0., 10.).unwrap().deadline_ms(),
            Some(110.)
        );
        assert!(session.moved(0., 80., 9.).is_err());
        assert_eq!(
            session.moved(0., 80., 20.).unwrap().deadline_ms(),
            Some(120.)
        );
        assert!(!session.tick(120.).unwrap().active());
    }

    #[test]
    fn arbitrary_preview_delegates_to_the_existing_planner() {
        let mut result = raw_pattern(&[2, 4, 1, 3], 440., 240., 8., 4., true, 1.).unwrap();
        let expected = pattern(
            &[
                Direction::Down,
                Direction::Right,
                Direction::Up,
                Direction::Left,
            ],
            440.,
            240.,
            8.,
            4.,
            true,
            1.,
        )
        .unwrap();
        assert_eq!(result.take_commands(), commands(&expected.path));
        assert_eq!(
            result.take_arrowhead(),
            commands(expected.arrowhead.as_ref().unwrap())
        );
    }

    #[test]
    fn nonfinite_release_interrupts_but_rejected_time_preserves_input() {
        for active in [false, true] {
            let mut session = RawSession::new(10., 25., 256, 64, 0., 0., false).unwrap();
            assert_eq!(session.register(&[4], 42)[0], 0);
            session.begin(0., 0., 10.).unwrap();
            if active {
                session.moved(40., 0., 11.).unwrap();
            }
            assert!(session.end(f64::NAN, 0., 9.).is_err());
            assert!(session.end(f64::NAN, 0., f64::NAN).is_err());
            assert!(session.tick(12.).unwrap().active());
            let ended = session.end(f64::NAN, f64::INFINITY, 13.).unwrap();
            assert!(!ended.active());
            assert_eq!(ended.matched(), None);
            assert!(ended.discard_pending_visible_trace());
            assert_eq!(ended.deadline_ms(), None);
            assert!(ended.trace.is_none());
            assert_eq!(session.end(80., 0., 14.).unwrap().matched(), None);
            session.begin(0., 0., 15.).unwrap();
            session.moved(40., 0., 16.).unwrap();
            assert_eq!(session.end(80., 0., 17.).unwrap().matched(), Some(42));
        }
    }
}
