use pointer_gesture_rendering::{
    Bounds, Direction, GeometryError, OverlayState, OverlayTiming, OverlayTrace, Point,
};
fn trace(x: f64) -> OverlayTrace {
    OverlayTrace::new(
        &[Direction::Right],
        &[Point { x: 0.0, y: 0.0 }, Point { x, y: 0.0 }],
        None,
    )
    .unwrap()
}
#[test]
fn coalesces_and_presents_final_trace_before_hiding() {
    let mut overlay = OverlayState::new(OverlayTiming::new(0.01, 24.0), None, 0);
    assert!(overlay.update(Some(trace(1.0)), 0).changed);
    let pending = overlay.update(Some(trace(2.0)), 1_000_000);
    assert!(!pending.changed);
    assert_eq!(pending.deadline_ns, Some(10_000_000));
    let pending = overlay.update(Some(trace(3.0)), 2_000_000);
    assert_eq!(pending.deadline_ns, Some(10_000_000));
    let final_frame = overlay.update(None, 3_000_000);
    assert_eq!(final_frame.trace, Some(trace(3.0)));
    assert_eq!(final_frame.deadline_ns, Some(23_000_000));
    assert!(!overlay.tick(22_999_999).changed);
    assert!(overlay.tick(23_000_000).trace.is_none());
    assert_eq!(final_frame.trace, Some(trace(3.0))); // owned snapshot
}
#[test]
fn returning_to_presented_geometry_cancels_older_pending_movement() {
    let mut overlay = OverlayState::new(OverlayTiming::new(0.01, 24.0), Some(trace(1.0)), 0);
    let queued = overlay.update(Some(trace(2.0)), 1);
    assert!(!queued.changed);
    assert_eq!(queued.deadline_ns, Some(10_000_000));

    let returned = overlay.update(Some(trace(1.0)), 2);
    assert!(!returned.changed);
    assert_eq!(returned.deadline_ns, None);
    let elapsed = overlay.tick(10_000_000);
    assert!(!elapsed.changed);
    assert_eq!(elapsed.trace, Some(trace(1.0)));
}
#[test]
fn resumed_trace_cancels_hide_and_lag_bypasses_are_rate_limited() {
    let mut overlay = OverlayState::new(OverlayTiming::new(0.01, 2.0), Some(trace(1.0)), 0);
    assert!(overlay.update(Some(trace(10.0)), 1).changed);
    assert!(!overlay.update(Some(trace(20.0)), 2).changed);
    let hide = overlay.update(None, 3);
    assert_eq!(hide.trace, Some(trace(20.0)));
    assert!(!overlay.update(Some(trace(20.0)), 4).changed);
    assert!(overlay.tick(100_000_000).trace.is_some());
    assert!(overlay.update(Some(trace(30.0)), 100_000_001).changed);
}
#[test]
fn direction_changes_are_immediate_and_clocks_never_regress() {
    let mut overlay = OverlayState::new(OverlayTiming::default(), Some(trace(1.0)), 100);
    let next = OverlayTrace::new(
        &[Direction::Down],
        &[Point { x: 0.0, y: 0.0 }, Point { x: 0.0, y: 3.0 }],
        None,
    )
    .unwrap();
    assert!(overlay.update(Some(next), 0).changed);
    assert_eq!(
        overlay.update(None, 0).deadline_ns,
        Some(
            100 + pointer_gesture_recognition::duration_nanoseconds(
                OverlayTiming::default().interval_seconds
            ) * 2
        )
    );
}
#[test]
fn trace_constructor_requires_nonempty_directions_matching_counts_and_finite_points() {
    let directions = &[Direction::Right];
    let origin = Point { x: 0.0, y: 0.0 };
    let end = Point {
        x: f64::MAX,
        y: -f64::MAX,
    };
    let tail = Point {
        x: -f64::MAX,
        y: f64::MAX,
    };
    for points in [&[][..], &[origin]] {
        assert_eq!(
            OverlayTrace::new(&[], points, None),
            Err(GeometryError::Empty)
        );
    }
    for points in [&[origin][..], &[origin, end, tail]] {
        assert_eq!(
            OverlayTrace::new(directions, points, None),
            Err(GeometryError::PointCount)
        );
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for invalid in [Point { x: value, y: 0.0 }, Point { x: 0.0, y: value }] {
            assert_eq!(
                OverlayTrace::new(directions, &[origin, invalid], None),
                Err(GeometryError::NonFinite)
            );
            assert_eq!(
                OverlayTrace::new(directions, &[origin, end], Some(invalid)),
                Err(GeometryError::NonFinite)
            );
        }
    }
    let valid = OverlayTrace::new(directions, &[origin, end], Some(tail)).unwrap();
    assert_eq!(valid.points(), &[origin, end]);
    assert_eq!(valid.tail(), Some(tail));
}

#[test]
fn normalizes_cadence_and_saturates_hide_deadlines() {
    assert_eq!(
        OverlayTiming::new(f64::INFINITY, -1.0),
        OverlayTiming::default()
    );
    let t = trace(1.0);
    assert!(t.intersects(
        Bounds {
            min_x: 2.0,
            min_y: 0.0,
            max_x: 3.0,
            max_y: 1.0
        },
        2.0
    ));
    let mut overlay = OverlayState::new(OverlayTiming::new(0.1, 24.0), Some(t), u64::MAX - 1);
    assert_eq!(
        overlay.update(None, u64::MAX - 1).deadline_ns,
        Some(u64::MAX)
    );
    assert!(overlay.tick(u64::MAX).trace.is_none());
}

#[test]
fn large_finite_lag_uses_distance_without_squaring_overflow() {
    let mut overlay = OverlayState::new(OverlayTiming::new(0.01, 1.0e308), Some(trace(1.0)), 0);
    assert!(overlay.update(Some(trace(1.6e308)), 1).changed);
    assert_eq!(pointer_gesture_rendering::finite_or_zero(-10.0), -10.0);
    assert_eq!(pointer_gesture_rendering::non_negative(f64::NAN), 0.0);
}

#[test]
fn reconfiguration_preserves_pending_geometry_and_anchors_deadlines_to_presentation() {
    let mut state = OverlayState::new(OverlayTiming::new(0.01, 24.0), Some(trace(1.0)), 0);
    state.update(Some(trace(2.0)), 1_000_000);
    let slower = state.reconfigure(OverlayTiming::new(0.02, 24.0), 2_000_000);
    assert!(!slower.changed);
    assert_eq!(slower.trace, Some(trace(1.0)));
    assert_eq!(slower.deadline_ns, Some(20_000_000));
    let faster = state.reconfigure(OverlayTiming::new(0.001, 24.0), 3_000_000);
    assert!(faster.changed);
    assert_eq!(faster.trace, Some(trace(2.0)));
    assert_eq!(faster.deadline_ns, None);
    state.update(None, 3_500_000);
    let hidden = state.reconfigure(OverlayTiming::new(0.0001, 24.0), 4_000_000);
    assert!(hidden.changed && hidden.trace.is_none() && hidden.deadline_ns.is_none());
}

#[test]
fn reconfiguration_never_restarts_final_hide_or_drops_final_geometry() {
    let mut state = OverlayState::new(OverlayTiming::new(0.01, 24.0), Some(trace(1.0)), 0);
    state.update(Some(trace(2.0)), 1_000_000);
    let final_frame = state.update(None, 2_000_000);
    assert_eq!(final_frame.trace, Some(trace(2.0)));
    let slower = state.reconfigure(OverlayTiming::new(0.02, 24.0), 3_000_000);
    assert_eq!(slower.trace, final_frame.trace);
    assert_eq!(slower.deadline_ns, Some(42_000_000));
    let same = state.reconfigure(OverlayTiming::new(0.02, 24.0), 30_000_000);
    assert_eq!(same.deadline_ns, slower.deadline_ns);
    assert!(!same.changed);
    assert!(state.tick(42_000_000).trace.is_none());
}

#[test]
fn reconfiguration_normalizes_timing_clamps_clock_and_reconsiders_lag() {
    let mut state = OverlayState::new(OverlayTiming::new(0.01, 24.0), Some(trace(1.0)), 100);
    state.update(Some(trace(10.0)), 101);
    let lag = state.reconfigure(OverlayTiming::new(0.01, 0.0), 0);
    assert!(lag.changed);
    assert_eq!(lag.trace, Some(trace(10.0)));
    state.update(Some(trace(11.0)), 102);
    let normalized = state.reconfigure(
        OverlayTiming {
            interval_seconds: f64::NAN,

            maximum_point_lag: -1.0,
        },
        0,
    );
    assert_eq!(
        normalized.deadline_ns,
        Some(
            101 + pointer_gesture_recognition::duration_nanoseconds(
                OverlayTiming::default().interval_seconds
            )
        )
    );
    assert_eq!(normalized.trace, lag.trace);
}

#[test]
fn scalar_normalization_preserves_finite_boundaries_and_rejects_nonfinite_values() {
    for value in [
        -f64::MAX,
        -f64::MIN_POSITIVE,
        -f64::from_bits(1),
        0.0,
        f64::from_bits(1),
        f64::MIN_POSITIVE,
        f64::MAX,
    ] {
        assert_eq!(pointer_gesture_rendering::finite_or_zero(value), value);
        assert_eq!(
            pointer_gesture_rendering::non_negative(value),
            value.max(0.0)
        );
    }
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(pointer_gesture_rendering::finite_or_zero(value), 0.0);
        assert_eq!(pointer_gesture_rendering::non_negative(value), 0.0);
    }
}

#[test]
fn clear_discards_pending_movement_and_hide_without_resurrection() {
    for hide_first in [false, true] {
        let mut overlay = OverlayState::new(OverlayTiming::new(0.01, 24.), None, 10);
        let retained = overlay.update(Some(trace(1.)), 10);
        overlay.update(Some(trace(2.)), 11);
        if hide_first {
            overlay.update(None, 12);
        }
        let cleared = overlay.clear(9); // regressing clocks still clamp
        assert!(cleared.changed);
        assert_eq!(cleared.trace, None);
        assert_eq!(cleared.deadline_ns, None);
        assert!(!overlay.clear(13).changed);
        assert_eq!(overlay.tick(u64::MAX).trace, None);
        let resumed = overlay.update(Some(trace(3.)), u64::MAX);
        assert!(resumed.changed);
        assert_eq!(resumed.deadline_ns, None);
        assert_eq!(retained.trace, Some(trace(1.)));
    }
    let mut overlay = OverlayState::new(OverlayTiming::new(0.01, 24.), None, 0);
    overlay.clear(0);
    overlay.update(Some(trace(1.)), 1);
    assert_eq!(
        overlay.update(Some(trace(2.)), 2).deadline_ns,
        Some(10_000_001)
    );
}

#[test]
fn tail_replacement_shares_geometry_preserves_bounds_and_rejects_invalid_values() {
    let original = trace(2.);
    let next = original.with_tail(Some(Point { x: 200., y: 30. })).unwrap();
    let viewport = Bounds {
        min_x: 199.,
        min_y: 29.,
        max_x: 201.,
        max_y: 31.,
    };
    assert_eq!(original.directions().as_ptr(), next.directions().as_ptr());
    assert_eq!(original.points().as_ptr(), next.points().as_ptr());
    assert_eq!(original.tail(), None);
    assert!(next.intersects(viewport, 0.));
    assert!(!original.intersects(viewport, 0.));
    let outside = next.with_tail(Some(Point { x: 180., y: 30. })).unwrap();
    let removed = next.with_tail(None).unwrap();
    assert!(!outside.intersects(viewport, 0.));
    assert!(!removed.intersects(viewport, 0.));
    assert_eq!(removed, original);
    assert_eq!(
        OverlayTrace::new(original.directions(), original.points(), None).unwrap(),
        original
    );
    assert_ne!(next, original);
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(next.with_tail(Some(Point { x: value, y: 0. })).is_err());
    }
    drop(original);
    assert_eq!(next.points().last().unwrap().x, 2.);
    assert!(next.intersects(viewport, 0.));
}

#[test]
fn snapshot_conversion_uses_previous_geometry_only_when_directions_and_points_match() {
    let directions = [Direction::Right, Direction::Down];
    let points = [
        Point { x: 0., y: 0. },
        Point { x: 40., y: 0. },
        Point { x: 40., y: 20. },
    ];
    let tail = Point { x: 40., y: 30. };
    let convert = |previous, directions: &[Direction], points: &[Point], tail| {
        OverlayTrace::from_snapshot(previous, true, directions, points, tail)
            .unwrap()
            .unwrap()
    };
    let (first, reused) = convert(None, &directions, &points, None);
    assert!(!reused);
    let (next, reused) = convert(Some(&first), &directions, &points, Some(tail));
    assert!(reused);
    assert_eq!(next.directions().as_ptr(), first.directions().as_ptr());
    assert_eq!(next.points().as_ptr(), first.points().as_ptr());
    assert_eq!(first.tail(), None);
    assert_eq!(next.tail(), Some(tail));
    assert_eq!(convert(Some(&next), &directions, &points, None).0, first);

    let viewport = Bounds {
        min_x: -2.,
        min_y: -1.,
        max_x: 0.5,
        max_y: 1.,
    };
    let mut changed_points = points;
    changed_points[0].x = 2.;
    let changed_directions = [Direction::Left, Direction::Down];
    for (directions, points, intersects) in [
        (&directions, &changed_points, false),
        (&changed_directions, &points, true),
    ] {
        let (updated, reused) = convert(Some(&first), directions, points, None);
        assert!(!reused);
        assert_ne!(updated.points().as_ptr(), first.points().as_ptr());
        assert_eq!(updated.directions(), directions);
        assert_eq!(updated.points(), points);
        assert_eq!(updated.intersects(viewport, 0.), intersects);
        assert!(first.intersects(viewport, 0.));
    }
    drop(first);
    assert_eq!(next.directions(), directions);
    assert_eq!(next.points(), points);
}

#[test]
fn snapshot_conversion_distinguishes_hidden_geometry_and_invalid_visible_values() {
    let previous = trace(2.);
    let invalid = Point { x: f64::NAN, y: 0. };
    assert_eq!(
        OverlayTrace::from_snapshot(Some(&previous), false, &[], &[], Some(invalid)),
        Ok(None)
    );
    for (directions, points, tail, expected) in [
        (&[][..], &[][..], None, GeometryError::Empty),
        (
            previous.directions(),
            &[][..],
            None,
            GeometryError::PointCount,
        ),
        (
            previous.directions(),
            &[previous.points()[0], invalid],
            None,
            GeometryError::NonFinite,
        ),
        (
            previous.directions(),
            previous.points(),
            Some(invalid),
            GeometryError::NonFinite,
        ),
        (
            previous.directions(),
            previous.points(),
            Some(Point {
                x: 0.,
                y: f64::INFINITY,
            }),
            GeometryError::NonFinite,
        ),
    ] {
        assert_eq!(
            OverlayTrace::from_snapshot(Some(&previous), true, directions, points, tail),
            Err(expected)
        );
    }
    let (reopened, reused) =
        OverlayTrace::from_snapshot(None, true, previous.directions(), previous.points(), None)
            .unwrap()
            .unwrap();
    assert!(!reused);
    assert_eq!(reopened, previous);
    assert_ne!(reopened.points().as_ptr(), previous.points().as_ptr());
}
