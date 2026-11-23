#[path = "../src/matcher.rs"]
pub mod matcher;

pub mod engine {
    // Read the actual wire structs without exposing their fields to Rust consumers.
    include!("../src/engine.rs");

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::matcher::*;

        struct OwnedFrame(*mut Frame);

        impl Drop for OwnedFrame {
            fn drop(&mut self) {
                unsafe { pointer_gesture_recognition_frame_destroy(self.0) };
            }
        }

        fn step(engine: *mut Engine, kind: u32, point: Point, now_ns: u64) -> OwnedFrame {
            let frame = unsafe {
                pointer_gesture_recognition_engine_step(
                    engine,
                    Event {
                        kind,
                        button: 1,
                        x: point.x,
                        y: point.y,
                        now_ns,
                        value: 0,
                    },
                )
            };
            assert!(!frame.is_null());
            OwnedFrame(frame)
        }

        fn assert_points(view: PointsView, expected: &[Point]) {
            assert_eq!(view.count, expected.len());
            assert!(!view.data.is_null());
            // The caller keeps the frame alive while comparing its borrowed points.
            let points = unsafe { std::slice::from_raw_parts(view.data, view.count) };
            for (actual, expected) in points.iter().zip(expected) {
                assert_eq!(
                    (actual.x.to_bits(), actual.y.to_bits()),
                    (expected.x.to_bits(), expected.y.to_bits()),
                );
            }
        }

        fn assert_direction(view: DirectionsView, direction: u32) {
            assert_eq!(view.count, 1);
            assert!(!view.data.is_null());
            assert_eq!(unsafe { *view.data }, direction);
        }

        #[test]
        fn overflowing_axis_deltas_preserve_finite_coordinates_and_ordered_effects() {
            let max = f64::MAX;
            let cases = [
                (
                    4,
                    Point { x: -max, y: -max },
                    Point {
                        x: max,
                        y: max / 2.0,
                    },
                ),
                (
                    1,
                    Point { x: -max, y: max },
                    Point {
                        x: max / 2.0,
                        y: -max,
                    },
                ),
            ];
            for (direction, start, end) in cases {
                let engine = pointer_gesture_recognition_engine_create(Config {
                    start_distance: 10.0,
                    change_distance: 25.0,
                    total_ns: 0,
                    idle_ns: 0,
                    maximum_points: 256,
                    button: 1,
                    pass_empty: 0,
                    maximum_directions: 4096,
                });
                assert!(!engine.is_null());
                let matcher = pointer_gesture_recognition_matcher_create();
                assert!(!matcher.is_null());
                unsafe {
                    assert_eq!(
                        pointer_gesture_recognition_matcher_register(
                            matcher,
                            &direction,
                            1,
                            u64::MAX
                        )
                        .code,
                        0
                    );
                    pointer_gesture_recognition_engine_use_matcher(engine, matcher);
                    pointer_gesture_recognition_matcher_destroy(matcher);
                }

                drop(step(engine, 6, ZERO, 0));
                drop(step(engine, 1, start, 1));
                let active = step(engine, 2, end, 2);
                let completed = step(engine, 3, end, 3);
                drop(step(engine, 5, ZERO, 4));
                unsafe { pointer_gesture_recognition_engine_destroy(engine) };

                let live = unsafe { pointer_gesture_recognition_frame_view(active.0) };
                assert_eq!((live.disposition, live.stage, live.visible), (1, 2, 1));
                assert_direction(live.directions, direction);
                assert_points(live.endpoints, &[start, end]);
                let final_frame = unsafe { pointer_gesture_recognition_frame_view(completed.0) };
                assert_eq!(
                    (
                        final_frame.disposition,
                        final_frame.stage,
                        final_frame.visible
                    ),
                    (1, 0, 0)
                );
                assert!(final_frame.effect_count >= 2);
                let release = unsafe {
                    pointer_gesture_recognition_frame_effect(
                        completed.0,
                        final_frame.effect_count - 2,
                    )
                };
                let command = unsafe {
                    pointer_gesture_recognition_frame_effect(
                        completed.0,
                        final_frame.effect_count - 1,
                    )
                };
                assert_eq!((release.kind, release.button), (4, 1));
                assert_points(release.points, &[end]);
                assert_eq!((command.kind, command.match_value), (5, u64::MAX));

                let report =
                    unsafe { pointer_gesture_recognition_frame_session_report(completed.0) };
                assert_eq!(
                    (
                        report.present,
                        report.outcome,
                        report.captured,
                        report.rejected_direction
                    ),
                    (1, 1, 1, 0)
                );
                assert_eq!(report.generation, live.input_generation);
                assert_eq!(report.maximum_axis_distance.to_bits(), max.to_bits());
                assert_direction(report.directions, direction);
            }
        }
    }
}
