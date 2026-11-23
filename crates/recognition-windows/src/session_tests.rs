use crate::{Error, Policy, SessionContext, session::*};
use pointer_gesture_recognition::{Configuration, Direction, Effect, Event, Matcher};

fn session() -> Session {
    let mut matcher = Matcher::default();
    matcher.register(&[Direction::Right], 42).unwrap();
    Session::new(
        Configuration {
            start_distance: 10.,
            change_distance: 5.,
            total_duration_ns: 100,
            idle_duration_ns: 0,
            maximum_points: 256,
            maximum_directions: 64,
            button: 1,
            passes_through_empty_matcher: true,
        },
        &matcher,
    )
    .unwrap()
}
fn pointer(x: f64, modifiers: u8, time: u32) -> Pointer {
    Pointer {
        point: pointer_gesture_recognition::Point { x, y: 10. },
        modifiers,
        time,
    }
}
fn buttons(delivery: &Delivery) -> Vec<MouseAction> {
    delivery
        .injections
        .iter()
        .filter_map(|event| match event {
            Injection::Mouse { action, .. } if *action != MouseAction::Move => Some(*action),
            _ => None,
        })
        .collect()
}
fn started() -> Session {
    let mut s = session();
    s.control(Event::Start, 0);
    s
}

fn context(identifier: u64, epoch: u64, policy: Policy) -> SessionContext {
    let mut matcher = Matcher::default();
    matcher.register(&[Direction::Right], identifier).unwrap();
    SessionContext {
        matcher,
        policy,
        epoch,
    }
}

#[test]
fn per_press_context_captures_matcher_policy_and_epoch_until_release() {
    let mut s = started();
    let old = context(11, 101, Policy::default());
    s.use_context(old).unwrap();
    let down = s.pointer(MouseAction::Down, pointer(20., 0, 1), 1);
    assert!(down.consume);
    assert_eq!(down.epoch, Some(101));
    let next = context(22, 202, Policy::default());
    // A control update publishes future registrations/enablement. The held
    // engine session and already created deliveries retain their old snapshot.
    s.engine.use_matcher(&next.matcher);
    s.policy.enabled = false;
    s.pointer(MouseAction::Move, pointer(40., 0, 2), 2);
    let up = s.pointer(MouseAction::Up, pointer(40., 0, 3), 3);
    assert_eq!(up.epoch, Some(101));
    assert!(up.output.effects.contains(&Effect::Match(11)));
    assert_eq!(down.epoch, Some(101));

    s.use_context(next).unwrap();
    s.pointer(MouseAction::Down, pointer(20., 0, 4), 4);
    s.pointer(MouseAction::Move, pointer(40., 0, 5), 5);
    let up = s.pointer(MouseAction::Up, pointer(40., 0, 6), 6);
    assert_eq!(up.epoch, Some(202));
    assert!(up.output.effects.contains(&Effect::Match(22)));
}

#[test]
fn per_press_disabled_or_missing_modifier_policy_passes_original_input() {
    for policy in [
        Policy {
            enabled: false,
            required_modifiers: 0,
        },
        Policy {
            enabled: true,
            required_modifiers: 2,
        },
    ] {
        let mut s = started();
        s.use_context(context(11, 101, policy)).unwrap();
        let down = s.pointer(MouseAction::Down, pointer(20., 0, 1), 1);
        assert!(!down.consume);
        assert_eq!(down.epoch, Some(101));
        s.pointer(MouseAction::Move, pointer(40., 0, 2), 2);
        let up = s.pointer(MouseAction::Up, pointer(40., 0, 3), 3);
        assert!(!up.consume);
        assert!(up.injections.is_empty());
        assert!(
            !up.output
                .effects
                .iter()
                .any(|effect| matches!(effect, Effect::Match(_)))
        );
    }
}

#[test]
fn invalid_context_preserves_prior_snapshot_and_interruption_retains_epoch() {
    let mut s = started();
    s.use_context(context(11, 101, Policy::default())).unwrap();
    assert_eq!(
        s.use_context(context(
            22,
            202,
            Policy {
                enabled: true,
                required_modifiers: 16
            }
        )),
        Err(Error::InvalidConfiguration)
    );
    let down = s.pointer(MouseAction::Down, pointer(20., 0, 1), 1);
    assert_eq!(down.epoch, Some(101));
    let cancel = s.control(Event::Cancel, 2);
    assert_eq!(cancel.epoch, Some(101));
    assert_eq!(buttons(&cancel), [MouseAction::Down]);
    assert_eq!(s.shutdown(3).epoch, Some(101));
}

#[test]
fn complete_click_is_delivered_once_and_not_returned_as_host_work() {
    let mut s = started();
    assert!(s.pointer(MouseAction::Down, pointer(20., 0, 1), 1).consume);
    let up = s.pointer(MouseAction::Up, pointer(21., 0, 3), 3);
    assert!(up.consume);
    assert_eq!(buttons(&up), [MouseAction::Down, MouseAction::Up]);
    assert!(
        up.output
            .effects
            .iter()
            .all(|e| matches!(e, Effect::Match(_) | Effect::Publish { .. }))
    );
    assert!(s.shutdown(4).injections.is_empty());
}

#[test]
fn hidden_press_keeps_native_cursor_moves_available() {
    let mut s = started();
    s.pointer(MouseAction::Down, pointer(20., 0, 1), 1);
    let moved = s.pointer(MouseAction::Move, pointer(40., 0, 2), 2);
    assert!(!moved.consume);
    assert_eq!(
        moved.output.disposition,
        pointer_gesture_recognition::Disposition::PassThrough
    );
    assert!(moved.injections.is_empty());
}

#[test]
fn match_has_no_naked_native_release_or_replayed_click() {
    let mut s = started();
    s.pointer(MouseAction::Down, pointer(20., 0, 1), 1);
    s.pointer(MouseAction::Move, pointer(40., 0, 2), 2);
    let up = s.pointer(MouseAction::Up, pointer(40., 0, 3), 3);
    assert!(up.consume);
    assert!(up.injections.is_empty());
    assert!(up.output.effects.contains(&Effect::Match(42)));
}

#[test]
fn rejected_prefix_keeps_deferred_down_move_and_up_ordered() {
    let mut s = started();
    s.pointer(MouseAction::Down, pointer(20., 0, 1), 1);
    let rejected = s.pointer(MouseAction::Move, pointer(0., 0, 2), 2);
    assert!(rejected.consume);
    assert_eq!(buttons(&rejected), [MouseAction::Down]);
    let moved = s.pointer(MouseAction::Move, pointer(-5., 0, 3), 3);
    assert!(moved.consume);
    assert!(matches!(
        moved.injections.as_slice(),
        [Injection::Mouse {
            action: MouseAction::Move,
            ..
        }]
    ));
    let up = s.pointer(MouseAction::Up, pointer(-5., 0, 4), 4);
    assert!(up.consume);
    assert_eq!(buttons(&up), [MouseAction::Up]);
    assert!(s.shutdown(5).injections.is_empty());
}

#[test]
fn cancelling_pending_input_restores_held_down_without_double_up() {
    let mut s = started();
    s.pointer(MouseAction::Down, pointer(20., 0, 1), 1);
    let cancel = s.control(Event::Cancel, 2);
    assert_eq!(buttons(&cancel), [MouseAction::Down]);
    let up = s.pointer(MouseAction::Up, pointer(20., 0, 3), 3);
    assert!(up.consume);
    assert_eq!(buttons(&up), [MouseAction::Up]);
}

#[test]
fn pending_replay_keeps_up_ordered_when_it_arrives_before_batch_completion() {
    let mut s = started();
    s.use_context(context(42, 101, Policy::default())).unwrap();
    s.pointer(MouseAction::Down, pointer(20., 0, 10), 1);
    s.pointer(MouseAction::Move, pointer(21., 0, 11), 2);
    let in_flight = s.control(Event::Cancel, 3);
    assert_eq!(buttons(&in_flight), [MouseAction::Down]);
    assert_eq!(in_flight.injections.len(), 2);
    assert!(!in_flight.logical_before);

    // The owner pumps the physical up while native submission of the recovered
    // down is still in flight. This up must wait behind that older batch.
    let queued_up = s.pointer(MouseAction::Up, pointer(21., 0, 12), 4);
    assert!(queued_up.consume && queued_up.logical_before);
    assert!(!s.physical_down);
    assert_eq!(buttons(&queued_up), [MouseAction::Up]);
    assert_eq!(in_flight.epoch, Some(101));
    assert_eq!(queued_up.epoch, in_flight.epoch);
    assert_eq!(
        [buttons(&in_flight), buttons(&queued_up)].concat(),
        [MouseAction::Down, MouseAction::Up]
    );
    for delivery in [&in_flight, &queued_up] {
        assert!(
            !delivery
                .output
                .effects
                .iter()
                .any(|effect| matches!(effect, Effect::Match(_)))
        );
    }
    // Both batches completed successfully in order; no further release is owed.
    assert!(s.shutdown(5).injections.is_empty());
}

#[test]
fn failed_pending_replay_reconciles_accepted_prefix_after_up_already_queued() {
    for accepted in 0..=1 {
        let mut s = started();
        s.use_context(context(42, 101, Policy::default())).unwrap();
        s.pointer(MouseAction::Down, pointer(20., 0, 10), 1);
        s.pointer(MouseAction::Move, pointer(21., 0, 11), 2);
        let in_flight = s.control(Event::Cancel, 3);
        assert_eq!(in_flight.injections.len(), 2);
        let queued_up = s.pointer(MouseAction::Up, pointer(21., 0, 12), 4);
        assert!(queued_up.consume && queued_up.logical_before);
        assert_eq!(buttons(&queued_up), [MouseAction::Up]);

        // The earlier native call now fails. Its actual accepted prefix wins
        // over speculative state changes from the already processed physical up.
        // The owner discards the queued up instead of sending it after failure.
        s.injection_failed(in_flight.logical_before, &in_flight.injections[..accepted]);
        drop(queued_up);
        let stop = s.shutdown(5);
        assert_eq!(stop.epoch, Some(101));
        let delivered = Delivery {
            injections: in_flight.injections[..accepted].to_vec(),
            ..in_flight
        };
        if accepted == 0 {
            assert!(stop.injections.is_empty());
            assert!(buttons(&delivered).is_empty());
        } else {
            assert_eq!(
                [buttons(&delivered), buttons(&stop)].concat(),
                [MouseAction::Down, MouseAction::Up]
            );
            assert_eq!(
                stop.injections,
                [Injection::Mouse {
                    point: pointer(21., 0, 0).point,
                    action: MouseAction::Up,
                    time: 12,
                }]
            );
        }
        assert!(
            !stop
                .output
                .effects
                .iter()
                .any(|effect| matches!(effect, Effect::Match(_)))
        );
        assert!(s.shutdown(6).injections.is_empty());
    }
}

#[test]
fn physical_press_identity_is_independent_of_host_context_and_duplicate_downs() {
    let mut s = started();
    let first = s.pointer(MouseAction::Down, pointer(20., 0, 1), 1);
    let duplicate = s.pointer(MouseAction::Down, pointer(21., 0, 2), 2);
    let release = s.pointer(MouseAction::Up, pointer(22., 0, 3), 3);
    assert_eq!(first.epoch, None);
    assert_eq!(duplicate.press_id, first.press_id);
    assert_eq!(release.press_id, first.press_id);
    assert_eq!(release.release, Some(pointer(22., 0, 3)));
    assert!(first.release.is_none() && duplicate.release.is_none());

    s.use_context(context(42, 101, Policy::default())).unwrap();
    let second = s.pointer(MouseAction::Down, pointer(30., 0, 4), 4);
    s.pointer(MouseAction::Up, pointer(30., 0, 5), 5);
    s.use_context(context(42, 101, Policy::default())).unwrap();
    let third = s.pointer(MouseAction::Down, pointer(40., 0, 6), 6);
    assert_ne!(second.press_id, first.press_id);
    assert_ne!(third.press_id, second.press_id);
    assert_eq!(second.epoch, third.epoch);
}

#[derive(Clone, Copy, Debug)]
enum LaterPress {
    Pending,
    Active,
    Released,
    Disabled,
    EmptyMatcher,
    ReleasedThenHeld,
}

impl LaterPress {
    const ALL: [Self; 6] = [
        Self::Pending,
        Self::Active,
        Self::Released,
        Self::Disabled,
        Self::EmptyMatcher,
        Self::ReleasedThenHeld,
    ];

    fn enter(self, s: &mut Session) -> Pointer {
        if matches!(self, Self::Disabled) {
            s.policy.enabled = false;
        }
        if matches!(self, Self::EmptyMatcher) {
            s.engine.use_matcher(&Matcher::default());
        }
        let press = pointer(50., 0, 20);
        let mut down = s.pointer(MouseAction::Down, press, 5);
        if matches!(self, Self::Disabled | Self::EmptyMatcher) {
            assert!(!down.consume && down.injections.is_empty());
            s.defer_passthrough_down(&mut down);
            assert!(down.consume);
            assert_eq!(buttons(&down), [MouseAction::Down]);
        } else {
            assert!(down.consume);
        }
        if matches!(self, Self::Active) {
            s.pointer(MouseAction::Move, pointer(70., 0, 21), 6);
        }
        if matches!(self, Self::Released | Self::ReleasedThenHeld) {
            let up = s.pointer(MouseAction::Up, pointer(51., 0, 22), 7);
            assert!(up.consume);
        }
        if matches!(self, Self::ReleasedThenHeld) {
            let press = pointer(80., 0, 30);
            s.pointer(MouseAction::Down, press, 8);
            press
        } else {
            press
        }
    }
}

fn pending_replay_with_release() -> (Session, Delivery, Pointer) {
    let mut s = started();
    s.use_context(context(42, 101, Policy::default())).unwrap();
    s.pointer(MouseAction::Down, pointer(20., 0, 10), 1);
    s.pointer(MouseAction::Move, pointer(21., 0, 11), 2);
    let in_flight = s.control(Event::Cancel, 3);
    assert_eq!(in_flight.injections.len(), 2);
    let release = pointer(23., 0, 12);
    let up = s.pointer(MouseAction::Up, release, 4);
    assert_eq!(up.press_id, in_flight.press_id);
    assert_eq!(up.release, Some(release));
    (s, in_flight, release)
}

#[test]
fn failed_old_replay_closes_its_native_press_before_recovering_a_later_held_press() {
    for later in LaterPress::ALL {
        for accepted in 0..=2 {
            let (mut s, in_flight, release) = pending_replay_with_release();
            let current = later.enter(&mut s);
            let close = s.reconcile_failed(
                in_flight.press_id,
                in_flight.logical_before,
                &in_flight.injections[..accepted],
                Some(release),
            );
            if accepted == 0 {
                assert_eq!(close, None, "{later:?}");
            } else {
                assert_eq!(
                    close,
                    Some(Injection::Mouse {
                        point: release.point,
                        action: MouseAction::Up,
                        time: release.time,
                    }),
                    "{later:?}"
                );
                s.replay_close_completed(true);
                // A second acknowledgment cannot change the current press.
                s.replay_close_completed(true);
            }
            let stop = s.shutdown(9);
            if matches!(later, LaterPress::Released) {
                // An observable fatal replay error discards never-submitted work.
                // A fully released later click does not become a new held down.
                assert!(stop.injections.is_empty());
            } else {
                assert_eq!(buttons(&stop), [MouseAction::Down], "{later:?}");
                assert!(stop.injections.contains(&Injection::Mouse {
                    point: current.point,
                    action: MouseAction::Down,
                    time: current.time,
                }));
            }
            assert!(s.shutdown(10).injections.is_empty(), "{later:?}");
        }
    }
}

#[test]
fn an_unaccepted_old_close_blocks_every_later_shutdown_down() {
    for later in LaterPress::ALL {
        let (mut s, in_flight, release) = pending_replay_with_release();
        later.enter(&mut s);
        assert!(
            s.reconcile_failed(
                in_flight.press_id,
                in_flight.logical_before,
                &in_flight.injections[..1],
                Some(release),
            )
            .is_some()
        );
        s.replay_close_completed(false);
        let stop = s.shutdown(9);
        assert!(stop.injections.is_empty(), "{later:?}");
        assert!(s.shutdown(10).injections.is_empty(), "{later:?}");
    }
}

#[test]
fn physical_up_during_old_close_never_queues_an_up_for_the_wrong_native_press() {
    for later in [
        LaterPress::Pending,
        LaterPress::Active,
        LaterPress::Disabled,
        LaterPress::EmptyMatcher,
    ] {
        for accepted_close in [false, true] {
            let (mut s, in_flight, release) = pending_replay_with_release();
            later.enter(&mut s);
            assert!(
                s.reconcile_failed(
                    in_flight.press_id,
                    in_flight.logical_before,
                    &in_flight.injections[..1],
                    Some(release),
                )
                .is_some()
            );
            let up = s.pointer(MouseAction::Up, pointer(71., 0, 25), 7);
            assert!(up.consume && up.logical_before, "{later:?}");
            assert!(up.injections.is_empty(), "{later:?}");
            assert!(!s.physical_down);
            // The old native Down stays owned by the pending close even though
            // this newer physical press has ended.
            assert!(s.control(Event::Snapshot, 8).logical_before);
            s.replay_close_completed(accepted_close);
            assert_eq!(
                s.control(Event::Snapshot, 9).logical_before,
                !accepted_close
            );
            assert!(s.shutdown(10).injections.is_empty(), "{later:?}");
        }
    }
}

#[test]
fn a_fresh_pass_through_down_during_old_close_waits_for_actual_close_acceptance() {
    for accepted_close in [false, true] {
        let (mut s, in_flight, release) = pending_replay_with_release();
        LaterPress::Pending.enter(&mut s);
        assert!(
            s.reconcile_failed(
                in_flight.press_id,
                in_flight.logical_before,
                &in_flight.injections[..1],
                Some(release),
            )
            .is_some()
        );
        let up2 = s.pointer(MouseAction::Up, pointer(51., 0, 25), 7);
        assert!(up2.consume && up2.injections.is_empty());

        s.policy.enabled = false;
        let press3 = pointer(80., 0, 30);
        let mut down3 = s.pointer(MouseAction::Down, press3, 8);
        assert!(!down3.consume);
        assert_ne!(down3.press_id, up2.press_id);
        s.defer_passthrough_down(&mut down3);
        assert!(down3.consume && down3.injections.is_empty());
        assert!(s.physical_down);
        s.replay_close_completed(accepted_close);
        let stop = s.shutdown(9);
        if accepted_close {
            assert_eq!(buttons(&stop), [MouseAction::Down]);
            assert!(stop.injections.contains(&Injection::Mouse {
                point: press3.point,
                action: MouseAction::Down,
                time: press3.time,
            }));
        } else {
            assert!(stop.injections.is_empty());
        }
        assert!(s.shutdown(10).injections.is_empty());
    }
}

#[test]
fn a_failed_replay_that_already_accepted_up_does_not_close_it_twice() {
    for accepted in 0..=1 {
        let mut s = started();
        s.pointer(MouseAction::Down, pointer(20., 0, 10), 1);
        let down = s.control(Event::Cancel, 2);
        assert_eq!(buttons(&down), [MouseAction::Down]);
        let in_flight = s.pointer(MouseAction::Up, pointer(23., 0, 12), 3);
        assert_eq!(buttons(&in_flight), [MouseAction::Up]);
        assert!(in_flight.logical_before);
        LaterPress::Pending.enter(&mut s);
        let close = s.reconcile_failed(
            in_flight.press_id,
            in_flight.logical_before,
            &in_flight.injections[..accepted],
            in_flight.release,
        );
        assert_eq!(close.is_some(), accepted == 0);
        if close.is_some() {
            s.replay_close_completed(true);
        }
        assert_eq!(buttons(&s.shutdown(9)), [MouseAction::Down]);
    }
}

#[test]
fn a_still_held_same_press_recovers_only_the_missing_native_down() {
    for accepted in 0..=2 {
        let mut s = started();
        s.pointer(MouseAction::Down, pointer(20., 0, 10), 1);
        s.pointer(MouseAction::Move, pointer(21., 0, 11), 2);
        let in_flight = s.control(Event::Cancel, 3);
        assert_eq!(
            s.reconcile_failed(
                in_flight.press_id,
                in_flight.logical_before,
                &in_flight.injections[..accepted],
                None,
            ),
            None
        );
        let stop = s.shutdown(4);
        if accepted == 0 {
            assert_eq!(buttons(&stop), [MouseAction::Down]);
        } else {
            assert!(stop.injections.is_empty());
        }
    }
}

#[test]
fn a_released_same_press_closes_only_an_accepted_native_down() {
    for accepted in 0..=2 {
        let (mut s, in_flight, release) = pending_replay_with_release();
        let close = s.reconcile_failed(
            in_flight.press_id,
            in_flight.logical_before,
            &in_flight.injections[..accepted],
            Some(release),
        );
        assert_eq!(close.is_some(), accepted > 0);
        if close.is_some() {
            s.replay_close_completed(true);
        }
        assert!(s.shutdown(5).injections.is_empty());
        assert!(s.shutdown(6).injections.is_empty());
    }
}

#[test]
fn a_buffered_pass_through_press_keeps_its_complete_native_tail_ordered() {
    for empty_matcher in [false, true] {
        let mut s = started();
        if empty_matcher {
            s.engine.use_matcher(&Matcher::default());
        } else {
            s.policy.enabled = false;
        }
        let mut down = s.pointer(MouseAction::Down, pointer(50., 0, 20), 1);
        assert!(!down.consume);
        s.defer_passthrough_down(&mut down);
        assert!(!down.logical_before);
        let moved = s.pointer(MouseAction::Move, pointer(51., 0, 21), 2);
        let up = s.pointer(MouseAction::Up, pointer(52., 0, 22), 3);
        assert!(moved.consume && up.consume);
        assert!(up.logical_before);
        assert_eq!(down.press_id, up.press_id);
        assert_eq!(up.release, Some(pointer(52., 0, 22)));
        assert_eq!(buttons(&down), [MouseAction::Down]);
        assert_eq!(buttons(&up), [MouseAction::Up]);
        assert!(s.shutdown(4).injections.is_empty());
    }
}

#[test]
fn cancelling_active_input_swallows_remaining_release_without_context_menu() {
    let mut s = started();
    s.pointer(MouseAction::Down, pointer(20., 0, 1), 1);
    s.pointer(MouseAction::Move, pointer(40., 0, 2), 2);
    assert!(s.control(Event::Cancel, 3).injections.is_empty());
    let up = s.pointer(MouseAction::Up, pointer(40., 0, 4), 4);
    assert!(up.consume);
    assert!(up.injections.is_empty());
}

#[test]
fn duplicate_pending_down_retains_original_context_and_recovers_one_stream() {
    let mut s = started();
    s.use_context(context(42, 101, Policy::default())).unwrap();
    s.pointer(MouseAction::Down, pointer(20., 0, 10), 1);
    let duplicate = s.pointer(MouseAction::Down, pointer(40., 0, 20), 2);
    assert!(duplicate.consume);
    assert_eq!(buttons(&duplicate), [MouseAction::Down]);
    assert_eq!(duplicate.epoch, Some(101));
    assert_eq!(duplicate.output.input_generation, 0);
    let report = duplicate.output.last_session.as_ref().unwrap();
    assert_eq!(report.generation, 1);
    assert_eq!(
        report.outcome,
        pointer_gesture_recognition::SessionOutcome::Cancelled
    );
    assert!(!report.captured);
    assert!(duplicate.injections.contains(&Injection::Mouse {
        point: pointer(20., 0, 0).point,
        action: MouseAction::Down,
        time: 10,
    }));
    assert!(
        s.pointer(MouseAction::Down, pointer(50., 0, 21), 3)
            .injections
            .is_empty()
    );
    let moved = s.pointer(MouseAction::Move, pointer(60., 0, 22), 4);
    assert!(moved.consume);
    assert!(buttons(&moved).is_empty());
    let up = s.pointer(MouseAction::Up, pointer(60., 0, 23), 5);
    assert!(up.consume);
    assert_eq!(buttons(&up), [MouseAction::Up]);
    assert!(
        !up.output
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::Match(_)))
    );
    assert_eq!(up.epoch, Some(101));

    s.use_context(context(84, 202, Policy::default())).unwrap();
    let next = s.pointer(MouseAction::Down, pointer(20., 0, 30), 6);
    assert!(next.consume && next.injections.is_empty());
    assert_eq!(next.output.input_generation, 2);
    assert_eq!(next.epoch, Some(202));
    s.pointer(MouseAction::Move, pointer(40., 0, 31), 7);
    let up = s.pointer(MouseAction::Up, pointer(40., 0, 32), 8);
    assert!(up.injections.is_empty());
    assert!(up.output.effects.contains(&Effect::Match(84)));
}

#[test]
fn duplicate_after_pending_cancellation_does_not_replay_or_match_again() {
    for (event, now) in [(Event::Cancel, 2), (Event::Tick, 110)] {
        let mut s = started();
        s.pointer(MouseAction::Down, pointer(20., 0, 10), 1);
        assert_eq!(buttons(&s.control(event, now)), [MouseAction::Down]);
        let duplicate = s.pointer(MouseAction::Down, pointer(20., 0, 20), now + 1);
        assert!(duplicate.consume);
        assert!(duplicate.injections.is_empty());
        assert_eq!(duplicate.output.input_generation, 0);
        s.pointer(MouseAction::Move, pointer(40., 0, 21), now + 2);
        let up = s.pointer(MouseAction::Up, pointer(40., 0, 22), now + 3);
        assert_eq!(buttons(&up), [MouseAction::Up]);
        assert!(
            !up.output
                .effects
                .iter()
                .any(|effect| matches!(effect, Effect::Match(_)))
        );
    }
}

#[test]
fn duplicate_active_down_never_leaks_buttons_or_matches_on_release() {
    let mut s = started();
    s.pointer(MouseAction::Down, pointer(20., 0, 10), 1);
    s.pointer(MouseAction::Move, pointer(40., 0, 11), 2);
    let duplicate = s.pointer(MouseAction::Down, pointer(50., 0, 20), 3);
    assert!(duplicate.consume && duplicate.injections.is_empty());
    assert_eq!(duplicate.output.input_generation, 0);
    let report = duplicate.output.last_session.as_ref().unwrap();
    assert_eq!(report.generation, 1);
    assert!(report.captured);
    assert_eq!(
        report.outcome,
        pointer_gesture_recognition::SessionOutcome::Cancelled
    );
    assert!(
        s.pointer(MouseAction::Down, pointer(60., 0, 21), 4)
            .injections
            .is_empty()
    );
    let up = s.pointer(MouseAction::Up, pointer(60., 0, 22), 5);
    assert!(up.consume && up.injections.is_empty());
    assert!(
        !up.output
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::Match(_)))
    );
    assert!(s.shutdown(6).injections.is_empty());
}

#[test]
fn duplicate_pass_through_down_keeps_the_original_native_pair_balanced() {
    for empty_matcher in [false, true] {
        let mut s = started();
        if empty_matcher {
            s.engine.use_matcher(&Matcher::default());
        } else {
            s.policy.enabled = false;
        }
        let down = s.pointer(MouseAction::Down, pointer(20., 0, 10), 1);
        assert!(!down.consume && down.injections.is_empty());
        let duplicate = s.pointer(MouseAction::Down, pointer(20., 0, 20), 2);
        assert!(duplicate.consume && duplicate.injections.is_empty());
        assert_eq!(duplicate.output.input_generation, 0);
        let up = s.pointer(MouseAction::Up, pointer(20., 0, 21), 3);
        assert!(!up.consume && up.injections.is_empty());
        assert!(
            !up.output
                .effects
                .iter()
                .any(|effect| matches!(effect, Effect::Match(_)))
        );
        assert!(s.shutdown(4).injections.is_empty());
    }
}

#[test]
fn duplicate_pending_recovery_reconciles_each_partial_delivery_before_stop() {
    // Duplicate-down cancellation queues one original down and a move. A failed
    // native batch must reconcile its accepted prefix before shutdown recovery.
    for accepted in 0..=2 {
        let mut s = started();
        s.use_context(context(42, 101, Policy::default())).unwrap();
        s.pointer(MouseAction::Down, pointer(20., 0, 10), 1);
        let duplicate = s.pointer(MouseAction::Down, pointer(40., 0, 20), 2);
        assert_eq!(duplicate.injections.len(), 2);
        s.injection_failed(duplicate.logical_before, &duplicate.injections[..accepted]);
        let stop = s.shutdown(3);
        let delivered_downs = duplicate.injections[..accepted]
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    Injection::Mouse {
                        action: MouseAction::Down,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(
            delivered_downs
                + buttons(&stop)
                    .iter()
                    .filter(|action| **action == MouseAction::Down)
                    .count(),
            1,
        );
        assert!(!buttons(&stop).contains(&MouseAction::Up));
        assert_eq!(stop.epoch, Some(101));
        if accepted == 0 {
            assert!(stop.injections.contains(&Injection::Mouse {
                point: pointer(20., 0, 0).point,
                action: MouseAction::Down,
                time: 10,
            }));
        } else {
            assert!(stop.injections.is_empty());
        }
        // Hooks are gone. The remaining real up balances that one native down.
        assert!(s.shutdown(4).injections.is_empty());
    }
}

#[test]
fn duplicate_release_partial_failure_balances_the_forwarded_down_once() {
    for accepted in 0..=1 {
        let mut s = started();
        s.pointer(MouseAction::Down, pointer(20., 0, 10), 1);
        let duplicate = s.pointer(MouseAction::Down, pointer(40., 0, 20), 2);
        assert_eq!(buttons(&duplicate), [MouseAction::Down]);
        let up = s.pointer(MouseAction::Up, pointer(40., 0, 21), 3);
        assert_eq!(buttons(&up), [MouseAction::Up]);
        s.injection_failed(up.logical_before, &up.injections[..accepted]);
        let stop = s.shutdown(4);
        assert_eq!(
            buttons(&stop),
            if accepted == 0 {
                vec![MouseAction::Up]
            } else {
                vec![]
            }
        );
        assert!(s.shutdown(5).injections.is_empty());
    }
}

#[test]
fn stopping_duplicate_active_or_pass_through_input_preserves_one_physical_lifetime() {
    for active in [false, true] {
        let mut s = started();
        s.use_context(context(42, 101, Policy::default())).unwrap();
        if !active {
            s.policy.enabled = false;
        }
        s.pointer(MouseAction::Down, pointer(20., 0, 10), 1);
        if active {
            s.pointer(MouseAction::Move, pointer(40., 0, 11), 2);
        }
        let duplicate = s.pointer(MouseAction::Down, pointer(50., 0, 20), 3);
        assert!(duplicate.consume && duplicate.injections.is_empty());
        let stop = s.shutdown(4);
        assert_eq!(stop.epoch, Some(101));
        if active {
            // Teardown restores the original hidden held press, not the
            // duplicate. The real up passes after hook removal to balance it.
            assert_eq!(buttons(&stop), [MouseAction::Down]);
            assert!(stop.injections.contains(&Injection::Mouse {
                point: pointer(20., 0, 0).point,
                action: MouseAction::Down,
                time: 10,
            }));
        } else {
            // The original down already passed through; do not inject another.
            assert!(stop.injections.is_empty());
        }
        assert!(s.shutdown(5).injections.is_empty());
    }
}

#[test]
fn stopping_active_input_restores_the_original_held_drag() {
    let mut s = started();
    s.pointer(MouseAction::Down, pointer(20., 0, 1), 1);
    s.pointer(MouseAction::Move, pointer(40., 0, 2), 2);
    let stop = s.shutdown(3);
    assert_eq!(buttons(&stop), [MouseAction::Down]);
    assert_eq!(
        stop.injections.last(),
        Some(&Injection::Mouse {
            point: pointer(40., 0, 2).point,
            action: MouseAction::Move,
            time: 2
        })
    );
    assert!(s.shutdown(4).injections.is_empty());
}

#[test]
fn expiration_on_up_does_not_leak_a_second_real_release() {
    let mut s = started();
    s.pointer(MouseAction::Down, pointer(20., 0, 1), 1);
    let up = s.pointer(MouseAction::Up, pointer(20., 0, 110), 110);
    assert!(up.consume);
    assert_eq!(buttons(&up), [MouseAction::Down, MouseAction::Up]);
}

#[test]
fn expiration_while_held_restores_pending_input_and_finishes_with_real_up() {
    let mut s = started();
    s.pointer(MouseAction::Down, pointer(20., 0, 1), 1);
    assert_eq!(buttons(&s.control(Event::Tick, 110)), [MouseAction::Down]);
    let up = s.pointer(MouseAction::Up, pointer(20., 0, 111), 111);
    assert!(up.consume);
    assert_eq!(buttons(&up), [MouseAction::Up]);
}

#[test]
fn original_press_modifiers_and_time_are_restored_then_latest_modifiers_return() {
    let mut s = started();
    s.pointer(MouseAction::Down, pointer(20., 4, 10), 1);
    let up = s.pointer(MouseAction::Up, pointer(21., 1, 30), 2);
    assert_eq!(
        &up.injections[..3],
        &[
            Injection::Key {
                index: 0,
                down: false
            },
            Injection::Key {
                index: 2,
                down: true
            },
            Injection::Mouse {
                point: pointer(20., 0, 0).point,
                action: MouseAction::Down,
                time: 10
            }
        ]
    );
    assert_eq!(
        &up.injections[3..5],
        &[
            Injection::Key {
                index: 2,
                down: false
            },
            Injection::Key {
                index: 0,
                down: true
            }
        ]
    );
    assert!(up.injections.contains(&Injection::Mouse {
        point: pointer(21., 0, 0).point,
        action: MouseAction::Move,
        time: 30
    }));
}

#[test]
fn required_modifiers_accept_either_side_and_pass_other_presses_unchanged() {
    let mut s = started();
    s.policy = Policy {
        enabled: true,
        required_modifiers: 2,
    };
    assert!(!s.pointer(MouseAction::Down, pointer(20., 0, 1), 1).consume);
    assert!(!s.pointer(MouseAction::Up, pointer(20., 0, 2), 2).consume);
    assert!(s.pointer(MouseAction::Down, pointer(20., 8, 3), 3).consume);
    assert_eq!(modifier_groups(0b10101010), 15);
}

#[test]
fn partial_delivery_tracks_only_accepted_down_for_following_recovery() {
    let mut s = started();
    s.pointer(MouseAction::Down, pointer(20., 0, 1), 1);
    let delivery = s.control(Event::Cancel, 2);
    s.injection_failed(delivery.logical_before, &[]);
    assert_eq!(buttons(&s.shutdown(3)), [MouseAction::Down]);
}

#[test]
fn partial_complete_click_releases_an_accepted_down_during_shutdown() {
    let mut s = started();
    s.pointer(MouseAction::Down, pointer(20., 0, 1), 1);
    let up = s.pointer(MouseAction::Up, pointer(20., 0, 2), 2);
    s.injection_failed(up.logical_before, &up.injections[..1]);
    assert_eq!(buttons(&s.shutdown(3)), [MouseAction::Up]);
    assert!(s.shutdown(4).injections.is_empty());
}

#[test]
fn virtual_desktop_projection_handles_negative_origins_and_edge_pixels() {
    assert_eq!(absolute(-1920., -1920, 3840), 0);
    assert_eq!(absolute(1919., -1920, 3840), 65535);
    assert_eq!(absolute(-3000., -1920, 3840), 0);
    assert_eq!(absolute(3000., -1920, 3840), 65535);
    assert_eq!(absolute(30., 0, 1), 0);
}

#[test]
fn invalid_native_button_and_unbounded_callback_work_are_rejected() {
    let matcher = Matcher::default();
    let mut config = Configuration {
        start_distance: 10.,
        change_distance: 5.,
        total_duration_ns: 0,
        idle_duration_ns: 0,
        maximum_points: 4097,
        maximum_directions: 64,
        button: 1,
        passes_through_empty_matcher: true,
    };
    assert!(matches!(
        Session::new(config, &matcher),
        Err(Error::InvalidConfiguration)
    ));
    config.maximum_points = 256;
    config.button = 5;
    assert!(matches!(
        Session::new(config, &matcher),
        Err(Error::InvalidConfiguration)
    ));
}
