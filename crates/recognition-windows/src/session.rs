use crate::{Error, Policy, SessionContext};
use pointer_gesture_recognition::{
    Configuration, Disposition, Effect, Engine, Event, Matcher, Output, Point,
};

// Side-specific modifier virtual keys, in the same order as native snapshots.
#[cfg(windows)]
pub(crate) const MODIFIER_KEYS: [u16; 8] = [0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0x5b, 0x5c];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MouseAction {
    Move,
    Down,
    Up,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Injection {
    Mouse {
        point: Point,
        action: MouseAction,
        time: u32,
    },
    Key {
        index: usize,
        down: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pointer {
    pub point: Point,
    pub modifiers: u8,
    pub time: u32,
}

pub(crate) struct Delivery {
    pub output: Output,
    pub epoch: Option<u64>,
    pub press_id: u64,
    pub release: Option<Pointer>,
    pub consume: bool,
    pub injections: Vec<Injection>,
    pub logical_before: bool,
}

/// Session metadata belongs to the adapter, independently of engine snapshots.
pub(crate) struct Session {
    pub engine: Engine,
    pub button: u32,
    pub policy: Policy,
    epoch: Option<u64>,
    pub physical_down: bool,
    pub last: Pointer,
    press: Option<Pointer>,
    press_id: u64,
    captured: bool,
    logical_down: bool,
    close_pending: bool,
}
impl Session {
    #[cfg(windows)]
    pub(crate) fn current_press_id(&self) -> u64 {
        self.press_id
    }

    #[cfg(windows)]
    pub(crate) fn diagnostic_state(&self) -> crate::DiagnosticState {
        crate::DiagnosticState {
            physical_down: self.physical_down,
            captured: self.captured,
            logical_down: self.logical_down,
            epoch: self.epoch,
        }
    }

    pub fn new(config: Configuration, matcher: &Matcher) -> Result<Self, Error> {
        if config.button > 4 || config.maximum_points > 4096 {
            return Err(Error::InvalidConfiguration);
        }
        let mut engine = Engine::new(config).map_err(|_| Error::InvalidConfiguration)?;
        engine.use_matcher(matcher);
        Ok(Self {
            engine,
            button: config.button,
            policy: Policy::default(),
            epoch: None,
            physical_down: false,
            last: Pointer {
                point: Point { x: 0., y: 0. },
                modifiers: 0,
                time: 0,
            },
            press: None,
            press_id: 0,
            captured: false,
            logical_down: false,
            close_pending: false,
        })
    }

    pub fn use_context(&mut self, context: SessionContext) -> Result<(), Error> {
        if context.policy.required_modifiers > 15 {
            return Err(Error::InvalidConfiguration);
        }
        self.engine.use_matcher(&context.matcher);
        self.policy = context.policy;
        self.epoch = Some(context.epoch);
        Ok(())
    }

    pub fn pointer(&mut self, action: MouseAction, pointer: Pointer, now: u64) -> Delivery {
        self.last = pointer;
        let is_tracked_release = action == MouseAction::Up && self.physical_down;
        if action == MouseAction::Down && self.physical_down {
            // A repeated native down is still part of the original held press.
            // Cancel recognition without replacing its capture/replay lifetime;
            // only the balancing up permits a fresh recognition generation.
            let mut delivery = self.control(Event::Cancel, now);
            delivery.consume = true;
            delivery.output.disposition = Disposition::Consume;
            return delivery;
        }
        if action == MouseAction::Down {
            self.press_id = self.press_id.wrapping_add(1);
            self.physical_down = true;
            self.press = Some(pointer);
            self.captured = false;
            let groups = modifier_groups(pointer.modifiers);
            self.engine.step(
                Event::Policy {
                    enabled: self.policy.enabled,
                    modifiers_satisfied: groups & self.policy.required_modifiers
                        == self.policy.required_modifiers,
                },
                now,
            );
        } else if is_tracked_release {
            self.physical_down = false;
        }
        let was_logical = self.logical_down;
        let event = match action {
            MouseAction::Down => Event::Down {
                button: self.button,
                point: pointer.point,
            },
            MouseAction::Move => Event::Move {
                button: self.button,
                point: pointer.point,
            },
            MouseAction::Up => Event::Up {
                button: self.button,
                point: pointer.point,
            },
        };
        let output = self.engine.step(event, now);
        if action == MouseAction::Down {
            self.captured = output.disposition == Disposition::Consume;
        }
        let mut delivery = self.deliver(output);
        // A low-level Windows move must pass while the original press is hidden:
        // suppressing it freezes the system cursor and makes the next relative
        // packet/up refer to the old location. No target button-down was delivered.
        if action == MouseAction::Move && !was_logical && delivery.injections.is_empty() {
            delivery.consume = false;
        }
        if !self.close_pending && self.captured && was_logical {
            // Restored input remains an ordered native stream until physical up.
            // Suppress and queue its tail too: it may arrive before a deferred
            // down has been submitted outside the hook callback.
            if !delivery.injections.iter().any(|x| {
                matches!(
                    x,
                    Injection::Mouse {
                        action: MouseAction::Up,
                        ..
                    }
                )
            }) {
                delivery.injections.push(Injection::Mouse {
                    point: pointer.point,
                    action,
                    time: pointer.time,
                });
            }
            delivery.consume = true;
        } else if action == MouseAction::Up && self.captured {
            delivery.consume = true;
        }
        if action == MouseAction::Move && !delivery.injections.is_empty() && self.captured {
            delivery.consume = true;
        }
        if action == MouseAction::Up {
            // An up with no tracked down cannot close an earlier replay batch.
            delivery.release = is_tracked_release.then_some(pointer);
            if !self.close_pending {
                self.logical_down = false;
            }
            self.press = None;
            self.captured = false;
        }
        delivery.output.disposition = if delivery.consume {
            Disposition::Consume
        } else {
            Disposition::PassThrough
        };
        delivery
    }

    /// Keep a normally passed press behind native work already in flight.
    /// Its original up must follow the deferred down through the same queue.
    pub fn defer_passthrough_down(&mut self, delivery: &mut Delivery) {
        debug_assert!(self.physical_down && !self.captured && !delivery.consume);
        debug_assert_eq!(delivery.press_id, self.press_id);
        let press = self.press.unwrap_or(self.last);
        self.captured = true;
        self.logical_down = true;
        delivery.consume = true;
        delivery.output.disposition = Disposition::Consume;
        if !self.close_pending {
            delivery.injections.push(Injection::Mouse {
                point: press.point,
                action: MouseAction::Down,
                time: press.time,
            });
        }
    }

    pub fn control(&mut self, event: Event, now: u64) -> Delivery {
        let output = self.engine.step(event, now);
        self.deliver(output)
    }

    pub fn shutdown(&mut self, now: u64) -> Delivery {
        let mut delivery = self.control(Event::Stop, now);
        // Once hooks are removed, the remaining real release must have a down
        // to balance. Restore a held press instead of injecting a complete click
        // and letting a second real release escape after teardown.
        if !self.close_pending && self.physical_down && self.captured && !self.logical_down {
            let origin = self.press.unwrap_or(self.last).point;
            self.recover(
                &Effect::DragStart {
                    button: self.button,
                    points: vec![origin, self.last.point].into(),
                },
                &mut delivery.injections,
            );
        }
        if !self.close_pending && !self.physical_down && self.logical_down {
            delivery.injections.push(Injection::Mouse {
                point: self.last.point,
                action: MouseAction::Up,
                time: self.last.time,
            });
            self.logical_down = false;
        }
        delivery
    }

    fn deliver(&mut self, mut output: Output) -> Delivery {
        let consume = output.disposition == Disposition::Consume;
        let logical_before = self.logical_down;
        let mut injections = vec![];
        if !self.close_pending {
            for effect in &output.effects {
                self.recover(effect, &mut injections);
            }
        }
        // Input effects have already been applied by this adapter. Exposing them
        // as host work would invite duplicate input injection.
        output
            .effects
            .retain(|e| matches!(e, Effect::Match(_) | Effect::Publish { .. }));
        Delivery {
            output,
            epoch: self.epoch,
            press_id: self.press_id,
            release: None,
            consume,
            injections,
            logical_before,
        }
    }

    fn recover(&mut self, effect: &Effect, result: &mut Vec<Injection>) {
        let (points, complete) = match effect {
            Effect::Click { point, .. } => (std::slice::from_ref(point), !self.physical_down),
            Effect::Drag { points, .. } => (points.as_ref(), !self.physical_down),
            Effect::DragStart { points, .. } => (points.as_ref(), false),
            Effect::Release { point, .. } => {
                if self.logical_down && !self.physical_down {
                    result.push(Injection::Mouse {
                        point: *point,
                        action: MouseAction::Up,
                        time: self.last.time,
                    });
                    self.logical_down = false;
                }
                return;
            }
            _ => return,
        };
        let sampled = pointer_gesture_recognition::replay_points(points, 64);
        let points = sampled.as_slice();
        let Some(first) = points.first() else {
            return;
        };
        let press = self.press.unwrap_or(self.last);
        modifiers(self.last.modifiers, press.modifiers, result);
        result.push(Injection::Mouse {
            point: *first,
            action: MouseAction::Down,
            time: press.time,
        });
        self.logical_down = true;
        modifiers(press.modifiers, self.last.modifiers, result);
        for point in points.iter().skip(1) {
            result.push(Injection::Mouse {
                point: *point,
                action: MouseAction::Move,
                time: self.last.time,
            });
        }
        if complete {
            result.push(Injection::Mouse {
                point: *points.last().unwrap(),
                action: MouseAction::Up,
                time: self.last.time,
            });
            self.logical_down = false;
        }
        if points.last() != Some(&self.last.point) {
            result.push(Injection::Mouse {
                point: self.last.point,
                action: MouseAction::Move,
                time: self.last.time,
            });
        }
    }

    pub fn injection_failed(&mut self, logical_before: bool, accepted: &[Injection]) {
        self.logical_down = logical_before;
        for event in accepted {
            match event {
                Injection::Mouse {
                    action: MouseAction::Down,
                    ..
                } => self.logical_down = true,
                Injection::Mouse {
                    action: MouseAction::Up,
                    ..
                } => self.logical_down = false,
                _ => {}
            }
        }
    }

    /// Reconcile one failed native batch before discarding speculative queued work.
    /// A released older press must close before restoring the currently held one.
    pub fn reconcile_failed(
        &mut self,
        press_id: u64,
        logical_before: bool,
        accepted: &[Injection],
        release: Option<Pointer>,
    ) -> Option<Injection> {
        self.injection_failed(logical_before, accepted);
        if self.logical_down
            && let Some(release) = release
        {
            self.close_pending = true;
            return Some(Injection::Mouse {
                point: release.point,
                action: MouseAction::Up,
                time: release.time,
            });
        }
        // A changed identity requires a prior physical up. If its metadata was
        // lost, retain the open native state and forbid a replacement down.
        self.close_pending = self.logical_down && self.press_id != press_id;
        None
    }

    /// Only actual acceptance of the old up permits current-press recovery.
    pub fn replay_close_completed(&mut self, accepted: bool) {
        if self.close_pending && accepted {
            self.close_pending = false;
            self.logical_down = false;
        }
    }
}

fn modifiers(from: u8, to: u8, out: &mut Vec<Injection>) {
    for index in 0..8 {
        let bit = 1 << index;
        if from & bit != 0 && to & bit == 0 {
            out.push(Injection::Key { index, down: false });
        }
    }
    for index in 0..8 {
        let bit = 1 << index;
        if from & bit == 0 && to & bit != 0 {
            out.push(Injection::Key { index, down: true });
        }
    }
}

pub(crate) fn modifier_groups(sides: u8) -> u32 {
    (0..4).fold(0, |groups, i| {
        groups
            | if sides & (3 << (i * 2)) != 0 {
                1 << i
            } else {
                0
            }
    })
}

/// Map physical pixels, including negative monitor origins, to SendInput's
/// inclusive virtual-desktop coordinate range.
pub(crate) fn absolute(value: f64, origin: i32, extent: i32) -> i32 {
    if extent <= 1 {
        return 0;
    }
    ((value - f64::from(origin)) * 65535. / f64::from(extent - 1))
        .round()
        .clamp(0., 65535.) as i32
}
