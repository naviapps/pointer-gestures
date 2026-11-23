use pointer_gesture_recognition::*;

fn configuration() -> Configuration {
    Configuration {
        start_distance: 10.0,
        change_distance: 5.0,
        total_duration_ns: 1000,
        idle_duration_ns: 100,
        maximum_points: 8,
        maximum_directions: 64,
        button: 1,
        passes_through_empty_matcher: true,
    }
}

fn down(x: f64) -> Event {
    Event::Down {
        button: 1,
        point: Point { x, y: 0.0 },
    }
}
fn moved(x: f64) -> Event {
    Event::Move {
        button: 1,
        point: Point { x, y: 0.0 },
    }
}

#[test]
fn empty_matcher_admission_respects_policy_and_keeps_recording_available() {
    let origin = Point { x: 0.0, y: 0.0 };
    let end = Point { x: 40.0, y: 0.0 };
    for passes_through in [false, true] {
        let mut config = configuration();
        config.passes_through_empty_matcher = passes_through;
        let mut engine = Engine::new(config).unwrap();
        engine.step(Event::Start, 0);

        let pressed = engine.step(down(origin.x), 1);
        let released = engine.step(
            Event::Up {
                button: 1,
                point: origin,
            },
            2,
        );
        if passes_through {
            for output in [pressed, released] {
                assert_eq!(output.disposition, Disposition::PassThrough);
                assert_eq!(output.stage, InputStage::Idle);
                assert!(output.effects.is_empty());
                assert!(output.last_session.is_none());
            }
        } else {
            assert_eq!(pressed.disposition, Disposition::Consume);
            assert_eq!(pressed.stage, InputStage::Pending);
            assert_eq!(released.disposition, Disposition::Consume);
            assert!(matches!(
                released.effects.as_slice(),
                [Effect::Click { button: 1, point }] if *point == origin
            ));
            assert_eq!(
                released.last_session.unwrap().outcome,
                SessionOutcome::BelowThreshold
            );
        }

        engine.step(Event::Recording(true), 3);
        assert_eq!(
            engine.step(down(origin.x), 4).disposition,
            Disposition::Consume
        );
        let active = engine.step(moved(end.x), 5);
        assert_eq!(active.stage, InputStage::Active);
        assert_eq!(active.trace.directions.as_ref(), [Direction::Right]);
        let recorded = engine.step(
            Event::Up {
                button: 1,
                point: end,
            },
            6,
        );
        assert_eq!(recorded.disposition, Disposition::Consume);
        assert_eq!(
            recorded.last_session.unwrap().outcome,
            SessionOutcome::Recorded
        );
        assert!(recorded.effects.iter().any(
            |effect| matches!(effect, Effect::Drag { button: 1, points } if points.as_ref() == [origin, end])
        ));
    }
}

#[test]
fn matcher_snapshot_and_zero_identifier_survive_replacement() {
    let mut matcher = Matcher::default();
    assert!(matcher.register(&[], 42).is_err());
    assert!(matcher.register(&[Direction::Right], 0).is_ok());
    let snapshot = matcher.clone();
    matcher.register(&[Direction::Right], u64::MAX).unwrap();
    assert_eq!(snapshot.match_pattern(&[Direction::Right]), Some(0));
    assert_eq!(matcher.match_pattern(&[Direction::Right]), Some(u64::MAX));
    assert_eq!(
        matcher.match_pattern(&[Direction::Right, Direction::Down]),
        None
    );

    let mut engine = Engine::new(configuration()).unwrap();
    engine.use_matcher(&snapshot);
    engine.step(Event::Start, 0);
    engine.step(down(0.0), 0);
    engine.use_matcher(&matcher);
    let active = engine.step(moved(20.0), 10);
    assert!(active.trace.visible);
    let complete = engine.step(
        Event::Up {
            button: 1,
            point: Point { x: 30.0, y: 0.0 },
        },
        20,
    );
    assert!(matches!(complete.effects.last(), Some(Effect::Match(0))));
    assert!(matches!(
        complete.effects[complete.effects.len() - 2],
        Effect::Release { button: 1, .. }
    ));
    assert_eq!(complete.stage, InputStage::Idle);
    assert!(!complete.trace.visible);
}

#[test]
fn expiration_and_close_return_owned_recovery_effects() {
    let mut engine = Engine::new(configuration()).unwrap();
    engine.step(Event::Start, 0);
    engine.step(Event::Recording(true), 0);
    engine.step(down(0.0), 10);
    let active = engine.step(moved(20.0), 20);
    assert_eq!(active.deadline_ns, Some(120));
    let expired = engine.step(Event::Tick, 120);
    assert_eq!(expired.status.failure, Some(Failure::Expired));
    assert!(matches!(
        expired.effects.first(),
        Some(Effect::Release { button: 1, .. })
    ));
    assert_eq!(
        active.trace.endpoints.last(),
        Some(&Point { x: 20.0, y: 0.0 })
    );
    engine.step(down(40.0), 130);
    let closed = engine.close(140);
    assert!(matches!(
        closed.effects.first(),
        Some(Effect::Click {
            button: 1,
            point: Point { x: 40.0, .. }
        })
    ));
    assert!(!closed.status.ready);
}

#[test]
fn invalid_input_and_tuning_do_not_capture_or_corrupt_deadlines() {
    let mut config = configuration();
    config.start_distance = f64::NAN;
    assert!(Engine::new(config).is_err());
    config = configuration();
    let mut engine = Engine::new(config).unwrap();
    engine.step(Event::Start, 0);
    assert_eq!(
        engine.step(down(f64::INFINITY), 0).disposition,
        Disposition::PassThrough
    );
    engine.step(Event::Recording(true), 0);
    engine.step(down(0.0), 10);
    config.idle_duration_ns = 200;
    assert_eq!(engine.update_tuning(config, 20, false), Ok(true));
    assert_eq!(engine.step(Event::Tick, 30).deadline_ns, Some(220));
    config.button = 0;
    assert!(engine.update_tuning(config, 40, false).is_err());
    assert_eq!(engine.step(Event::Tick, 50).deadline_ns, Some(220));
    assert_eq!(
        resolve_movement(Point { x: 0.0, y: 0.0 }, Point { x: 10.0, y: 10.0 })
            .unwrap()
            .direction,
        None
    );
    assert!(
        resolve_movement(
            Point {
                x: f64::NAN,
                y: 0.0
            },
            Point { x: 10.0, y: 0.0 }
        )
        .is_none()
    );
}

#[test]
fn overflowing_finite_axis_deltas_still_resolve_the_dominant_direction() {
    let max = f64::MAX;
    let point = |x, y| Point { x, y };
    let almost_max = max.next_down();
    for (start, end, expected) in [
        (
            point(-max, -max),
            point(max, max / 2.0),
            Some(Direction::Right),
        ),
        (
            point(max, max),
            point(-max, -max / 2.0),
            Some(Direction::Left),
        ),
        (
            point(-max, -max),
            point(max / 2.0, max),
            Some(Direction::Down),
        ),
        (
            point(max, max),
            point(-max / 2.0, -max),
            Some(Direction::Up),
        ),
        (point(-max, max), point(max, -max), None),
        // Both half-scale deltas also round to the same value.
        (
            point(-almost_max, -almost_max),
            point(almost_max, almost_max.next_down()),
            Some(Direction::Right),
        ),
        (point(-max, 0.0), point(max, max), Some(Direction::Right)),
    ] {
        assert_eq!(
            resolve_movement(start, end),
            Some(Movement {
                direction: expected,
                maximum_axis_delta: f64::INFINITY,
            }),
        );
    }
}

#[test]
fn overflowing_finite_motion_captures_matches_and_releases_the_held_button() {
    let max = f64::MAX;
    let start = Point { x: -max, y: -max };
    let end = Point {
        x: max,
        y: max / 2.0,
    };
    let mut matcher = Matcher::default();
    matcher.register(&[Direction::Right], 42).unwrap();
    let mut engine = Engine::new(configuration()).unwrap();
    engine.use_matcher(&matcher);
    engine.step(Event::Start, 0);
    engine.step(
        Event::Down {
            button: 1,
            point: start,
        },
        1,
    );
    let active = engine.step(
        Event::Move {
            button: 1,
            point: end,
        },
        2,
    );
    assert_eq!(active.stage, InputStage::Active);
    assert_eq!(active.trace.directions.as_ref(), [Direction::Right]);
    assert_eq!(active.trace.endpoints.as_ref(), [start, end]);

    let finished = engine.step(
        Event::Up {
            button: 1,
            point: end,
        },
        3,
    );
    assert!(matches!(
        finished.effects.as_slice(),
        [.., Effect::Release { button: 1, point }, Effect::Match(42)] if *point == end
    ));
    let report = finished.last_session.unwrap();
    assert_eq!(report.outcome, SessionOutcome::Matched);
    assert_eq!(report.maximum_axis_distance, f64::MAX);
    assert!(!finished.trace.visible);
}

#[test]
fn published_snapshots_share_owned_buffers_and_invalid_release_clears_recording() {
    use std::sync::Arc;
    let mut matcher = Matcher::default();
    matcher.register(&[Direction::Right], 1).unwrap();
    let mut engine = Engine::new(configuration()).unwrap();
    engine.use_matcher(&matcher);
    engine.step(Event::Start, 0);
    engine.step(down(0.0), 0);
    let first = engine.step(moved(20.0), 10);
    let published = first
        .effects
        .iter()
        .find_map(|e| {
            if let Effect::Publish { trace, .. } = e {
                Some(trace)
            } else {
                None
            }
        })
        .unwrap();
    assert!(Arc::ptr_eq(&first.trace.endpoints, &published.endpoints));
    assert!(Arc::ptr_eq(&first.trace.raw_points, &published.raw_points));
    let saved = first.trace.clone();
    assert!(Arc::ptr_eq(&saved.endpoints, &first.trace.endpoints));
    engine.step(
        Event::Up {
            button: 1,
            point: Point { x: 30.0, y: 0.0 },
        },
        20,
    );
    engine.step(down(0.0), 30);
    engine.step(moved(20.0), 40);
    let invalid = engine.step(
        Event::Up {
            button: 1,
            point: Point { x: 20.0, y: 20.0 },
        },
        50,
    );
    assert!(invalid.status.recorded_directions.is_empty());
    assert!(
        invalid
            .effects
            .iter()
            .all(|e| !matches!(e, Effect::Match(_)))
    );
    assert!(matches!(invalid.effects.last(), Some(Effect::Drag { .. })));
    engine.close(60);
    assert_eq!(saved.endpoints.last(), Some(&Point { x: 20.0, y: 0.0 }));
}

#[test]
fn host_timing_and_tuning_boundaries() {
    use pointer_gesture_recognition::{InvalidTuning, duration_nanoseconds as ns, validate_tuning};
    for (seconds, expected) in [
        (f64::NAN, 0),
        (f64::INFINITY, 0),
        (-1.0, 0),
        (0.0, 0),
        (f64::from_bits(1), 1),
        (f64::MIN_POSITIVE, 1),
        (0.000_000_000_1, 1),
        (1.25, 1_250_000_000),
        (1.000_000_000_1, 1_000_000_001),
        (f64::MAX, u64::MAX),
    ] {
        assert_eq!(ns(seconds), expected, "{seconds}");
    }
    assert_eq!(validate_tuning(0.0, 0.0, None, None, 1), Ok(()));
    assert_eq!(
        validate_tuning(f64::NAN, -1.0, Some(0.0), None, 0),
        Err(InvalidTuning::StartDistance)
    );
    assert_eq!(
        validate_tuning(1.0, 1.0, None, Some(f64::INFINITY), 1),
        Err(InvalidTuning::IdleDuration)
    );
}

#[test]
fn deleting_and_clearing_future_patterns_preserves_active_session() {
    for clear in [false, true] {
        let mut matcher = Matcher::default();
        matcher.register(&[Direction::Right], 0).unwrap();
        matcher.register(&[Direction::Down], u64::MAX).unwrap();
        let mut engine = Engine::new(configuration()).unwrap();
        engine.use_matcher(&matcher);
        engine.step(Event::Start, 0);
        engine.step(down(0.0), 1);
        if clear {
            matcher.clear();
        } else {
            assert_eq!(matcher.remove(&[Direction::Right]), Some(0));
            assert_eq!(matcher.remove(&[Direction::Right]), None);
        }
        engine.use_matcher(&matcher);
        engine.step(moved(20.0), 2);
        let done = engine.step(
            Event::Up {
                button: 1,
                point: Point { x: 20.0, y: 0.0 },
            },
            3,
        );
        assert!(matches!(done.effects.last(), Some(Effect::Match(0))));
        let next = engine.step(down(0.0), 4);
        assert_eq!(
            next.disposition,
            if clear {
                Disposition::PassThrough
            } else {
                Disposition::Consume
            }
        );
        let closed = engine.close(5);
        assert!(!closed.effects.iter().any(|e| matches!(e, Effect::Match(_))));
    }
}

// Algorithm cases belong here; adapters test translation, delivery and lifetimes.
#[test]
fn thresholds_axes_and_exact_prefixes_share_one_behavior_matrix() {
    type Case = (&'static str, bool, &'static [(f64, f64)], Option<u64>);
    let cases: [Case; 8] = [
        (
            "start threshold differs from turn threshold",
            true,
            &[(11.0, 0.0)],
            Some(10),
        ),
        ("below start threshold", true, &[(9.0, 0.0)], None),
        ("no dominant axis", true, &[(20.0, 20.0)], None),
        (
            "incomplete exact pattern",
            false,
            &[(90.0, 0.0), (110.0, 0.0)],
            None,
        ),
        (
            "exact minimum turn",
            false,
            &[(10.0, 0.0), (10.0, 25.0)],
            Some(20),
        ),
        (
            "short shared prefix",
            true,
            &[(90.0, 0.0), (110.0, 0.0)],
            Some(10),
        ),
        (
            "long shared prefix",
            true,
            &[(90.0, 0.0), (90.0, 80.0)],
            Some(20),
        ),
        (
            "invalid branch after prefix",
            false,
            &[(90.0, 0.0), (90.0, -90.0)],
            None,
        ),
    ];
    for (name, short, path, expected) in cases {
        let mut matcher = Matcher::default();
        matcher
            .register(&[Direction::Right, Direction::Down], 20)
            .unwrap();
        if short {
            matcher.register(&[Direction::Right], 10).unwrap();
        }
        let mut config = configuration();
        config.change_distance = 25.0;
        let mut engine = Engine::new(config).unwrap();
        engine.use_matcher(&matcher);
        engine.step(Event::Start, 0);
        engine.step(down(0.0), 1);
        let mut effects = Vec::new();
        for (i, &(x, y)) in path.iter().enumerate() {
            let output = engine.step(
                Event::Move {
                    button: 1,
                    point: Point { x, y },
                },
                i as u64 + 2,
            );
            if name == "no dominant axis" {
                assert!(output.trace.directions.is_empty());
            }
            effects.extend(output.effects);
        }
        let &(x, y) = path.last().unwrap();
        let completed = engine.step(
            Event::Up {
                button: 1,
                point: Point { x, y },
            },
            20,
        );
        assert_eq!(completed.stage, InputStage::Idle, "{name}");
        effects.extend(completed.effects);
        let matches: Vec<_> = effects
            .iter()
            .filter_map(|effect| {
                if let Effect::Match(id) = effect {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(matches, expected.into_iter().collect::<Vec<_>>(), "{name}");
    }
}

#[test]
fn pending_move_and_release_jitter_recover_one_click_at_the_press() {
    for moved_before_release in [false, true] {
        let mut matcher = Matcher::default();
        matcher.register(&[Direction::Right], 1).unwrap();
        let mut engine = Engine::new(configuration()).unwrap();
        engine.use_matcher(&matcher);
        engine.step(Event::Start, 0);
        engine.step(down(0.0), 1);
        if moved_before_release {
            engine.step(moved(9.0), 2);
        }
        let output = engine.step(
            Event::Up {
                button: 1,
                point: Point { x: 2.0, y: 1.0 },
            },
            3,
        );
        assert!(
            matches!(output.effects.as_slice(), [Effect::Click {point, button:1}] if *point == Point {x:0.0,y:0.0})
        );
        assert_eq!(output.stage, InputStage::Idle);
        engine.close(4);
    }
}

#[test]
fn validation_reports_first_unreachable_run_without_mutating_snapshots() {
    use pointer_gesture_recognition::{PatternError, validate_pattern};
    let mut matcher = Matcher::default();
    matcher.register(&[Direction::Up], 0).unwrap();
    let captured = matcher.clone();
    for (pattern, error) in [
        (vec![], PatternError::Empty),
        (vec![Direction::Up; 4097], PatternError::TooLong),
        (
            vec![
                Direction::Up,
                Direction::Right,
                Direction::Right,
                Direction::Right,
            ],
            PatternError::RepeatedDirection { index: 2 },
        ),
    ] {
        assert_eq!(validate_pattern(&pattern), Err(error));
        assert_eq!(matcher.register(&pattern, u64::MAX), Err(error));
        assert_eq!(matcher.match_pattern(&[Direction::Up]), Some(0));
        assert_eq!(captured.match_pattern(&[Direction::Up]), Some(0));
    }
}
