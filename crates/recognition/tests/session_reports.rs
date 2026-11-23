use pointer_gesture_recognition::*;

fn config() -> Configuration {
    Configuration {
        start_distance: 10.,
        change_distance: 5.,
        total_duration_ns: 0,
        idle_duration_ns: 0,
        maximum_points: 4,
        maximum_directions: 64,
        button: 1,
        passes_through_empty_matcher: true,
    }
}
fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}
fn engine(configuration: Configuration, recording: bool, pattern: &[Direction]) -> Engine {
    let mut engine = Engine::new(configuration).unwrap();
    let mut matcher = Matcher::default();
    if !pattern.is_empty() {
        matcher.register(pattern, 0).unwrap();
    }
    engine.use_matcher(&matcher);
    engine.step(Event::Start, 0);
    engine.step(Event::Recording(recording), 0);
    engine
}
fn down(e: &mut Engine) {
    e.step(
        Event::Down {
            button: 1,
            point: point(0., 0.),
        },
        1,
    );
}
fn moved(e: &mut Engine, x: f64, y: f64, now: u64) -> Output {
    e.step(
        Event::Move {
            button: 1,
            point: point(x, y),
        },
        now,
    )
}
fn up(e: &mut Engine, x: f64, y: f64, now: u64) -> Output {
    e.step(
        Event::Up {
            button: 1,
            point: point(x, y),
        },
        now,
    )
}
#[test]
fn press_admission_applies_host_policy_and_clears_stale_failure() {
    let mut configuration = config();
    configuration.total_duration_ns = 20;
    for (enabled, modifiers, recording, failure) in [
        (false, false, false, Some(Failure::Disabled)),
        (false, true, false, Some(Failure::Disabled)),
        (false, false, true, Some(Failure::Disabled)),
        (false, true, true, Some(Failure::Disabled)),
        (true, false, false, Some(Failure::ModifiersNotSatisfied)),
        (true, true, false, None),
        (true, false, true, None),
        (true, true, true, None),
    ] {
        let mut engine = engine(configuration, false, &[Direction::Right]);
        engine.step(
            Event::Policy {
                enabled: true,
                modifiers_satisfied: false,
            },
            0,
        );
        let press = Event::Down {
            button: 1,
            point: point(0., 0.),
        };
        assert_eq!(
            engine.step(press, 1).status.failure,
            Some(Failure::ModifiersNotSatisfied)
        );
        engine.step(Event::Recording(recording), 1);
        engine.step(
            Event::Policy {
                enabled,
                modifiers_satisfied: modifiers,
            },
            1,
        );
        let output = engine.step(press, 2);

        let admitted = failure.is_none();
        let (disposition, stage) = if admitted {
            (Disposition::Consume, InputStage::Pending)
        } else {
            (Disposition::PassThrough, InputStage::Idle)
        };
        assert_eq!((output.disposition, output.stage), (disposition, stage));
        assert!(output.status.ready);
        assert!(!output.status.capturing);
        assert_eq!(output.status.recording, recording);
        assert_eq!(output.status.failure, failure);
        assert_eq!(output.input_generation, u64::from(admitted));
        assert_eq!(output.deadline_ns, admitted.then_some(22));
        assert!(output.effects.is_empty());
        assert!(output.last_session.is_none());
    }
}

#[test]
fn a_denied_replacement_press_recovers_the_captured_input_once() {
    for staged in [false, true] {
        let mut engine = engine(config(), false, &[Direction::Right]);
        down(&mut engine);
        assert_eq!(moved(&mut engine, 20., 0., 2).stage, InputStage::Active);
        let prepared = staged.then(|| engine.prepare_replacement_press(3));
        engine.step(
            Event::Policy {
                enabled: true,
                modifiers_satisfied: false,
            },
            3,
        );
        let rejected = engine.step(
            Event::Down {
                button: 1,
                point: point(40., 0.),
            },
            3,
        );
        let recovery = prepared.as_ref().unwrap_or(&rejected);
        assert!(matches!(
            recovery.effects.as_slice(),
            [Effect::Release { button: 1, point: release }, Effect::Publish { trace, .. }]
                if *release == point(20., 0.) && !trace.visible
        ));
        let report = recovery.last_session.as_ref().unwrap();
        assert_eq!(report.outcome, SessionOutcome::Superseded);
        assert!(report.captured);
        assert_eq!(report.directions.as_ref(), &[Direction::Right]);
        assert_eq!(rejected.disposition, Disposition::PassThrough);
        assert_eq!(rejected.stage, InputStage::Idle);
        assert_eq!(
            rejected.status.failure,
            Some(Failure::ModifiersNotSatisfied)
        );
        assert_eq!(rejected.input_generation, 0);
        assert_eq!(rejected.deadline_ns, None);
        if staged {
            assert!(rejected.effects.is_empty());
        }
        let released = up(&mut engine, 50., 0., 4);
        assert_eq!(released.disposition, Disposition::PassThrough);
        assert!(released.effects.is_empty());
        assert_eq!(released.last_session.as_ref(), Some(report));
    }
}

#[test]
fn exact_zero_id_match_has_an_owned_coordinate_free_report() {
    let mut e = engine(config(), false, &[Direction::Right]);
    down(&mut e);
    moved(&mut e, 20., 0., 2);
    let output = up(&mut e, 30., 0., 5);
    let report = output.last_session.unwrap();
    assert_eq!(report.outcome, SessionOutcome::Matched);
    assert_eq!(report.elapsed_ns, 4);
    assert_eq!(report.point_count, 3);
    assert_eq!(report.maximum_axis_distance, 30.);
    assert_eq!(report.directions.as_ref(), &[Direction::Right]);
    assert!(report.captured);
    assert!(output.effects.contains(&Effect::Match(0)));
    assert_eq!(
        e.step(Event::Snapshot, 20).last_session.as_ref(),
        Some(&report)
    );
    e.close(30);
    assert_eq!(report.directions.as_ref(), &[Direction::Right]);
}
#[test]
fn normal_click_and_stationary_samples_are_not_reported_as_errors() {
    let mut e = engine(config(), true, &[]);
    down(&mut e);
    moved(&mut e, 0., 0., 2);
    moved(&mut e, 4., 2., 3);
    let output = up(&mut e, 4., 2., 5);
    let report = output.last_session.unwrap();
    assert_eq!(report.outcome, SessionOutcome::BelowThreshold);
    assert_eq!(report.point_count, 4);
    assert_eq!(report.idle_ns, 2);
    assert!(!report.captured);
    assert!(report.directions.is_empty());
    assert!(
        output
            .effects
            .iter()
            .any(|e| matches!(e, Effect::Click { .. }))
    );
}

#[test]
fn stationary_threshold_promotion_does_not_restart_the_idle_clock_again() {
    let mut configuration = config();
    configuration.start_distance = 100.;
    configuration.idle_duration_ns = 100;
    let mut e = engine(configuration, true, &[]);
    down(&mut e);
    moved(&mut e, 20., 0., 2);
    configuration.start_distance = 10.;
    assert!(e.update_tuning(configuration, 20, false).unwrap());
    let promoted = moved(&mut e, 20., 0., 30);
    assert_eq!(promoted.stage, InputStage::Active);
    assert_eq!(promoted.deadline_ns, Some(120));
    let report = e.step(Event::Tick, 120).last_session.unwrap();
    assert_eq!(report.outcome, SessionOutcome::IdleDeadline);
    assert_eq!(report.idle_ns, 118);
    assert_eq!(report.point_count, 3);
}
#[test]
fn valid_prefix_and_invalid_prefix_have_distinct_recovery_reports() {
    let mut e = engine(config(), false, &[Direction::Right, Direction::Down]);
    down(&mut e);
    moved(&mut e, 20., 0., 2);
    let out = up(&mut e, 25., 0., 3);
    assert_eq!(out.last_session.unwrap().outcome, SessionOutcome::Unmatched);
    let mut e = engine(config(), false, &[Direction::Down]);
    down(&mut e);
    let out = moved(&mut e, 20., 0., 2);
    let report = out.last_session.unwrap();
    assert_eq!(report.outcome, SessionOutcome::UnmatchedPrefix);
    assert_eq!(report.rejected_direction, Some(Direction::Right));
    assert!(
        out.effects
            .iter()
            .any(|e| matches!(e, Effect::DragStart { .. }))
    );
    assert_eq!(up(&mut e, 20., 0., 3).disposition, Disposition::PassThrough);
}
#[test]
fn direction_budget_recovers_held_input_before_vectors_can_grow() {
    let mut c = config();
    c.maximum_directions = 2;
    let mut e = engine(c, true, &[]);
    down(&mut e);
    moved(&mut e, 20., 0., 2);
    moved(&mut e, 20., 20., 3);
    let out = moved(&mut e, 0., 20., 4);
    let report = out.last_session.unwrap();
    assert_eq!(report.outcome, SessionOutcome::DirectionLimit);
    assert_eq!(
        report.directions.as_ref(),
        &[Direction::Right, Direction::Down]
    );
    assert_eq!(report.rejected_direction, Some(Direction::Left));
    assert_eq!(report.point_count, 4);
    assert_eq!(out.stage, InputStage::Idle);
    assert!(out.discard_pending_visible_trace);
    assert!(
        out.effects
            .iter()
            .any(|e| matches!(e, Effect::DragStart { .. }))
    );
    assert_eq!(up(&mut e, 0., 20., 5).disposition, Disposition::PassThrough);
}
#[test]
fn direction_budget_is_captured_and_deadline_resets_do_not_rewrite_measurements() {
    let mut c = config();
    c.maximum_directions = 2;
    let mut e = engine(c, true, &[]);
    down(&mut e);
    moved(&mut e, 20., 0., 2);
    c.maximum_directions = 1;
    assert!(e.update_tuning(c, 10, true).unwrap());
    let report = up(&mut e, 20., 20., 20).last_session.unwrap();
    assert_eq!(report.outcome, SessionOutcome::Recorded);
    assert_eq!(report.directions.len(), 2);
    assert_eq!(report.elapsed_ns, 19);
    let mut e = engine(config(), true, &[]);
    down(&mut e);
    moved(&mut e, 20., 0., 2);
    e.update_tuning(config(), 10, true).unwrap();
    let report = up(&mut e, 20., 0., 20).last_session.unwrap();
    assert_eq!(report.elapsed_ns, 19);
    assert_eq!(report.idle_ns, 18);
}
#[test]
fn earliest_deadline_is_reported_with_total_winning_ties() {
    for (total, idle, outcome) in [
        (10, 5, SessionOutcome::IdleDeadline),
        (5, 10, SessionOutcome::TotalDeadline),
        (5, 5, SessionOutcome::TotalDeadline),
    ] {
        let mut c = config();
        c.total_duration_ns = total;
        c.idle_duration_ns = idle;
        let mut e = engine(c, true, &[]);
        down(&mut e);
        let report = e.step(Event::Tick, 20).last_session.unwrap();
        assert_eq!(report.outcome, outcome);
        assert_eq!(report.elapsed_ns, 19);
        assert_eq!(report.idle_ns, 19);
    }
}

#[test]
fn promotion_preserves_the_original_total_deadline() {
    let mut configuration = config();
    configuration.total_duration_ns = 100;
    configuration.idle_duration_ns = 75;
    let mut e = engine(configuration, true, &[]);
    down(&mut e);
    let promoted = moved(&mut e, 20., 0., 51);
    assert_eq!(promoted.stage, InputStage::Active);
    assert_eq!(promoted.deadline_ns, Some(101));
}

#[test]
fn only_changed_locations_refresh_pending_and_active_idle_deadlines() {
    let mut configuration = config();
    configuration.idle_duration_ns = 100;
    let mut e = engine(configuration, true, &[]);
    down(&mut e);
    for (time, x, stage, deadline) in [
        (11, 0., InputStage::Pending, 101),
        (21, 1., InputStage::Pending, 121),
        (31, 20., InputStage::Active, 131),
        (41, 20., InputStage::Active, 131),
        (51, 21., InputStage::Active, 151),
    ] {
        let output = moved(&mut e, x, 0., time);
        assert_eq!(output.stage, stage);
        assert_eq!(output.deadline_ns, Some(deadline), "at {time}");
    }
}

#[test]
fn capped_pending_drag_replays_the_press_and_most_recent_points() {
    let mut configuration = config();
    configuration.start_distance = 50.;
    configuration.maximum_points = 3;
    let mut e = engine(configuration, false, &[Direction::Right]);
    down(&mut e);
    for (time, coordinate) in [(2, 20.), (3, 40.), (4, 60.), (5, 80.)] {
        let output = moved(&mut e, coordinate, coordinate, time);
        assert_eq!(output.disposition, Disposition::Consume);
        assert_eq!(output.stage, InputStage::Pending);
    }
    let output = up(&mut e, 90., 90., 6);
    assert_eq!(output.disposition, Disposition::Consume);
    assert!(matches!(
        output.effects.as_slice(),
        [Effect::Drag { button: 1, points }]
            if points.as_ref() == [point(0., 0.), point(80., 80.), point(90., 90.)]
    ));
}

#[test]
fn cancellation_stop_replacement_and_invalid_release_keep_their_actual_reason() {
    for (event, outcome) in [
        (Event::Cancel, SessionOutcome::Cancelled),
        (Event::Stop, SessionOutcome::Stopped),
        (
            Event::Down {
                button: 1,
                point: point(10., 0.),
            },
            SessionOutcome::Superseded,
        ),
        (
            Event::Up {
                button: 1,
                point: point(f64::NAN, 0.),
            },
            SessionOutcome::InvalidRelease,
        ),
    ] {
        let mut e = engine(config(), true, &[]);
        down(&mut e);
        moved(&mut e, 20., 0., 2);
        let out = e.step(event, 3);
        let report = out.last_session.unwrap();
        assert_eq!(report.outcome, outcome);
        assert_eq!(report.point_count, 2);
        assert_eq!(report.directions.as_ref(), &[Direction::Right]);
        assert!(!out.effects.iter().any(|e| matches!(e, Effect::Match(_))));
    }
}

#[test]
fn prepared_replacement_recovers_the_same_old_input_before_admitting_the_new_press() {
    let mut configuration = config();
    configuration.total_duration_ns = 10;
    let replacement = Event::Down {
        button: 1,
        point: point(0., 20.),
    };
    for (when, outcome) in [
        (3, SessionOutcome::Superseded),
        (11, SessionOutcome::TotalDeadline),
    ] {
        for captured in [false, true] {
            let mut direct = engine(configuration, true, &[]);
            let mut split = engine(configuration, true, &[]);
            for engine in [&mut direct, &mut split] {
                down(engine);
                if captured {
                    moved(engine, 20., 0., 2);
                }
            }

            let prepared = split.prepare_replacement_press(when);
            let combined = direct.step(replacement, when);
            assert_eq!(prepared.disposition, Disposition::Control);
            assert_eq!(prepared.stage, InputStage::Idle);
            assert_eq!(prepared.input_generation, 0);
            assert_eq!(prepared.deadline_ns, None);
            assert!(prepared.discard_pending_visible_trace);
            assert_eq!(prepared.effects, combined.effects);
            assert_eq!(prepared.last_session, combined.last_session);
            assert_eq!(prepared.last_session.as_ref().unwrap().outcome, outcome);
            assert_eq!(combined.stage, InputStage::Pending);

            let admitted = split.step(replacement, when);
            assert_eq!(admitted.stage, InputStage::Pending);
            assert_eq!(admitted.input_generation, combined.input_generation);
            assert_eq!(admitted.deadline_ns, combined.deadline_ns);
            assert_eq!(admitted.last_session, prepared.last_session);
            assert!(admitted.effects.is_empty());
        }
    }
}

#[test]
fn invalid_budgets_fail_construction_and_extreme_finite_movement_stays_finite() {
    for limit in [0, 4097, usize::MAX] {
        let mut c = config();
        c.maximum_directions = limit;
        assert!(Engine::new(c).is_err());
    }
    let mut e = engine(config(), true, &[]);
    e.step(
        Event::Down {
            button: 1,
            point: point(-f64::MAX, 0.),
        },
        1,
    );
    let report = up(&mut e, f64::MAX, 0., 2).last_session.unwrap();
    assert_eq!(report.maximum_axis_distance, f64::MAX);
    assert!(report.maximum_axis_distance.is_finite());
}
