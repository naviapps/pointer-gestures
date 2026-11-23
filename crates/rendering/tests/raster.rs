#![cfg(feature = "raster")]
use pointer_gesture_rendering::{Command, Path, Point, raster};

fn path() -> Path {
    Path {
        commands: vec![
            Command::Move(Point { x: -100., y: -40. }),
            Command::Line(Point { x: -70., y: -40. }),
        ],
    }
}
#[test]
fn negative_origin_translation_alpha_and_native_channel_order() {
    let image = raster::stroke(
        &path(),
        40,
        20,
        Point { x: -105., y: -50. },
        1.,
        4.,
        [240, 80, 20, 128],
    )
    .unwrap();
    assert_eq!((image.width(), image.height()), (40, 20));
    let pixel = &image.pixels()[(10 * 40 + 15) * 4..(10 * 40 + 15) * 4 + 4];
    assert_eq!(pixel, [120, 40, 10, 128]);
    assert_eq!(&image.pixels()[0..4], [0, 0, 0, 0]);
    let bytes = image.into_bgra();
    assert_eq!(
        &bytes[(10 * 40 + 15) * 4..(10 * 40 + 15) * 4 + 4],
        [10, 40, 120, 128]
    );
}
#[test]
fn raster_rejects_unbounded_allocation_and_malformed_paths() {
    for (width, height) in [(0, 20), (20000, 1), (4001, 4000), (u32::MAX, u32::MAX)] {
        assert!(
            raster::stroke(
                &path(),
                width,
                height,
                Point { x: 0., y: 0. },
                1.,
                4.,
                [0; 4]
            )
            .is_err()
        );
    }
    for scale in [0., -1., f64::NAN, f64::INFINITY] {
        assert!(
            raster::stroke(&path(), 40, 20, Point { x: 0., y: 0. }, scale, 4., [0; 4]).is_err()
        );
    }
    let bad = Path {
        commands: vec![Command::Line(Point { x: 0., y: 0. })],
    };
    assert!(raster::stroke(&bad, 40, 20, Point { x: 0., y: 0. }, 1., 4., [0; 4]).is_err());
    let huge = Path {
        commands: vec![Command::Move(Point { x: f64::MAX, y: 0. })],
    };
    assert!(raster::stroke(&huge, 40, 20, Point { x: 0., y: 0. }, 1., 4., [0; 4]).is_err());
}

#[test]
fn invalid_widths_never_become_implicit_hairline_strokes() {
    for width in [f64::MIN_POSITIVE, 0., -1., f64::NAN, f64::INFINITY, 4097.] {
        assert!(!raster::validate_line_width(width));
        assert!(
            raster::stroke(&path(), 40, 20, Point { x: 0., y: 0. }, 1., width, [0; 4]).is_err()
        );
    }
    for width in [0.25, 1., 4096.] {
        assert!(raster::validate_line_width(width));
    }
}
