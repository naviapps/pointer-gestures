//! One typed deterministic reducer shared by safe Rust and foreign-language hosts.
use crate::matcher::Trie;
use crate::{
    Configuration, Direction, Disposition, Effect, Event, Failure, InputStage,
    InvalidConfiguration, Matcher, Output, Point, SessionOutcome, SessionReport, Status, Trace,
};
use std::sync::Arc;

struct Capture {
    directions: Vec<Direction>,
    endpoints: Vec<Point>,
    raw: Vec<Point>,
    published: Arc<[Point]>,
    unpublished_count: usize,
    tail: Point,
}
struct Input {
    generation: u64,
    start: Point,
    points: Vec<Point>,
    started: u64,
    last_moved: u64,
    matcher: Arc<Trie>,
    cursor: Option<usize>,
    recording: bool,
    pressed_at: u64,
    last_observed_move: u64,
    latest: Point,
    point_count: u64,
    maximum_axis_distance: f64,
    direction_limit: usize,
    // None is pending input; Some is a valid capturing session. Abandoned input
    // is replayed and removed in the same step instead of persisting flag states.
    capture: Option<Capture>,
}
impl Input {
    fn observe(&mut self, point: Point, now: u64) {
        self.point_count = self.point_count.saturating_add(1);
        if point != self.latest {
            self.last_moved = now;
            self.last_observed_move = now;
        }
        self.latest = point;
        let distance = (point.x - self.start.x)
            .abs()
            .max((point.y - self.start.y).abs())
            .min(f64::MAX);
        self.maximum_axis_distance = self.maximum_axis_distance.max(distance);
    }
    fn report(
        &self,
        outcome: SessionOutcome,
        now: u64,
        rejected_direction: Option<Direction>,
    ) -> SessionReport {
        SessionReport {
            generation: self.generation,
            outcome,
            elapsed_ns: now.saturating_sub(self.pressed_at),
            idle_ns: now.saturating_sub(self.last_observed_move),
            point_count: self.point_count,
            maximum_axis_distance: self.maximum_axis_distance,
            captured: self.capture.is_some(),
            recording: self.recording,
            directions: self
                .capture
                .as_ref()
                .map_or_else(Arc::default, |c| Arc::from(c.directions.as_slice())),
            rejected_direction,
        }
    }
}
impl Trace {
    fn from_input(input: Option<&Input>, previous: &Self) -> Self {
        let Some(capture) = input.and_then(|i| i.capture.as_ref()) else {
            return Self::default();
        };
        Self {
            visible: true,
            directions: if previous.directions.as_ref() == capture.directions.as_slice() {
                Arc::clone(&previous.directions)
            } else {
                Arc::from(capture.directions.as_slice())
            },
            endpoints: Arc::from(capture.endpoints.as_slice()),
            raw_points: Arc::clone(&capture.published),
            tail: Some(capture.tail),
        }
    }
}

/// Serial recognition. Call `close` and deliver its recovery effects before dropping
/// the input adapter. Dropping memory alone cannot recover OS input.
pub struct Engine {
    config: Configuration,
    ready: bool,
    recording: bool,
    enabled: bool,
    modifiers: bool,
    failure: Option<Failure>,
    matcher: Arc<Trie>,
    generation: u64,
    input: Option<Input>,
    recorded: Arc<[Direction]>,
    effects: Vec<Effect>,
    trace: Trace,
    discard_visible: bool,
    last_session: Option<SessionReport>,
}
impl Engine {
    /// Create a stopped engine with an empty matcher and enabled host policy.
    /// Invalid distances, a zero point limit or a direction budget outside 1..=4096 return [`InvalidConfiguration`].
    pub fn new(configuration: Configuration) -> Result<Self, InvalidConfiguration> {
        if !configuration.valid() {
            return Err(InvalidConfiguration);
        }
        Ok(Self {
            config: configuration,
            ready: false,
            recording: false,
            enabled: true,
            modifiers: true,
            failure: None,
            matcher: Arc::new(Trie::new()),
            generation: 0,
            input: None,
            recorded: Arc::default(),
            effects: vec![],
            trace: Trace::default(),
            discard_visible: false,
            last_session: None,
        })
    }
    /// Future input uses the new matcher; active input retains its captured snapshot.
    pub fn use_matcher(&mut self, matcher: &Matcher) {
        self.matcher = Arc::clone(&matcher.storage);
    }
    /// Replace mutable tuning and restart active deadlines at `now_ns` when values
    /// change or `reset_unchanged` is true. Returns whether an update was applied.
    /// A smaller point limit trims the active trace immediately and bounds recovery.
    /// Invalid tuning or a changed button/empty-matcher policy leaves state unchanged.
    pub fn update_tuning(
        &mut self,
        configuration: Configuration,
        now_ns: u64,
        reset_unchanged: bool,
    ) -> Result<bool, InvalidConfiguration> {
        if !configuration.valid()
            || configuration.button != self.config.button
            || configuration.passes_through_empty_matcher
                != self.config.passes_through_empty_matcher
        {
            return Err(InvalidConfiguration);
        }
        if configuration == self.config && !reset_unchanged {
            return Ok(false);
        }
        let point_limit_reduced = configuration.maximum_points < self.config.maximum_points;
        self.config = configuration;
        if let Some(i) = &mut self.input {
            i.started = now_ns;
            i.last_moved = now_ns;
            if point_limit_reduced {
                Self::trim_replay_points(&mut i.points, configuration.maximum_points);
                if let Some(capture) = &mut i.capture
                    && capture.raw.len() > configuration.maximum_points
                {
                    let excess = capture.raw.len() - configuration.maximum_points;
                    capture.raw.drain(..excess);
                    capture.published = Arc::from(capture.raw.as_slice());
                    capture.unpublished_count = 0;
                    self.trace.raw_points = Arc::clone(&capture.published);
                }
            }
        }
        Ok(true)
    }
    /// Stop and consume the engine. Deliver every returned recovery effect before
    /// disposing the input adapter; dropping the engine alone cannot replay input.
    pub fn close(mut self, now_ns: u64) -> Output {
        self.step(Event::Stop, now_ns)
    }

    fn status(&self, capturing: bool) -> Status {
        Status {
            ready: self.ready,
            capturing,
            recording: self.recording,
            failure: self.failure,
            recorded_directions: Arc::clone(&self.recorded),
        }
    }
    fn publish_trace(&mut self, input: Option<&Input>) {
        let next = Trace::from_input(input, &self.trace);
        // Movement usually changes the tail. Decide that before comparing long
        // endpoint arrays; equal tails still require the complete trace check.
        if (self.trace.tail != next.tail || self.trace != next)
            && (self.trace.visible || next.visible)
        {
            self.effects.push(Effect::Publish {
                status: self.status(input.is_some_and(|i| i.capture.is_some())),
                trace: next.clone(),
            });
        }
        self.trace = next;
    }
    fn release(&mut self, point: Point) {
        self.effects.push(Effect::Release {
            button: self.config.button,
            point,
        });
    }
    fn consumed(&mut self, points: &[Point], click: Point) {
        self.effects.push(if points.len() > 1 {
            Effect::Drag {
                button: self.config.button,
                points: Arc::from(points),
            }
        } else {
            Effect::Click {
                button: self.config.button,
                point: click,
            }
        });
    }
    fn pending_replay(&mut self, i: Input) {
        // Classification uses observed motion even when the replay budget
        // has evicted the sample farthest from the origin.
        if i.points.len() > 1 && i.maximum_axis_distance >= self.config.start_distance {
            self.effects.push(Effect::Drag {
                button: self.config.button,
                points: Arc::from(i.points),
            });
        } else {
            self.effects.push(Effect::Click {
                button: self.config.button,
                point: i.start,
            });
        }
    }
    fn interrupt(&mut self, outcome: SessionOutcome, now: u64) {
        self.discard_visible = true;
        if let Some(i) = self.input.take() {
            self.last_session = Some(i.report(outcome, now, None));
            if let Some(capture) = &i.capture {
                self.release(*capture.raw.last().unwrap_or(&i.start));
            } else {
                self.pending_replay(i);
            }
        }
        self.publish_trace(None);
    }
    fn interrupt_retained_input(&mut self, outcome: SessionOutcome, now: u64) -> bool {
        if self.input.is_none() {
            return false;
        }
        self.interrupt(outcome, now);
        self.failure = None;
        true
    }
    fn expire(&mut self, now: u64) {
        if let Some((deadline, outcome)) =
            self.input.as_ref().and_then(|input| self.deadline(input))
            && now >= deadline
        {
            self.failure = Some(Failure::Expired);
            self.interrupt(outcome, now);
        }
    }
    fn append(&self, points: &mut Vec<Point>, point: Point) {
        if points.last() != Some(&point) {
            points.push(point);
        }
        Self::trim_replay_points(points, self.config.maximum_points);
    }
    fn trim_replay_points(points: &mut Vec<Point>, maximum: usize) {
        let maximum = maximum.max(2);
        if points.len() > maximum {
            points.drain(1..1 + points.len() - maximum);
        }
    }
    // A rejected turn ends the retained input; the caller selects the recovery effect.
    fn advance(
        &self,
        input: &mut Input,
        point: Point,
        immediate: bool,
    ) -> Result<(), (SessionOutcome, Direction)> {
        let capture = input
            .capture
            .as_mut()
            .expect("advance requires an active capture");
        if capture.tail == point {
            if immediate && capture.unpublished_count != 0 {
                capture.unpublished_count = 0;
                capture.published = Arc::from(capture.raw.as_slice());
            }
            return Ok(());
        }
        self.append(&mut input.points, point);
        capture.raw.push(point);
        capture.unpublished_count += 1;
        capture.tail = point;
        let excess = capture.raw.len().saturating_sub(self.config.maximum_points);
        capture.raw.drain(..excess);
        let endpoint = *capture
            .endpoints
            .last()
            .expect("capture has a start endpoint");
        let (direction, distance) = movement(endpoint, point);
        let mut appended = false;
        if let Some(direction) = direction {
            if capture.directions.last() == Some(&direction) {
                *capture.endpoints.last_mut().unwrap() = point;
            } else if capture.directions.is_empty() || distance >= self.config.change_distance {
                if capture.directions.len() >= input.direction_limit {
                    return Err((SessionOutcome::DirectionLimit, direction));
                }
                let cursor = input.matcher.advance(input.cursor, direction);
                if !input.recording && cursor.is_none() {
                    return Err((SessionOutcome::UnmatchedPrefix, direction));
                }
                input.cursor = cursor;
                capture.directions.push(direction);
                capture.endpoints.push(point);
                appended = true;
            }
        }
        if immediate || capture.unpublished_count >= 10 || appended {
            capture.unpublished_count = 0;
            capture.published = Arc::from(capture.raw.as_slice());
        }
        Ok(())
    }
    fn reject_turn(
        &mut self,
        input: &Input,
        (outcome, direction): (SessionOutcome, Direction),
        now: u64,
    ) {
        self.last_session = Some(input.report(outcome, now, Some(direction)));
        self.discard_visible = true;
        self.publish_trace(None);
    }
    /// End retained input before a replacement press whose policy needs host callbacks.
    /// Deliver the returned recovery before resolving that policy and submitting
    /// the actual button-down. An elapsed deadline takes precedence over replacement.
    pub fn prepare_replacement_press(&mut self, now_ns: u64) -> Output {
        self.begin_step();
        if self.ready {
            self.expire(now_ns);
            self.interrupt_retained_input(SessionOutcome::Superseded, now_ns);
        }
        self.output(Disposition::Control)
    }

    fn begin_step(&mut self) {
        self.effects.clear();
        self.discard_visible = false;
    }

    /// Process one event at a host-supplied monotonic nanosecond timestamp.
    /// Apply the returned disposition to the original input and deliver effects in
    /// order before processing another event. Schedule [`Event::Tick`] at the
    /// returned deadline; snapshots alone do not expire input. Outputs own their data.
    pub fn step(&mut self, event: Event, now_ns: u64) -> Output {
        self.begin_step();
        let disposition = match event {
            Event::Stop => {
                if self.ready {
                    self.ready = false;
                    self.interrupt(SessionOutcome::Stopped, now_ns);
                    self.failure = None;
                }
                Disposition::Control
            }
            Event::Start => {
                if !self.ready {
                    self.ready = true;
                    self.failure = None;
                }
                Disposition::Control
            }
            Event::Recording(value) => {
                self.recording = value;
                Disposition::Control
            }
            Event::Policy {
                enabled,
                modifiers_satisfied,
            } => {
                self.enabled = enabled;
                self.modifiers = modifiers_satisfied;
                Disposition::Control
            }
            Event::Snapshot => Disposition::Control,
            Event::Tick => {
                self.expire(now_ns);
                Disposition::Control
            }
            Event::Cancel => {
                if self.ready {
                    self.expire(now_ns);
                }
                if self.ready && self.interrupt_retained_input(SessionOutcome::Cancelled, now_ns) {
                    Disposition::Consume
                } else {
                    Disposition::PassThrough
                }
            }
            Event::Down { button, point }
            | Event::Move { button, point }
            | Event::Up { button, point } => {
                if !self.ready {
                    Disposition::PassThrough
                } else if !point.is_finite() {
                    if matches!(event, Event::Up { .. })
                        && button == self.config.button
                        && self.input.is_some()
                    {
                        // A physical release ends retained input even when its
                        // location is unusable. Recover from finite history only;
                        // never match or leave capture waiting for another up.
                        self.expire(now_ns);
                        self.interrupt(SessionOutcome::InvalidRelease, now_ns);
                        Disposition::Consume
                    } else {
                        Disposition::PassThrough
                    }
                } else {
                    self.expire(now_ns);
                    if button != self.config.button {
                        Disposition::PassThrough
                    } else {
                        match event {
                            Event::Down { .. } => self.down(point, now_ns),
                            Event::Move { .. } => self.moved(point, now_ns),
                            Event::Up { .. } => self.up(point, now_ns),
                            _ => unreachable!(),
                        }
                    }
                }
            }
        };
        self.output(disposition)
    }
    fn down(&mut self, point: Point, now: u64) -> Disposition {
        self.interrupt_retained_input(SessionOutcome::Superseded, now);
        if !self.enabled {
            self.failure = Some(Failure::Disabled);
            return Disposition::PassThrough;
        }
        if !self.recording && !self.modifiers {
            self.failure = Some(Failure::ModifiersNotSatisfied);
            return Disposition::PassThrough;
        }
        self.failure = None;
        if !self.recording && self.config.passes_through_empty_matcher && self.matcher.is_empty() {
            return Disposition::PassThrough;
        }
        self.generation = self.generation.wrapping_add(1).max(1);
        self.input = Some(Input {
            generation: self.generation,
            start: point,
            points: vec![point],
            started: now,
            last_moved: now,
            matcher: Arc::clone(&self.matcher),
            cursor: Some(0),
            recording: self.recording,
            pressed_at: now,
            last_observed_move: now,
            latest: point,
            point_count: 1,
            maximum_axis_distance: 0.0,
            direction_limit: self.config.maximum_directions,
            capture: None,
        });
        Disposition::Consume
    }
    fn try_begin_capture(&self, input: &mut Input, point: Point) -> bool {
        let (direction, distance) = movement(input.start, point);
        if direction.is_none() || distance < self.config.start_distance {
            return false;
        }
        input.capture = Some(Capture {
            directions: vec![],
            endpoints: vec![input.start],
            raw: vec![input.start],
            published: Arc::from([input.start].as_slice()),
            unpublished_count: 0,
            tail: input.start,
        });
        true
    }
    fn moved(&mut self, point: Point, now: u64) -> Disposition {
        let Some(mut input) = self.input.take() else {
            return Disposition::PassThrough;
        };
        input.observe(point, now);
        if input
            .capture
            .as_ref()
            .is_some_and(|capture| capture.tail == point)
        {
            self.input = Some(input);
            return Disposition::Consume;
        }
        if input.capture.is_none() && !self.try_begin_capture(&mut input, point) {
            self.append(&mut input.points, point);
            self.input = Some(input);
            return Disposition::Consume;
        }
        if let Err(rejection) = self.advance(&mut input, point, false) {
            self.reject_turn(&input, rejection, now);
            self.effects.push(Effect::DragStart {
                button: self.config.button,
                points: Arc::from(input.points.as_slice()),
            });
            return Disposition::Consume;
        }
        self.publish_trace(Some(&input));
        self.input = Some(input);
        Disposition::Consume
    }
    fn up(&mut self, point: Point, now: u64) -> Disposition {
        let Some(mut input) = self.input.take() else {
            return Disposition::PassThrough;
        };
        // Only recording may start on release, and only after real drag motion.
        // Check the earlier samples before the release can contribute displacement.
        if input.capture.is_none() && input.recording && input.maximum_axis_distance > 0.0 {
            self.try_begin_capture(&mut input, point);
        }
        input.observe(point, now);
        if input.capture.is_none() {
            self.last_session = Some(input.report(SessionOutcome::BelowThreshold, now, None));
            self.append(&mut input.points, point);
            self.pending_replay(input);
            return Disposition::Consume;
        }
        if let Err(rejection) = self.advance(&mut input, point, true) {
            self.reject_turn(&input, rejection, now);
            self.recorded = Arc::default();
            self.consumed(&input.points, input.start);
            return Disposition::Consume;
        }
        self.publish_trace(Some(&input));
        let capture = input.capture.as_ref().unwrap();
        let matched = if capture.directions.is_empty() || input.recording {
            None
        } else {
            input
                .cursor
                .and_then(|cursor| input.matcher.nodes[cursor].terminal)
        };
        self.recorded = Arc::from(capture.directions.as_slice());
        self.last_session = Some(input.report(
            if input.recording {
                SessionOutcome::Recorded
            } else if matched.is_some() {
                SessionOutcome::Matched
            } else {
                SessionOutcome::Unmatched
            },
            now,
            None,
        ));
        self.publish_trace(None);
        if matched.is_none() {
            self.consumed(&input.points, input.start);
        } else {
            self.release(point);
        }
        if let Some(value) = matched {
            self.effects.push(Effect::Match(value));
        }
        Disposition::Consume
    }
    // Both publication and expiration use the same deadline. Total duration wins ties.
    fn deadline(&self, input: &Input) -> Option<(u64, SessionOutcome)> {
        let total = (self.config.total_duration_ns > 0)
            .then(|| input.started.saturating_add(self.config.total_duration_ns));
        let idle = (self.config.idle_duration_ns > 0).then(|| {
            input
                .last_moved
                .saturating_add(self.config.idle_duration_ns)
        });
        match (total, idle) {
            (Some(total), Some(idle)) if idle < total => Some((idle, SessionOutcome::IdleDeadline)),
            (Some(total), _) => Some((total, SessionOutcome::TotalDeadline)),
            (None, Some(idle)) => Some((idle, SessionOutcome::IdleDeadline)),
            (None, None) => None,
        }
    }
    fn output(&mut self, disposition: Disposition) -> Output {
        let stage = self.input.as_ref().map_or(InputStage::Idle, |i| {
            if i.capture.is_some() {
                InputStage::Active
            } else {
                InputStage::Pending
            }
        });
        let deadline_ns = self
            .input
            .as_ref()
            .and_then(|input| self.deadline(input))
            .map(|(deadline, _)| deadline);
        Output {
            last_session: self.last_session.clone(),
            disposition,
            stage,
            status: self.status(stage == InputStage::Active),
            trace: self.trace.clone(),
            input_generation: self.input.as_ref().map_or(0, |i| i.generation),
            discard_pending_visible_trace: self.discard_visible,
            deadline_ns,
            effects: std::mem::take(&mut self.effects),
        }
    }
}
fn movement(start: Point, end: Point) -> (Option<Direction>, f64) {
    crate::resolve_movement(start, end).map_or((None, 0.0), |m| (m.direction, m.maximum_axis_delta))
}
