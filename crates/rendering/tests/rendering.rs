use pointer_gesture_rendering::*;

fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn path_points(path: &Path) -> Vec<Point> {
    path.commands
        .iter()
        .flat_map(|c| match *c {
            Command::Move(p) | Command::Line(p) => vec![p],
            Command::Cubic {
                control1,
                control2,
                to,
            } => vec![control1, control2, to],
            Command::Close => vec![],
        })
        .collect()
}

#[test]
fn website_examples_fit_the_viewport_with_orthogonal_cubic_tangents() {
    use Direction::{Down, Left, Right};

    for line_width in [3., 4.] {
        let [left, down, corner] = [&[Left][..], &[Down], &[Down, Right]]
            .map(|directions| pattern(directions, 440., 240., 38., line_width, false, 1.).unwrap());
        let [Command::Move(start), Command::Line(end)] = left.path.commands.as_slice() else {
            panic!("Expected a straight left run");
        };
        assert_eq!(start.y, end.y);
        assert!(end.x < start.x);

        let [Command::Move(start), Command::Line(end)] = down.path.commands.as_slice() else {
            panic!("Expected a straight down run");
        };
        assert_eq!(start.x, end.x);
        assert!(end.y > start.y);

        let [
            Command::Move(_),
            Command::Line(entry),
            Command::Cubic {
                control1,
                control2,
                to,
            },
            Command::Line(_),
        ] = corner.path.commands.as_slice()
        else {
            panic!("Expected a cubic between two straight runs");
        };
        assert_eq!(entry.x, control1.x);
        assert_eq!(control2.y, to.y);
        assert!(control1.y > entry.y && to.x > control2.x);

        for geometry in [left, down, corner] {
            assert!(geometry.arrowhead.is_none());
            for point in path_points(&geometry.path) {
                assert!((0. ..=440.).contains(&point.x));
                assert!((0. ..=240.).contains(&point.y));
            }
        }
    }
}

#[test]
fn website_recorded_traces_project_wobble_and_keep_the_final_turn_in_bounds() {
    let west = live_trace(
        &[Direction::Left],
        &[p(300., 120.), p(100., 118.)],
        Some(p(100., 118.)),
        CornerStyle::default(),
    )
    .unwrap();
    assert_eq!(
        west.commands,
        [Command::Move(p(300., 120.)), Command::Line(p(100., 120.))]
    );

    let turn = live_trace(
        &[Direction::Down, Direction::Right],
        &[p(100., 40.), p(100., 160.), p(280., 160.)],
        Some(p(280., 160.)),
        CornerStyle::default(),
    )
    .unwrap();
    assert!(matches!(
        turn.commands.as_slice(),
        [
            Command::Move(_),
            Command::Line(_),
            Command::Cubic { .. },
            Command::Line(_)
        ]
    ));
    assert_eq!(turn.commands[3], Command::Line(p(280., 160.)));
    for point in path_points(&turn) {
        assert!((100. - 1e-9..=280. + 1e-9).contains(&point.x));
        assert!((40. - 1e-9..=160. + 1e-9).contains(&point.y));
    }
}

#[test]
fn preview_scaling_preserves_arrow_and_shaft_proportions() {
    for directions in [
        vec![Direction::Right],
        vec![Direction::Up],
        vec![Direction::Right, Direction::Down],
        vec![
            Direction::Right,
            Direction::Left,
            Direction::Right,
            Direction::Left,
        ],
    ] {
        let original = pattern(&directions, 96., 64., 8., 3., true, 1.).unwrap();
        for scale in [0.5, 2.] {
            let resized = pattern(
                &directions,
                96. * scale,
                64. * scale,
                8. * scale,
                3. * scale,
                true,
                1.,
            )
            .unwrap();
            for (a, b) in [
                (&original.path, &resized.path),
                (
                    original.arrowhead.as_ref().unwrap(),
                    resized.arrowhead.as_ref().unwrap(),
                ),
            ] {
                let original_points = path_points(a);
                let resized_points = path_points(b);
                assert_eq!(original_points.len(), resized_points.len());
                for (a, b) in original_points.iter().zip(&resized_points) {
                    assert!(
                        (a.x * scale - b.x).abs() < 1e-9,
                        "{directions:?}: {a:?} -> {b:?}"
                    );
                    assert!(
                        (a.y * scale - b.y).abs() < 1e-9,
                        "{directions:?}: {a:?} -> {b:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn preview_repeated_reversals_do_not_retrace_earlier_runs() {
    for directions in [
        [
            Direction::Right,
            Direction::Left,
            Direction::Right,
            Direction::Left,
        ],
        [
            Direction::Down,
            Direction::Up,
            Direction::Down,
            Direction::Up,
        ],
        [
            Direction::Right,
            Direction::Left,
            Direction::Down,
            Direction::Up,
        ],
    ] {
        let geometry = pattern(&directions, 112., 68., 8., 4., true, 0.).unwrap();
        let points = path_points(&geometry.path);
        let segments: Vec<_> = points.windows(2).collect();
        for (i, a) in segments.iter().enumerate() {
            assert!(
                a[0].x == a[1].x || a[0].y == a[1].y,
                "Straight runs must stay on XY axes"
            );
            for b in &segments[i + 1..] {
                let overlap = |a: f64, b: f64, c: f64, d: f64| {
                    a.max(b).min(c.max(d)) - a.min(b).max(c.min(d)) > 1e-9
                };
                let horizontal = a[0].y == a[1].y
                    && b[0].y == b[1].y
                    && (a[0].y - b[0].y).abs() < 1e-9
                    && overlap(a[0].x, a[1].x, b[0].x, b[1].x);
                let vertical = a[0].x == a[1].x
                    && b[0].x == b[1].x
                    && (a[0].x - b[0].x).abs() < 1e-9
                    && overlap(a[0].y, a[1].y, b[0].y, b[1].y);
                assert!(
                    !horizontal && !vertical,
                    "{directions:?}: {a:?} retraces {b:?}"
                );
            }
        }
    }
}

#[test]
fn preview_short_reversals_keep_their_final_arrow_at_thick_strokes() {
    for directions in [
        [
            Direction::Right,
            Direction::Left,
            Direction::Right,
            Direction::Left,
        ],
        [
            Direction::Down,
            Direction::Up,
            Direction::Down,
            Direction::Up,
        ],
        [
            Direction::Right,
            Direction::Left,
            Direction::Down,
            Direction::Up,
        ],
    ] {
        for width in [2., 4., 16.] {
            let geometry = pattern(&directions, 112., 68., 8., width, true, 1.).unwrap();
            assert!(
                geometry.arrowhead.is_some(),
                "{directions:?}, {width}pt needs a visible final direction"
            );
        }
    }
}

#[test]
fn preview_reversal_lanes_rotate_with_the_pattern() {
    for roundness in [0., 0.5, 1.5] {
        let a = pattern(
            &[
                Direction::Right,
                Direction::Left,
                Direction::Right,
                Direction::Left,
            ],
            112.,
            68.,
            8.,
            4.,
            true,
            roundness,
        )
        .unwrap();
        let b = pattern(
            &[
                Direction::Down,
                Direction::Up,
                Direction::Down,
                Direction::Up,
            ],
            68.,
            112.,
            8.,
            4.,
            true,
            roundness,
        )
        .unwrap();
        for (a, b) in [
            (&a.path, &b.path),
            (a.arrowhead.as_ref().unwrap(), b.arrowhead.as_ref().unwrap()),
        ] {
            let a = path_points(a);
            let b = path_points(b);
            assert_eq!(a.len(), b.len());
            for (a, b) in a.iter().zip(b) {
                assert!((68. - a.y - b.x).abs() < 1e-9);
                assert!((a.x - b.y).abs() < 1e-9);
            }
        }
    }
}

#[test]
fn live_paths_validate_inputs_and_keep_endpoints_and_sharp_turns() {
    assert_eq!(
        live_trace(&[], &[p(0., 0.)], None, CornerStyle::default()),
        Err(GeometryError::Empty)
    );
    assert_eq!(
        live_trace(&[Direction::Right], &[], None, CornerStyle::default()),
        Err(GeometryError::PointCount)
    );
    assert_eq!(
        live_trace(
            &[Direction::Right],
            &[p(0., 0.), p(f64::NAN, 1.)],
            None,
            CornerStyle::default()
        ),
        Err(GeometryError::NonFinite)
    );
    let points = [p(0., 0.), p(80., 0.), p(80., 80.)];
    let sharp = live_trace(
        &[Direction::Right, Direction::Down],
        &points,
        None,
        CornerStyle { radius_scale: 0. },
    )
    .unwrap();
    assert_eq!(
        sharp.commands,
        vec![
            Command::Move(points[0]),
            Command::Line(points[1]),
            Command::Line(points[2])
        ]
    );
    let rounded = live_trace(
        &[Direction::Right, Direction::Down],
        &points,
        None,
        CornerStyle::default(),
    )
    .unwrap();
    assert_eq!(rounded.commands.first(), Some(&Command::Move(points[0])));
    assert_eq!(rounded.commands.last(), Some(&Command::Line(points[2])));
    assert!(
        rounded
            .commands
            .iter()
            .any(|c| matches!(c, Command::Cubic { .. }))
    );
}

#[test]
fn all_cardinal_transitions_and_degenerate_geometry_stay_finite() {
    let directions = [
        Direction::Up,
        Direction::Down,
        Direction::Left,
        Direction::Right,
    ];
    for &a in &directions {
        for &b in &directions {
            let first = pattern(&[a, b], 96., 64., 8., 3.0, true, 1.0).unwrap();
            let hidden = pattern(&[a, b], 96., 64., 8., 3.0, false, 1.0).unwrap();
            assert_eq!(first.path, hidden.path);
            assert!(first.path.is_finite());
            assert_eq!(
                first.arrowhead.unwrap().commands.last(),
                Some(&Command::Close)
            );
        }
    }
    for size in [0., -1., f64::NAN, f64::INFINITY, 1., 20.] {
        let value = pattern(&directions, size, size, 8., 3.0, true, 1.0).unwrap();
        assert!(value.path.is_finite());
        assert!(value.arrowhead.as_ref().is_none_or(Path::is_finite));
    }
    let cramped = pattern(&[Direction::Right], 20., 20., 100., 3., true, 1.).unwrap();
    let center = p(10., 10.);
    assert_eq!(
        cramped.path.commands,
        [Command::Move(center), Command::Line(center)]
    );
    assert!(cramped.arrowhead.is_none());
    let huge = pattern(&directions, f64::MAX, f64::MAX, 8., 3.0, true, 1.0).unwrap();
    assert!(huge.path.is_finite());
    assert!(huge.arrowhead.as_ref().is_none_or(Path::is_finite));
    let coincident = live_trace(
        &[Direction::Right, Direction::Down],
        &[p(1., 1.); 3],
        None,
        CornerStyle::default(),
    )
    .unwrap();
    assert!(coincident.is_finite());
    let huge = live_trace(
        &[Direction::Right, Direction::Down],
        &[p(-f64::MAX, 0.), p(f64::MAX, 0.), p(f64::MAX, f64::MAX)],
        None,
        CornerStyle::default(),
    );
    assert_eq!(huge, Err(GeometryError::NonFinite));
}

#[test]
fn every_preview_control_fits_its_padding_across_pattern_complexity() {
    use Direction::{Down, Left, Right, Up};

    let dense = [Up, Right, Left, Down].repeat(1024);
    for (directions, padding, shows_arrowhead) in [
        (&[Right][..], 6., false),
        (&[Right, Down][..], 6., false),
        (&[Right, Down, Left, Up, Right][..], 6., false),
        (
            &[Up, Down, Left, Right, Down, Up, Right, Left][..],
            6.,
            false,
        ),
        (dense.as_slice(), 8., true),
    ] {
        let geometry = pattern(directions, 96., 64., padding, 3., shows_arrowhead, 1.).unwrap();
        let points = path_points(&geometry.path);
        assert!(!points.is_empty());
        assert!(
            geometry.arrowhead.is_none(),
            "A hidden arrow or a dense final run must leave only the shaft"
        );
        for point in points {
            assert!((padding - 1e-9..=96. - padding + 1e-9).contains(&point.x));
            assert!((padding - 1e-9..=64. - padding + 1e-9).contains(&point.y));
        }
    }
}

#[test]
fn conservative_visibility_includes_tail_stroke_and_touching_edges() {
    let bounds = Bounds::from_points(&[p(0., 0.), p(10., 10.)]).unwrap();
    let viewport = Bounds {
        min_x: 11.,
        min_y: 0.,
        max_x: 20.,
        max_y: 10.,
    };
    assert!(!bounds.intersects(None, viewport, 0.));
    assert!(bounds.intersects(None, viewport, 2.));
    assert!(bounds.intersects(
        None,
        Bounds {
            min_x: 10.,
            ..viewport
        },
        0.
    ));
    assert!(!bounds.intersects(
        None,
        Bounds {
            min_x: 12.,
            ..viewport
        },
        2.
    ));
    assert!(bounds.intersects(Some(p(12., 0.)), viewport, 0.));
    assert!(!bounds.intersects(Some(p(f64::NAN, 0.)), viewport, 0.));
    assert!(Bounds::from_points(&[]).is_none());
}

#[test]
fn visibility_rejects_invalid_viewports_and_normalizes_invalid_widths() {
    let bounds = Bounds::from_points(&[p(0., 0.), p(10., 10.)]).unwrap();
    let viewport = Bounds {
        min_x: 11.,
        min_y: 0.,
        max_x: 20.,
        max_y: 10.,
    };
    for width in [-4., f64::NAN, f64::NEG_INFINITY, f64::INFINITY] {
        assert!(!bounds.intersects(None, viewport, width));
    }
    for invalid in [
        Bounds {
            min_x: f64::NAN,
            ..viewport
        },
        Bounds {
            min_y: f64::NEG_INFINITY,
            ..viewport
        },
        Bounds {
            max_x: f64::INFINITY,
            ..viewport
        },
        Bounds {
            max_x: 10.,
            ..viewport
        },
        Bounds {
            max_y: -1.,
            ..viewport
        },
    ] {
        assert!(!bounds.intersects(None, invalid, 40.), "{invalid:?}");
    }
}

#[test]
fn loop_exit_and_live_tail_remain_visible() {
    let value = pattern(
        &[
            Direction::Right,
            Direction::Down,
            Direction::Left,
            Direction::Up,
            Direction::Right,
        ],
        60.,
        38.,
        6.,
        3.0,
        false,
        1.0,
    )
    .unwrap();
    let cubics = value
        .path
        .commands
        .iter()
        .filter(|command| matches!(command, Command::Cubic { .. }))
        .count();
    assert_eq!(cubics, 4);
    let endpoints: Vec<_> = value
        .path
        .commands
        .iter()
        .filter_map(|command| match *command {
            Command::Move(point) | Command::Line(point) | Command::Cubic { to: point, .. } => {
                Some(point)
            }
            Command::Close => None,
        })
        .collect();
    let visible = Bounds::from_points(&endpoints).unwrap();
    assert!(visible.max_x - visible.min_x > 10.);
    assert!(visible.max_y - visible.min_y > 10.);
    let hull = Bounds::from_points(&path_points(&value.path)).unwrap();
    assert!(hull.min_x >= 6. - 1e-9 && hull.max_x <= 54. + 1e-9);
    assert!(hull.max_y - hull.min_y < 32.);
    let Command::Move(start) = value.path.commands[0] else {
        panic!("missing start");
    };
    let Command::Line(end) = *value.path.commands.last().unwrap() else {
        panic!("missing end");
    };
    assert!(end.x > start.x && end.y > start.y + 0.5);
    let tail = p(40., 40.);
    let value = live_trace(
        &[Direction::Right],
        &[p(10., 10.), p(40., 10.)],
        Some(tail),
        CornerStyle::default(),
    )
    .unwrap();
    assert_eq!(
        value.commands.last(),
        Some(&Command::Line(p(40., 10.))),
        "Tail feedback must follow the captured horizontal axis"
    );
    assert_eq!(
        live_trace(
            &[Direction::Right],
            &[p(10., 10.), p(40., 10.)],
            Some(p(f64::NAN, 0.)),
            CornerStyle::default()
        ),
        Err(GeometryError::NonFinite)
    );
}

#[test]
fn live_trace_ignores_orthogonal_overflow_for_endpoints_and_tails() {
    let max = f64::MAX;
    let style = CornerStyle::default();
    for (direction, start, sample, projected) in [
        (Direction::Right, p(11., -max), p(51., max), p(51., -max)),
        (Direction::Left, p(11., max), p(-29., -max), p(-29., max)),
        (Direction::Down, p(-max, 11.), p(max, 51.), p(-max, 51.)),
        (Direction::Up, p(max, 11.), p(-max, -29.), p(max, -29.)),
    ] {
        let committed = live_trace(&[direction], &[start, sample], None, style).unwrap();
        let moving = live_trace(&[direction], &[start, start], Some(sample), style).unwrap();
        assert_eq!(
            committed.commands,
            [Command::Move(start), Command::Line(projected)],
            "{direction:?} endpoint"
        );
        assert_eq!(moving, committed, "{direction:?} tail");
    }
}

#[test]
fn live_trace_rejects_projected_distance_overflow_for_endpoints_and_tails() {
    let max = f64::MAX;
    for (direction, start, sample) in [
        (Direction::Right, p(-max, 11.), p(max, 17.)),
        (Direction::Left, p(max, 11.), p(-max, 17.)),
        (Direction::Down, p(11., -max), p(17., max)),
        (Direction::Up, p(11., max), p(17., -max)),
    ] {
        for (points, tail) in [([start, sample], None), ([start, start], Some(sample))] {
            assert_eq!(
                live_trace(&[direction], &points, tail, CornerStyle::default()),
                Err(GeometryError::NonFinite),
                "{direction:?}, tail: {tail:?}"
            );
        }
    }
}

#[test]
fn website_trace_projection_preserves_origin_and_orthogonal_corners() {
    let points = [p(30., 35.), p(180., 45.), p(191., 175.)];
    let path = live_trace(
        &[Direction::Right, Direction::Down],
        &points,
        Some(points[2]),
        CornerStyle::default(),
    )
    .unwrap();
    assert_eq!(
        path.commands.len(),
        4,
        "Captured endpoint is projected once, without a raw-tail spur"
    );
    assert_eq!(path.commands[0], Command::Move(p(30., 35.)));
    assert_eq!(path.commands[1], Command::Line(p(167., 35.)));
    let Command::Cubic {
        control1,
        control2,
        to,
    } = path.commands[2]
    else {
        panic!("Expected rounded corner")
    };
    for (actual, expected) in [
        (control1, p(183., 35.)),
        (control2, p(191., 43.)),
        (to, p(191., 59.)),
    ] {
        assert!((actual.x - expected.x).abs() < 1e-9 && (actual.y - expected.y).abs() < 1e-9);
    }
    assert_eq!(path.commands[3], Command::Line(p(191., 175.)));
    let long = live_trace(
        &[Direction::Right, Direction::Down],
        &[p(30., 35.), p(330., 35.), p(330., 205.)],
        None,
        CornerStyle::default(),
    )
    .unwrap();
    assert_eq!(
        long.commands[1],
        Command::Line(p(306., 35.)),
        "Long gestures keep the website's 24-point corner limit"
    );
}

#[test]
fn preview_centers_the_capped_arrow_and_clears_loop_exits() {
    for directions in [
        vec![Direction::Right],
        vec![Direction::Down],
        vec![Direction::Left],
        vec![Direction::Up],
        vec![Direction::Right, Direction::Down],
    ] {
        for width in [2., 4., 16.] {
            let geometry = pattern(&directions, 112., 68., 8., width, true, 1.0).unwrap();
            let mut b = Bounds::from_points(&path_points(&geometry.path)).unwrap();
            b.min_x -= width / 2.;
            b.min_y -= width / 2.;
            b.max_x += width / 2.;
            b.max_y += width / 2.;
            for p in path_points(geometry.arrowhead.as_ref().unwrap()) {
                b.min_x = b.min_x.min(p.x);
                b.min_y = b.min_y.min(p.y);
                b.max_x = b.max_x.max(p.x);
                b.max_y = b.max_y.max(p.y);
            }
            assert!(((b.min_x + b.max_x) / 2. - 56.).abs() < 1e-9);
            assert!(((b.min_y + b.max_y) / 2. - 34.).abs() < 1e-9);
            assert!(b.min_x >= 8. - 1e-9 && b.max_x <= 104. + 1e-9);
            assert!(b.min_y >= 8. - 1e-9 && b.max_y <= 60. + 1e-9);
        }
    }
    let geometry = pattern(
        &[
            Direction::Right,
            Direction::Down,
            Direction::Left,
            Direction::Up,
            Direction::Right,
        ],
        96.,
        64.,
        8.,
        3.0,
        true,
        1.0,
    )
    .unwrap();
    let Command::Line(end) = *geometry.path.commands.last().unwrap() else {
        panic!("Expected final direction run")
    };
    let earlier = Path {
        commands: geometry.path.commands[..geometry.path.commands.len() - 1].to_vec(),
    };
    let b = Bounds::from_points(&path_points(&earlier)).unwrap();
    assert!(
        end.x > b.max_x + 6.,
        "Loop arrow must exit beyond earlier strokes"
    );
}

#[test]
fn every_straight_live_segment_and_following_tail_stays_on_an_axis() {
    for a in [
        Direction::Up,
        Direction::Down,
        Direction::Left,
        Direction::Right,
    ] {
        for b in [
            Direction::Up,
            Direction::Down,
            Direction::Left,
            Direction::Right,
        ] {
            let path = live_trace(
                &[a, b],
                &[p(10., 12.), p(80., 21.), p(94., 90.)],
                Some(p(110., 102.)),
                CornerStyle::default(),
            )
            .unwrap();
            let mut previous = None;
            for command in path.commands {
                match command {
                    Command::Move(to) => previous = Some(to),
                    Command::Line(to) => {
                        let from = previous.unwrap();
                        assert!(
                            (from.x - to.x).abs() < 1e-9 || (from.y - to.y).abs() < 1e-9,
                            "Straight segment is diagonal: {from:?} to {to:?}"
                        );
                        previous = Some(to);
                    }
                    Command::Cubic { to, .. } => previous = Some(to),
                    Command::Close => unreachable!(),
                }
            }
        }
    }
    let horizontal = live_trace(
        &[Direction::Right],
        &[p(10., 12.), p(80., 21.)],
        Some(p(110., 30.)),
        CornerStyle::default(),
    )
    .unwrap();
    assert_eq!(
        horizontal.commands,
        vec![Command::Move(p(10., 12.)), Command::Line(p(110., 12.))]
    );
}

#[test]
fn arrow_remains_visible_outside_a_round_stroke_cap() {
    for width in [3., 12.] {
        let geometry = pattern(&[Direction::Right], 96., 64., 0., width, true, 1.0).unwrap();
        let Command::Line(end) = geometry.path.commands.last().unwrap() else {
            panic!("No shaft")
        };
        let arrow = geometry.arrowhead.unwrap();
        let Command::Line(tip) = arrow.commands[2] else {
            panic!("No tip")
        };
        let Command::Move(base1) = arrow.commands[0] else {
            panic!("No first base")
        };
        let Command::Line(base2) = arrow.commands[1] else {
            panic!("No second base")
        };
        assert!(
            tip.x > end.x + width / 2.,
            "A rounded shaft must not cover the arrow tip"
        );
        assert!((base1.y - base2.y).abs() >= width * 1.5);
    }
    let cramped = pattern(&[Direction::Down], 20., 20., 0., 12., true, 1.0).unwrap();
    assert!(cramped.path.is_finite() && cramped.arrowhead.is_none());
}

#[test]
fn preview_roundness_changes_curves_without_moving_the_arrowhead() {
    let turn = [Direction::Right, Direction::Down];
    let two_turns = [Direction::Right, Direction::Down, Direction::Right];
    for (directions, width, height, padding, line_width) in [
        (&turn[..], 112., 68., 0., 3.),
        (&turn[..], 112., 68., 8., 3.),
        (&turn[..], 96., 64., 0., 3.),
        (&turn[..], 96., 64., 8., 3.),
        (&two_turns[..], 112., 68., 8., 4.),
    ] {
        let plan =
            |scale| pattern(directions, width, height, padding, line_width, true, scale).unwrap();
        let has_curve = |path: &Path| {
            path.commands
                .iter()
                .any(|c| matches!(c, Command::Cubic { .. }))
        };
        let sharp = plan(0.);
        assert!(!has_curve(&sharp.path));
        assert!(sharp.arrowhead.is_some());

        let [subtle, standard, rounded] = [0.5, 1., 1.5].map(plan);
        for preview in [&subtle, &standard, &rounded] {
            assert!(has_curve(&preview.path));
            assert_eq!(preview.arrowhead, sharp.arrowhead);
        }
        assert_ne!(subtle.path, standard.path);
        assert_ne!(standard.path, rounded.path);
        assert_ne!(subtle.path, rounded.path);

        for invalid in [f64::NAN, f64::INFINITY, -1.] {
            assert_eq!(plan(invalid), sharp);
        }
        assert!(plan(f64::MAX).path.is_finite());
    }
}
