use pointer_gesture_recognition::{
    Configuration, Direction, Effect, Engine, Event, Matcher, Point,
};

fn main() {
    let mut matcher = Matcher::default();
    matcher.register(&[Direction::Right], 42).unwrap();
    let mut engine = Engine::new(Configuration {
        start_distance: 10.0,
        change_distance: 5.0,
        total_duration_ns: 1_000,
        idle_duration_ns: 100,
        maximum_points: 8,
        maximum_directions: 16,
        button: 1,
        passes_through_empty_matcher: true,
    })
    .unwrap();
    engine.use_matcher(&matcher);
    engine.step(Event::Start, 0);
    engine.step(
        Event::Down {
            button: 1,
            point: Point { x: 4.0, y: 8.0 },
        },
        1,
    );
    let endpoint = Point { x: 24.0, y: 8.0 };
    let active = engine.step(
        Event::Move {
            button: 1,
            point: endpoint,
        },
        2,
    );
    assert!(active.trace.visible);
    let completed = engine.step(
        Event::Up {
            button: 1,
            point: endpoint,
        },
        3,
    );
    assert!(matches!(completed.effects.last(), Some(Effect::Match(42))));

    #[cfg(feature = "geometry")]
    {
        use pointer_gesture_rendering::{CornerStyle, OverlayTrace, live_trace};

        // Passing recognition's own types proves both Git dependencies share their identity.
        let (trace, reused) = OverlayTrace::from_snapshot(
            None,
            active.trace.visible,
            &active.trace.directions,
            &active.trace.endpoints,
            active.trace.tail,
        )
        .unwrap()
        .unwrap();
        assert!(!reused);
        let path = live_trace(
            trace.directions(),
            trace.points(),
            trace.tail(),
            CornerStyle::default(),
        )
        .unwrap();
        assert!(path.is_finite() && !path.commands.is_empty());

        #[cfg(feature = "raster")]
        {
            use pointer_gesture_rendering::raster;

            let image = raster::stroke(
                &path,
                32,
                16,
                Point { x: 0.0, y: 0.0 },
                1.0,
                2.0,
                [255, 255, 255, 255],
            )
            .unwrap();
            assert!(image.pixels().chunks_exact(4).any(|pixel| pixel[3] > 0));
        }
    }
    println!("PointerGestures Rust Git consumer passed");
}
