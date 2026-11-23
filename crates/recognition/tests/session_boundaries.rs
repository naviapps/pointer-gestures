use pointer_gesture_recognition::*;
use std::sync::Arc;

fn configuration() -> Configuration {
    Configuration {
        start_distance: 10.0,
        change_distance: 5.0,
        total_duration_ns: 1_000,
        idle_duration_ns: 100,
        maximum_points: 32,
        maximum_directions: 64,
        button: 1,
        passes_through_empty_matcher: true,
    }
}
fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}
fn recording_engine(config: Configuration) -> Engine {
    let mut engine = Engine::new(config).unwrap();
    engine.step(Event::Start, 0);
    engine.step(Event::Recording(true), 0);
    engine
}
#[test]
fn established_loop_remains_visible_when_it_returns_to_its_origin() {
    let mut engine = recording_engine(configuration());
    engine.step(
        Event::Down {
            button: 1,
            point: point(0.0, 0.0),
        },
        1,
    );
    for (index, p) in [
        point(40.0, 0.0),
        point(40.0, 40.0),
        point(0.0, 40.0),
        point(0.0, 0.0),
    ]
    .into_iter()
    .enumerate()
    {
        let output = engine.step(
            Event::Move {
                button: 1,
                point: p,
            },
            2 + index as u64,
        );
        assert!(
            output.trace.visible,
            "an established path disappeared at {p:?}"
        );
    }
    let completed = engine.step(
        Event::Up {
            button: 1,
            point: point(0.0, 0.0),
        },
        10,
    );
    assert_eq!(
        completed.status.recorded_directions.as_ref(),
        &[
            Direction::Right,
            Direction::Down,
            Direction::Left,
            Direction::Up
        ]
    );
    assert!(!completed.trace.visible);
    engine.close(11);
}
#[test]
fn stationary_events_preserve_the_owned_snapshot_without_trace_notifications() {
    let mut engine = recording_engine(configuration());
    engine.step(
        Event::Down {
            button: 1,
            point: point(0.0, 0.0),
        },
        1,
    );
    let first = engine.step(
        Event::Move {
            button: 1,
            point: point(20.0, 0.0),
        },
        2,
    );
    for now in 3..20 {
        let output = engine.step(
            Event::Move {
                button: 1,
                point: point(20.0, 0.0),
            },
            now,
        );
        assert_eq!(output.trace, first.trace);
        assert!(Arc::ptr_eq(
            &output.trace.raw_points,
            &first.trace.raw_points
        ));
        assert!(
            !output
                .effects
                .iter()
                .any(|e| matches!(e, Effect::Publish { .. }))
        );
        assert_eq!(output.deadline_ns, Some(102));
    }
    engine.close(21);
}
#[test]
fn moving_snapshots_share_only_unchanged_directions_and_outlive_capture() {
    let mut engine = recording_engine(configuration());
    let down = |x, y| Event::Down {
        button: 1,
        point: point(x, y),
    };
    let movement = |x, y| Event::Move {
        button: 1,
        point: point(x, y),
    };
    engine.step(down(0.0, 0.0), 1);
    let first = engine.step(movement(20.0, 0.0), 2);
    let moved = engine.step(movement(40.0, 0.0), 3);
    assert!(Arc::ptr_eq(
        &first.trace.directions,
        &moved.trace.directions
    ));
    assert_eq!(
        first.trace.endpoints.as_ref(),
        &[point(0.0, 0.0), point(20.0, 0.0)]
    );
    assert_eq!(
        moved.trace.endpoints.as_ref(),
        &[point(0.0, 0.0), point(40.0, 0.0)]
    );

    let tail_only = engine.step(movement(40.0, 1.0), 4);
    assert_eq!(tail_only.trace.endpoints, moved.trace.endpoints);
    assert_eq!(tail_only.trace.tail, Some(point(40.0, 1.0)));
    let turned = engine.step(movement(40.0, 20.0), 5);
    let extended = engine.step(movement(40.0, 40.0), 6);
    assert_eq!(
        turned.trace.directions.as_ref(),
        &[Direction::Right, Direction::Down]
    );
    assert!(!Arc::ptr_eq(
        &moved.trace.directions,
        &turned.trace.directions
    ));
    assert!(Arc::ptr_eq(
        &turned.trace.directions,
        &extended.trace.directions
    ));
    assert_eq!(turned.trace.endpoints.last(), Some(&point(40.0, 20.0)));
    assert_eq!(extended.trace.endpoints.last(), Some(&point(40.0, 40.0)));
    for output in [&first, &moved, &tail_only, &turned, &extended] {
        assert!(
            matches!(output.effects.as_slice(), [Effect::Publish { trace, .. }] if trace == &output.trace)
        );
    }
    let cancelled = engine.step(Event::Cancel, 7);
    assert!(cancelled.discard_pending_visible_trace && !cancelled.trace.visible);
    assert!(
        matches!(cancelled.effects.first(), Some(Effect::Release { point: p, .. }) if *p == point(40.0, 40.0))
    );

    // A same-length sequence in a new session must not reuse stale directions.
    engine.step(down(100.0, 100.0), 8);
    let next = engine.step(movement(80.0, 100.0), 9);
    let closed = engine.close(10);
    assert_eq!(next.trace.directions.as_ref(), &[Direction::Left]);
    assert_eq!(first.trace.directions.as_ref(), &[Direction::Right]);
    assert_eq!(
        first.trace.endpoints.as_ref(),
        &[point(0.0, 0.0), point(20.0, 0.0)]
    );
    assert_eq!(
        turned.trace.directions.as_ref(),
        &[Direction::Right, Direction::Down]
    );
    assert_eq!(closed.stage, InputStage::Idle);
}

#[test]
fn saturated_reported_deadline_expires_on_its_tick() {
    let mut config = configuration();
    config.idle_duration_ns = 0;
    let mut engine = recording_engine(config);
    let start = engine.step(
        Event::Down {
            button: 1,
            point: point(0.0, 0.0),
        },
        u64::MAX - 10,
    );
    assert_eq!(start.deadline_ns, Some(u64::MAX));
    let expired = engine.step(Event::Tick, start.deadline_ns.unwrap());
    assert_eq!(expired.status.failure, Some(Failure::Expired));
    assert_eq!(expired.input_generation, 0);
    assert!(matches!(
        expired.effects.first(),
        Some(Effect::Click { .. })
    ));
    engine.close(u64::MAX);
}

#[test]
fn moves_at_the_deadline_expire_pending_and_active_input_in_one_transition() {
    for active in [false, true] {
        let mut config = configuration();
        config.idle_duration_ns = 0;
        let mut engine = recording_engine(config);
        let down = engine.step(
            Event::Down {
                button: 1,
                point: point(0.0, 0.0),
            },
            1,
        );
        if active {
            engine.step(
                Event::Move {
                    button: 1,
                    point: point(40.0, 0.0),
                },
                2,
            );
        }
        let output = engine.step(
            Event::Move {
                button: 1,
                point: point(140.0, 0.0),
            },
            down.deadline_ns.unwrap(),
        );
        assert_eq!(output.disposition, Disposition::PassThrough);
        assert_eq!(output.stage, InputStage::Idle);
        assert!(!output.trace.visible);
        assert_eq!(output.status.failure, Some(Failure::Expired));
        assert_eq!(
            output.last_session.unwrap().outcome,
            SessionOutcome::TotalDeadline
        );
        if active {
            assert!(matches!(
                output.effects.as_slice(),
                [Effect::Release { point: p, .. }, Effect::Publish { .. }]
                    if *p == point(40.0, 0.0)
            ));
        } else {
            assert!(matches!(
                output.effects.as_slice(),
                [Effect::Click { point: p, .. }] if *p == point(0.0, 0.0)
            ));
        }
    }
}

#[test]
fn recording_accepts_the_release_that_first_reaches_the_start_distance() {
    let mut engine = recording_engine(configuration());
    let start = point(0.0, 0.0);
    let drag = point(8.0, 0.0);
    let end = point(10.0, 0.0);
    engine.step(
        Event::Down {
            button: 1,
            point: start,
        },
        1,
    );
    let pending = engine.step(
        Event::Move {
            button: 1,
            point: drag,
        },
        2,
    );
    assert_eq!(pending.stage, InputStage::Pending);
    assert!(!pending.trace.visible);
    // The capture is still a recording even when the next press will not be.
    engine.step(Event::Recording(false), 3);
    let finished = engine.step(
        Event::Up {
            button: 1,
            point: end,
        },
        4,
    );

    let report = finished.last_session.unwrap();
    assert_eq!(
        report.outcome,
        SessionOutcome::Recorded,
        "The release must finish the pending recording after a real drag sample"
    );
    assert!(report.captured && report.recording);
    assert_eq!(report.point_count, 3);
    assert_eq!(report.directions.as_ref(), &[Direction::Right]);
    assert_eq!(
        finished.status.recorded_directions.as_ref(),
        &[Direction::Right]
    );
    assert_eq!(finished.disposition, Disposition::Consume);
    assert_eq!(finished.stage, InputStage::Idle);
    assert!(matches!(
        finished.effects.as_slice(),
        [Effect::Publish { trace: visible, .. }, Effect::Publish { trace: hidden, .. },
            Effect::Drag { button: 1, points }]
        if visible.visible && visible.endpoints.as_ref() == [start, end]
            && !hidden.visible && points.as_ref() == [start, drag, end]
    ));
}

#[test]
fn terminal_recording_does_not_promote_stationary_short_or_diagonal_input() {
    let start = point(0.0, 0.0);
    for (drag, end, expects_click) in [
        (start, point(10.0, 0.0), false),
        (point(8.0, 0.0), point(9.0, 0.0), true),
        (point(8.0, 0.0), point(10.0, 10.0), false),
    ] {
        let mut engine = recording_engine(configuration());
        engine.step(
            Event::Down {
                button: 1,
                point: start,
            },
            1,
        );
        assert_eq!(
            engine
                .step(
                    Event::Move {
                        button: 1,
                        point: drag
                    },
                    2
                )
                .stage,
            InputStage::Pending
        );
        let finished = engine.step(
            Event::Up {
                button: 1,
                point: end,
            },
            3,
        );
        let report = finished.last_session.unwrap();
        assert_eq!(report.outcome, SessionOutcome::BelowThreshold);
        assert!(report.directions.is_empty());
        if expects_click {
            assert!(matches!(
                finished.effects.as_slice(),
                [Effect::Click { button: 1, point }] if *point == start
            ));
        } else {
            assert!(matches!(
                finished.effects.as_slice(),
                [Effect::Drag { button: 1, points }] if points.last() == Some(&end)
            ));
        }
    }
}

#[test]
fn release_does_not_start_matching_or_extend_expired_recording() {
    let start = point(0.0, 0.0);
    let drag = point(8.0, 0.0);
    let end = point(10.0, 0.0);
    let mut matcher = Matcher::default();
    matcher.register(&[Direction::Right], 1).unwrap();
    let mut recognition = Engine::new(configuration()).unwrap();
    recognition.use_matcher(&matcher);
    recognition.step(Event::Start, 0);
    recognition.step(
        Event::Down {
            button: 1,
            point: start,
        },
        1,
    );
    recognition.step(
        Event::Move {
            button: 1,
            point: drag,
        },
        2,
    );
    recognition.step(Event::Recording(true), 3);
    let finished = recognition.step(
        Event::Up {
            button: 1,
            point: end,
        },
        4,
    );
    let report = finished.last_session.unwrap();
    assert_eq!(report.outcome, SessionOutcome::BelowThreshold);
    assert!(!report.recording);
    assert!(matches!(
        finished.effects.as_slice(),
        [Effect::Drag { button: 1, points }] if points.as_ref() == [start, drag, end]
    ));

    let mut recording = recording_engine(configuration());
    recording.step(
        Event::Down {
            button: 1,
            point: start,
        },
        1,
    );
    let pending = recording.step(
        Event::Move {
            button: 1,
            point: drag,
        },
        2,
    );
    let finished = recording.step(
        Event::Up {
            button: 1,
            point: end,
        },
        pending.deadline_ns.unwrap(),
    );
    assert_eq!(finished.disposition, Disposition::PassThrough);
    assert_eq!(
        finished.last_session.unwrap().outcome,
        SessionOutcome::IdleDeadline
    );
    assert!(matches!(
        finished.effects.as_slice(),
        [Effect::Click { button: 1, point }] if *point == start
    ));
}

#[test]
fn release_without_move_events_recovers_a_drag_but_preserves_click_jitter() {
    for (end, is_drag) in [(point(2.0, 1.0), false), (point(40.0, 0.0), true)] {
        let mut engine = recording_engine(configuration());
        engine.step(
            Event::Down {
                button: 1,
                point: point(0.0, 0.0),
            },
            1,
        );
        let output = engine.step(
            Event::Up {
                button: 1,
                point: end,
            },
            2,
        );
        assert_eq!(output.stage, InputStage::Idle);
        if is_drag {
            assert!(
                matches!(output.effects.as_slice(), [Effect::Drag { points, .. }]
                if points.as_ref() == [point(0.0, 0.0), end])
            );
        } else {
            assert!(
                matches!(output.effects.as_slice(), [Effect::Click { point: p, .. }]
                if *p == point(0.0, 0.0))
            );
        }
        engine.close(3);
    }
}

#[test]
fn small_raw_buffers_still_publish_recent_points_on_the_normal_cadence() {
    for maximum_points in [1, 3, 8] {
        let mut config = configuration();
        config.maximum_points = maximum_points;
        let mut engine = recording_engine(config);
        engine.step(
            Event::Down {
                button: 1,
                point: point(0.0, 0.0),
            },
            1,
        );
        engine.step(
            Event::Move {
                button: 1,
                point: point(20.0, 0.0),
            },
            2,
        );
        for offset in 1..=10 {
            let output = engine.step(
                Event::Move {
                    button: 1,
                    point: point(20.0 + offset as f64, 0.0),
                },
                2 + offset,
            );
            assert!(output.trace.raw_points.len() <= maximum_points);
            if offset == 10 {
                assert_eq!(output.trace.raw_points.last(), Some(&point(30.0, 0.0)));
            }
        }
        engine.close(13);
    }
}

#[test]
fn reducing_the_point_budget_trims_current_trace_and_recovery_without_new_motion() {
    for maximum_points in [1, 3] {
        for active in [false, true] {
            let mut config = configuration();
            config.total_duration_ns = 0;
            config.idle_duration_ns = 0;
            let mut engine = recording_engine(config);
            let sample = |x| point(x, if active { 0.0 } else { x });
            engine.step(
                Event::Down {
                    button: 1,
                    point: sample(0.0),
                },
                0,
            );
            for x in 20_u32..=32 {
                engine.step(
                    Event::Move {
                        button: 1,
                        point: sample(f64::from(x)),
                    },
                    x.into(),
                );
            }
            let before = engine.step(Event::Snapshot, 33);
            let original_raw = before.trace.raw_points.to_vec();
            if active {
                assert_eq!(original_raw.last(), Some(&sample(30.0)));
            }

            config.maximum_points = maximum_points;
            assert_eq!(engine.update_tuning(config, 34, false), Ok(true));
            let after = engine.step(Event::Snapshot, 34);
            assert_eq!(after.stage, before.stage);
            assert_eq!(after.input_generation, before.input_generation);
            assert_eq!(after.trace.directions, before.trace.directions);
            assert_eq!(after.trace.endpoints, before.trace.endpoints);
            if active {
                let expected: Vec<_> = ((33 - maximum_points)..33)
                    .map(|x| sample(x as f64))
                    .collect();
                assert_eq!(after.trace.raw_points.as_ref(), expected);
            }
            assert_eq!(before.trace.raw_points.as_ref(), original_raw);

            let end = if active {
                Event::Up {
                    button: 1,
                    point: sample(32.0),
                }
            } else {
                Event::Cancel
            };
            let ended = engine.step(end, 35);
            let expected = if maximum_points == 1 {
                vec![sample(0.0), sample(32.0)]
            } else {
                vec![sample(0.0), sample(31.0), sample(32.0)]
            };
            assert!(ended.effects.iter().any(
                |effect| matches!(effect, Effect::Drag { points, .. } if points.as_ref() == expected)
            ));
        }
    }
}

#[test]
fn growing_a_reduced_point_budget_does_not_restore_evicted_history() {
    let mut config = configuration();
    config.maximum_points = 8;
    let mut engine = recording_engine(config);
    let origin = point(0.0, 0.0);
    engine.step(
        Event::Down {
            button: 1,
            point: origin,
        },
        0,
    );
    for x in 20_u32..=32 {
        engine.step(
            Event::Move {
                button: 1,
                point: point(f64::from(x), 0.0),
            },
            x.into(),
        );
    }
    let original = engine.step(Event::Snapshot, 33).trace;
    let retained = point(32.0, 0.0);
    config.maximum_points = 1;
    assert_eq!(engine.update_tuning(config, 34, false), Ok(true));
    let reduced = engine.step(Event::Snapshot, 34).trace;
    assert_eq!(reduced.raw_points.as_ref(), [retained]);

    config.maximum_points = 6;
    assert_eq!(engine.update_tuning(config, 35, false), Ok(true));
    let grown = engine.step(Event::Snapshot, 35).trace;
    assert_eq!(grown.raw_points.as_ref(), [retained]);
    assert_eq!(grown.endpoints, original.endpoints);

    let next = point(33.0, 0.0);
    let turn = point(33.0, 10.0);
    engine.step(
        Event::Move {
            button: 1,
            point: next,
        },
        36,
    );
    let turned = engine.step(
        Event::Move {
            button: 1,
            point: turn,
        },
        37,
    );
    assert_eq!(turned.trace.raw_points.as_ref(), [retained, next, turn]);
    let ended = engine.step(
        Event::Up {
            button: 1,
            point: turn,
        },
        38,
    );
    assert!(matches!(
        ended.effects.as_slice(),
        [.., Effect::Drag { button: 1, points }]
            if points.as_ref() == [origin, retained, next, turn]
    ));
    assert_eq!(original.raw_points.last(), Some(&point(30.0, 0.0)));
    assert_eq!(reduced.raw_points.as_ref(), [retained]);
}

#[test]
fn reducing_the_point_budget_preserves_pending_drag_recovery_classification() {
    let origin = point(0.0, 0.0);
    let tail = point(1.0, 1.0);
    for end in [
        Event::Cancel,
        Event::Up {
            button: 1,
            point: tail,
        },
    ] {
        let mut config = configuration();
        config.total_duration_ns = 0;
        config.idle_duration_ns = 0;
        let mut engine = recording_engine(config);
        engine.step(
            Event::Down {
                button: 1,
                point: origin,
            },
            0,
        );
        engine.step(
            Event::Move {
                button: 1,
                point: point(20.0, 20.0),
            },
            1,
        );
        let pending = engine.step(
            Event::Move {
                button: 1,
                point: tail,
            },
            2,
        );
        assert_eq!(pending.stage, InputStage::Pending);

        config.maximum_points = 1;
        assert_eq!(engine.update_tuning(config, 3, false), Ok(true));
        let ended = engine.step(end, 4);
        assert!(ended.effects.iter().any(|effect|
            matches!(effect, Effect::Drag { points, .. } if points.as_ref() == [origin, tail])
        ));
    }
}

#[test]
fn terminal_input_recovers_finite_history_once_and_in_order() {
    let origin = point(0., 0.);
    let malformed_up = |point| {
        (
            Event::Up { button: 1, point },
            SessionOutcome::InvalidRelease,
        )
    };
    let terminals = [
        (Event::Cancel, SessionOutcome::Cancelled),
        malformed_up(point(f64::NAN, 0.)),
        malformed_up(point(0., f64::INFINITY)),
        malformed_up(point(f64::NEG_INFINITY, 0.)),
        malformed_up(point(f64::NAN, f64::INFINITY)),
    ];
    let mut matcher = Matcher::default();
    matcher.register(&[Direction::Right], 42).unwrap();
    let mut config = configuration();
    config.total_duration_ns = 0;
    config.idle_duration_ns = 0;

    for (terminal, outcome) in terminals {
        for last in [
            None,
            Some(point(2., 1.)),
            Some(point(40., 40.)),
            Some(point(40., 0.)),
        ] {
            let mut engine = Engine::new(config).unwrap();
            engine.use_matcher(&matcher);
            engine.step(Event::Start, 0);
            let pressed = engine.step(
                Event::Down {
                    button: 1,
                    point: origin,
                },
                1,
            );
            if let Some(point) = last {
                engine.step(Event::Move { button: 1, point }, 2);
            }
            let output = engine.step(terminal, 3);
            assert_eq!(output.stage, InputStage::Idle);
            assert_eq!(output.disposition, Disposition::Consume);
            assert_eq!(output.input_generation, 0);
            assert_eq!(output.deadline_ns, None);
            assert!(output.discard_pending_visible_trace);
            assert_eq!(output.trace, Trace::default());
            assert!(output.status.recorded_directions.is_empty());
            let report = output.last_session.as_ref().unwrap();
            assert_eq!(report.generation, pressed.input_generation);
            assert_eq!(report.outcome, outcome);
            assert_eq!(report.captured, last == Some(point(40., 0.)));
            assert_eq!(report.point_count, if last.is_some() { 2 } else { 1 });
            assert_eq!(
                output.effects,
                match last {
                    Some(p) if p == point(40., 0.) => vec![
                        Effect::Release {
                            button: 1,
                            point: p
                        },
                        Effect::Publish {
                            status: output.status.clone(),
                            trace: Trace::default()
                        },
                    ],
                    Some(p) if p == point(40., 40.) => vec![Effect::Drag {
                        button: 1,
                        points: vec![origin, p].into(),
                    }],
                    _ => vec![Effect::Click {
                        button: 1,
                        point: origin
                    }],
                },
                "{terminal:?} with last point {last:?}"
            );
            for later in [
                Event::Up {
                    button: 1,
                    point: point(80., 0.),
                },
                terminal,
                Event::Cancel,
            ] {
                let after = engine.step(later, 4);
                assert_eq!(after.disposition, Disposition::PassThrough);
                assert!(after.effects.is_empty());
                assert_eq!(after.last_session.as_ref(), Some(report));
            }
            assert!(engine.close(5).effects.is_empty());
        }
    }
}

#[test]
fn nonfinite_motion_and_unrelated_releases_preserve_valid_capture() {
    let mut matcher = Matcher::default();
    matcher.register(&[Direction::Right], 42).unwrap();
    let mut engine = Engine::new(configuration()).unwrap();
    engine.use_matcher(&matcher);
    engine.step(Event::Start, 0);
    engine.step(
        Event::Down {
            button: 1,
            point: point(0., 0.),
        },
        1,
    );
    let active = engine.step(
        Event::Move {
            button: 1,
            point: point(40., 0.),
        },
        2,
    );
    for event in [
        Event::Down {
            button: 1,
            point: point(f64::NAN, 0.),
        },
        Event::Move {
            button: 1,
            point: point(0., f64::INFINITY),
        },
        Event::Up {
            button: 2,
            point: point(f64::NAN, 0.),
        },
    ] {
        let output = engine.step(event, 3);
        assert_eq!(output.disposition, Disposition::PassThrough);
        assert_eq!(output.trace, active.trace);
        assert_eq!(output.deadline_ns, active.deadline_ns);
        assert!(output.effects.is_empty());
    }
    let output = engine.step(
        Event::Up {
            button: 1,
            point: point(50., 0.),
        },
        4,
    );
    assert_eq!(output.effects.last(), Some(&Effect::Match(42)));
}

#[test]
fn terminal_input_at_deadline_expires_and_recovers_once() {
    let origin = point(0., 0.);
    let moved = point(40., 0.);
    let mut matcher = Matcher::default();
    matcher.register(&[Direction::Right], 42).unwrap();
    for button in [0, 1] {
        let finite_up = Event::Up {
            button,
            point: point(80., 0.),
        };
        let malformed_up = Event::Up {
            button,
            point: point(f64::NAN, f64::INFINITY),
        };
        let unrelated_up = Event::Up {
            button: 1 - button,
            point: point(f64::NAN, f64::INFINITY),
        };
        for (total, idle, reason) in [
            (10, 0, SessionOutcome::TotalDeadline),
            (0, 10, SessionOutcome::IdleDeadline),
        ] {
            for captured in [false, true] {
                for (terminal, disposition, recording) in [
                    (Event::Cancel, Disposition::PassThrough, true),
                    (finite_up, Disposition::PassThrough, false),
                    (malformed_up, Disposition::Consume, false),
                    (malformed_up, Disposition::Consume, true),
                ] {
                    let mut engine = Engine::new(Configuration {
                        button,
                        total_duration_ns: total,
                        idle_duration_ns: idle,
                        ..configuration()
                    })
                    .unwrap();
                    engine.use_matcher(&matcher);
                    engine.step(Event::Start, 0);
                    engine.step(Event::Recording(recording), 0);
                    let pressed = engine.step(
                        Event::Down {
                            button,
                            point: origin,
                        },
                        1,
                    );
                    assert_eq!(pressed.stage, InputStage::Pending);
                    let held = if captured {
                        engine.step(
                            Event::Move {
                                button,
                                point: moved,
                            },
                            2,
                        )
                    } else {
                        pressed
                    };
                    assert_eq!(held.trace.visible, captured);
                    let deadline = held.deadline_ns.unwrap();
                    assert_eq!(
                        engine.step(unrelated_up, deadline),
                        Output {
                            disposition: Disposition::PassThrough,
                            effects: vec![],
                            ..held
                        },
                        "Unrelated malformed input must leave the retained press untouched"
                    );

                    let output = engine.step(terminal, deadline);
                    assert_eq!(output.disposition, disposition, "{terminal:?}");
                    assert_eq!(output.stage, InputStage::Idle);
                    assert_eq!(output.input_generation, 0);
                    assert_eq!(output.deadline_ns, None);
                    assert_eq!(output.status.failure, Some(Failure::Expired));
                    assert!(output.discard_pending_visible_trace);
                    assert_eq!(output.trace, Trace::default());
                    assert_eq!(output.last_session.as_ref().unwrap().outcome, reason);
                    let expected_effects = if captured {
                        vec![
                            Effect::Release {
                                button,
                                point: moved,
                            },
                            Effect::Publish {
                                status: output.status.clone(),
                                trace: Trace::default(),
                            },
                        ]
                    } else {
                        vec![Effect::Click {
                            button,
                            point: origin,
                        }]
                    };
                    assert_eq!(output.effects, expected_effects, "{terminal:?}");
                    for later in [finite_up, malformed_up, Event::Cancel] {
                        let next = engine.step(later, deadline + 1);
                        assert_eq!(next.disposition, Disposition::PassThrough);
                        assert_eq!(next.last_session, output.last_session);
                        assert!(next.effects.is_empty());
                    }
                    assert!(engine.close(deadline + 2).effects.is_empty());
                }
            }
        }
    }
}
