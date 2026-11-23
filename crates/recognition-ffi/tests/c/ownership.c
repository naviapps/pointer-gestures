#include "pointer_gesture_recognition.h"
#ifdef _WIN32
#include "windows/pointer_gesture_recognition_windows.h"
#endif
#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdio.h>

static PointerGestureRecognitionConfig config(void) {
    return (PointerGestureRecognitionConfig){.start_distance = 10,
                                             .change_distance = 25,
                                             .maximum_points = 4,
                                             .maximum_directions = 128,
                                             .button = 1};
}
static PointerGestureRecognitionFrame *step(PointerGestureRecognitionEngine *engine, uint32_t kind,
                                            double x, double y) {
    return pointer_gesture_recognition_engine_step(
        engine, (PointerGestureRecognitionEvent){.kind = kind, .button = 1, .x = x, .y = y});
}
/* Stop returns owned recovery before memory-only destruction. */
static PointerGestureRecognitionFrame *stop_and_destroy(PointerGestureRecognitionEngine *engine,
                                                        uint64_t now) {
    PointerGestureRecognitionFrame *frame = pointer_gesture_recognition_engine_step(
        engine, (PointerGestureRecognitionEvent){.kind = 5, .now_ns = now});
    pointer_gesture_recognition_engine_destroy(engine);
    return frame;
}
int main(void) {
    assert(pointer_gesture_recognition_abi_version() == POINTER_GESTURE_RECOGNITION_ABI_VERSION);
#ifdef _WIN32
    PointerGestureRecognitionWindowsError windows_error = {0};
    assert(pointer_gesture_recognition_windows_abi_version() ==
           POINTER_GESTURE_RECOGNITION_WINDOWS_ABI_VERSION);
    assert(pointer_gesture_recognition_windows_start_with_context(config(), NULL, 0,
                                                                  &windows_error) == NULL);
    assert(windows_error.kind == 1);
    assert(pointer_gesture_recognition_windows_select_context(NULL, NULL, 0, 0, 0) == 0);
#endif
    assert(pointer_gesture_recognition_frame_session_report(NULL).present == 0);
    assert(!pointer_gesture_recognition_validate_direction_limit(0) &&
           !pointer_gesture_recognition_validate_direction_limit(4097));
    assert(pointer_gesture_recognition_validate_direction_limit(1) &&
           pointer_gesture_recognition_validate_direction_limit(4096));
    PointerGestureRecognitionMovement extreme =
        pointer_gesture_recognition_movement(-DBL_MAX, -DBL_MAX, DBL_MAX, DBL_MAX / 2);
    assert(extreme.valid && extreme.direction == 4 && extreme.maximum_axis_delta == INFINITY);
    assert(pointer_gesture_recognition_duration_nanoseconds(0.0000000001) == 1);
    assert(pointer_gesture_recognition_elapsed_nanoseconds(2, 500000000000000000LL) ==
           2500000000ULL);
    assert(pointer_gesture_recognition_elapsed_nanoseconds(INT64_MAX, 0) == UINT64_MAX);
    assert(pointer_gesture_recognition_validate_tuning(1, 2, 0, 0, 0, 256) == 0);
    assert(pointer_gesture_recognition_validate_tuning(1, 2, 0, 0, 1, 256) == 3);
    /* C callers may release or mutate their delay buffers after installation. */
    double delays[] = {0, 0.5};
    assert(pointer_gesture_recognition_validate_retry_delays(delays, 2) == SIZE_MAX);
    PointerGestureRecognitionStartup *startup = pointer_gesture_recognition_startup_create();
    assert(pointer_gesture_recognition_startup_set_delays(startup, delays, 2) == 1);
    delays[0] = NAN;
    assert(pointer_gesture_recognition_validate_retry_delays(delays, 2) == 0);
    assert(pointer_gesture_recognition_startup_request(startup, 1, 0) == 1);
    uint64_t revision = pointer_gesture_recognition_startup_view(startup).revision;
    assert(pointer_gesture_recognition_startup_request(startup, 6, revision) == 1);
    PointerGestureRecognitionStartupView retrying =
        pointer_gesture_recognition_startup_view(startup);
    assert(retrying.revision == revision && retrying.lifecycle == 3 && retrying.requested);
    uint64_t delay = UINT64_MAX;
    assert(pointer_gesture_recognition_startup_next_delay(startup, &delay) == 1 && delay == 0);
    pointer_gesture_recognition_startup_destroy(startup);
    pointer_gesture_recognition_startup_destroy(NULL);
    /* Two-call replay sampling never writes an undersized output buffer. */
    PointerGestureRecognitionPoint path[] = {{0, 0}, {1, 0}, {2, 0}, {3, 0}, {4, 0}};
    PointerGestureRecognitionPoint sampled[] = {{99, 99}, {99, 99}};
    assert(pointer_gesture_recognition_replay_points(NULL, 5, 2, NULL, 0) == 2);
    assert(pointer_gesture_recognition_replay_points(path, 5, 2, sampled, 1) == 2 &&
           sampled[0].x == 99);
    assert(pointer_gesture_recognition_replay_points(path, 5, 2, sampled, 2) == 2);
    assert(sampled[0].x == 0 && sampled[1].x == 4);
    PointerGestureRecognitionConfig invalid = config();
    invalid.start_distance = NAN;
    assert(pointer_gesture_recognition_engine_create(invalid) == NULL);
    invalid = config();
    invalid.maximum_points = 0;
    assert(pointer_gesture_recognition_engine_create(invalid) == NULL);
    invalid = config();
    invalid.maximum_directions = 0;
    assert(pointer_gesture_recognition_engine_create(invalid) == NULL);

    PointerGestureRecognitionEngine *engine = pointer_gesture_recognition_engine_create(config());
    assert(engine != NULL);
    PointerGestureRecognitionMatcher *matcher = pointer_gesture_recognition_matcher_create();
    uint32_t pattern[128];
    for (size_t i = 0; i < 128; i++)
        pattern[i] = i % 2 == 0 ? 4 : 2;
    assert(pointer_gesture_recognition_matcher_register(matcher, pattern, 128, 999).code == 0);
    assert(pointer_gesture_recognition_matcher_register(matcher, NULL, 0, 1).code != 0);
    uint32_t repeated[] = {4, 4};
    assert(pointer_gesture_recognition_matcher_register(matcher, repeated, 2, 1).code != 0);
    PointerGestureRecognitionPatternValidation empty =
        pointer_gesture_recognition_pattern_validate(NULL, 0);
    assert(empty.code == 1 && empty.index == UINT32_MAX);
    assert(pointer_gesture_recognition_pattern_validate(NULL, 1).code == 5);
    assert(pointer_gesture_recognition_pattern_validate(NULL, SIZE_MAX).code == 2);
    uint32_t duplicate[] = {1, 2, 2};
    PointerGestureRecognitionPatternValidation repeated_error =
        pointer_gesture_recognition_pattern_validate(duplicate, 3);
    assert(repeated_error.code == 3 && repeated_error.index == 2);
    uint32_t bad = 9;
    assert(pointer_gesture_recognition_matcher_register(matcher, &bad, 1, 1).code != 0);
    pointer_gesture_recognition_engine_use_matcher(engine, matcher);
    pointer_gesture_recognition_frame_destroy(step(engine, 6, 0, 0));
    pointer_gesture_recognition_frame_destroy(step(engine, 1, 0, 0));
    PointerGestureRecognitionFrame *held = step(engine, 2, 30, 0);
    PointerGestureRecognitionFrameView borrowed = pointer_gesture_recognition_frame_view(held);
    assert(borrowed.directions.count == 1 && borrowed.directions.data[0] == 4);
    assert(borrowed.endpoints.count == 2 && borrowed.endpoints.data[1].x == 30);
    PointerGestureRecognitionEffectView visible = pointer_gesture_recognition_frame_effect(held, 0);
    assert(visible.kind == 6 && visible.trace.visible == 1);
    assert(visible.trace.endpoints.data[1].x == 30);
    /* The active input retains its old matcher even after replacing a duplicate rule. */
    assert(pointer_gesture_recognition_matcher_register(matcher, pattern, 128, 1000).code == 0);
    assert(pointer_gesture_recognition_matcher_remove(matcher, pattern, 128).value == 1000);
    assert(!pointer_gesture_recognition_matcher_remove(matcher, pattern, 128).present);
    pointer_gesture_recognition_matcher_clear(matcher);
    assert(pointer_gesture_recognition_matcher_is_empty(matcher));
    pointer_gesture_recognition_engine_use_matcher(engine, matcher);
    pointer_gesture_recognition_matcher_destroy(matcher);
    pattern[0] = 1; /* Registration must have copied the caller's pattern. */
    double x = 30, y = 0;
    for (size_t i = 1; i < 128; i++) {
        if (i % 2 == 0)
            x += 30;
        else
            y += 30;
        pointer_gesture_recognition_frame_destroy(step(engine, 2, x, y));
    }
    PointerGestureRecognitionFrame *completed = step(engine, 3, x, y);
    PointerGestureRecognitionFrameView completion =
        pointer_gesture_recognition_frame_view(completed);
    PointerGestureRecognitionSessionReportView report =
        pointer_gesture_recognition_frame_session_report(completed);
    assert(report.present && report.outcome == 1 && report.captured && !report.recording);
    assert(report.point_count == 130 && report.directions.count == 128);
    assert(report.generation != 0 && report.rejected_direction == 0);
    assert(completion.recorded.count == 128 && completion.effect_count >= 3);
    size_t release_index = completion.effect_count - 2;
    assert(pointer_gesture_recognition_frame_effect(completed, release_index - 1).kind == 6);
    assert(pointer_gesture_recognition_frame_effect(completed, release_index - 1).trace.visible ==
           0);
    assert(pointer_gesture_recognition_frame_effect(completed, release_index).kind == 4);
    PointerGestureRecognitionEffectView match =
        pointer_gesture_recognition_frame_effect(completed, release_index + 1);
    assert(match.kind == 5 && match.match_value == 999 && match.points.count == 0);
    PointerGestureRecognitionEffectView absent =
        pointer_gesture_recognition_frame_effect(completed, completion.effect_count);
    assert(absent.kind == 0 && absent.points.count == 0 && absent.points.data == NULL);
    pointer_gesture_recognition_frame_destroy(stop_and_destroy(engine, 0));
    /* Sanitized C reads check the borrowed views after later steps AND engine destruction. */
    assert(borrowed.directions.data[0] == 4 && borrowed.endpoints.data[1].x == 30);
    assert(visible.trace.endpoints.data[1].x == 30 && visible.trace.directions.data[0] == 4);
    assert(completion.recorded.data[127] == 2 &&
           pointer_gesture_recognition_frame_effect(completed, release_index + 1).match_value ==
               999);
    assert(report.directions.data[127] == 2);
    pointer_gesture_recognition_frame_destroy(completed);
    pointer_gesture_recognition_frame_destroy(held);

    for (int active = 0; active < 2; active++) {
        engine = pointer_gesture_recognition_engine_create(config());
        uint32_t right = 4;
        matcher = pointer_gesture_recognition_matcher_create();
        assert(pointer_gesture_recognition_matcher_register(matcher, &right, 1, 1).code == 0);
        pointer_gesture_recognition_engine_use_matcher(engine, matcher);
        pointer_gesture_recognition_matcher_destroy(matcher);
        pointer_gesture_recognition_frame_destroy(step(engine, 6, 0, 0));
        pointer_gesture_recognition_frame_destroy(step(engine, 1, 0, 0));
        if (active)
            pointer_gesture_recognition_frame_destroy(step(engine, 2, 25, 0));
        PointerGestureRecognitionFrame *closing = stop_and_destroy(engine, 0);
        PointerGestureRecognitionFrameView closed = pointer_gesture_recognition_frame_view(closing);
        assert(closed.ready == 0 && closed.stage == 0 && closed.effect_count == (active ? 2u : 1u));
        PointerGestureRecognitionEffectView replay =
            pointer_gesture_recognition_frame_effect(closing, 0);
        assert(replay.kind == (active ? 4u : 1u) && replay.button == 1 && replay.points.count == 1);
        assert(replay.points.data[0].x == (active ? 25.0 : 0.0));
        pointer_gesture_recognition_frame_destroy(closing);
    }
    /* Malformed terminal coordinates cannot retain input or leak into recovery. */
    for (int active = 0; active < 2; active++) {
        engine = pointer_gesture_recognition_engine_create(config());
        pointer_gesture_recognition_frame_destroy(step(engine, 6, 0, 0));
        pointer_gesture_recognition_frame_destroy(pointer_gesture_recognition_engine_step(
            engine, (PointerGestureRecognitionEvent){.kind = 7, .value = 1}));
        pointer_gesture_recognition_frame_destroy(step(engine, 1, 10, 10));
        if (active)
            pointer_gesture_recognition_frame_destroy(step(engine, 2, 40, 10));
        PointerGestureRecognitionFrame *ended = step(engine, 3, NAN, INFINITY);
        PointerGestureRecognitionFrameView end = pointer_gesture_recognition_frame_view(ended);
        assert(end.disposition == 1 && end.stage == 0 && end.input_generation == 0);
        assert(end.visible == 0 && end.discard_visible == 1 && end.deadline_present == 0);
        assert(end.effect_count == (active ? 2u : 1u));
        PointerGestureRecognitionEffectView recovery =
            pointer_gesture_recognition_frame_effect(ended, 0);
        assert(recovery.kind == (active ? 4u : 1u));
        assert(recovery.points.count == 1 && recovery.points.data[0].x == (active ? 40 : 10));
        assert(recovery.points.data[0].y == 10);
        PointerGestureRecognitionFrame *late = step(engine, 3, 80, 10);
        assert(pointer_gesture_recognition_frame_view(late).effect_count == 0);
        pointer_gesture_recognition_frame_destroy(late);
        pointer_gesture_recognition_frame_destroy(stop_and_destroy(engine, 0));
        /* Recovery views retain their finite data after the engine has closed. */
        assert(isfinite(recovery.points.data[0].x) && isfinite(recovery.points.data[0].y));
        pointer_gesture_recognition_frame_destroy(ended);
    }
    /* Memory-only destruction also preserves independently owned active frames. */
    engine = pointer_gesture_recognition_engine_create(config());
    pointer_gesture_recognition_frame_destroy(step(engine, 6, 0, 0));
    pointer_gesture_recognition_frame_destroy(pointer_gesture_recognition_engine_step(
        engine, (PointerGestureRecognitionEvent){.kind = 7, .value = 1}));
    pointer_gesture_recognition_frame_destroy(step(engine, 1, 0, 0));
    PointerGestureRecognitionFrame *retained = step(engine, 2, 40, 0);
    PointerGestureRecognitionFrameView retained_view =
        pointer_gesture_recognition_frame_view(retained);
    pointer_gesture_recognition_engine_destroy(engine);
    assert(retained_view.stage == 2 && retained_view.visible &&
           retained_view.directions.data[0] == 4);
    assert(retained_view.endpoints.count == 2 && retained_view.endpoints.data[1].x == 40);
    pointer_gesture_recognition_frame_destroy(retained);

    matcher = pointer_gesture_recognition_matcher_create();
    uint32_t down_right[] = {2, 4};
    assert(pointer_gesture_recognition_matcher_register(matcher, down_right, 2, UINT64_MAX).code ==
           0);
    PointerGestureRecognitionMatcher *captured = pointer_gesture_recognition_matcher_clone(matcher);
    assert(pointer_gesture_recognition_matcher_is_empty(captured) == 0);
    assert(pointer_gesture_recognition_matcher_register(matcher, down_right, 2, 0).code == 0);
    assert(pointer_gesture_recognition_matcher_match(matcher, down_right, 2).present == 1);
    assert(pointer_gesture_recognition_matcher_match(matcher, down_right, 2).value == 0);
    assert(pointer_gesture_recognition_matcher_remove(matcher, down_right, 2).present);
    assert(!pointer_gesture_recognition_matcher_remove(matcher, NULL, 0).present);
    assert(!pointer_gesture_recognition_matcher_remove(matcher, &bad, 1).present);
    pointer_gesture_recognition_matcher_clear(matcher);
    pointer_gesture_recognition_matcher_destroy(matcher);
    assert(pointer_gesture_recognition_matcher_match(captured, down_right, 2).value == UINT64_MAX);
    pointer_gesture_recognition_matcher_destroy(captured);
    engine = pointer_gesture_recognition_engine_create(config());
    pointer_gesture_recognition_frame_destroy(step(engine, 6, 0, 0));
    pointer_gesture_recognition_frame_destroy(step(engine, 1, 0, 0));
    /* Invalid protocol input is rejected without advancing time or mutating input. */
    assert(step(engine, 99, NAN, NAN) == NULL);
    PointerGestureRecognitionFrame *snapshot = step(engine, 9, NAN, NAN);
    assert(pointer_gesture_recognition_frame_view(snapshot).stage == 1);
    pointer_gesture_recognition_frame_destroy(snapshot);
    /* Unused coordinates cannot suppress a control event's input recovery. */
    PointerGestureRecognitionFrame *replaced =
        step(engine, POINTER_GESTURE_RECOGNITION_PREPARE_REPLACEMENT_PRESS, NAN, NAN);
    assert(pointer_gesture_recognition_frame_view(replaced).disposition == 2);
    assert(pointer_gesture_recognition_frame_session_report(replaced).outcome == 8);
    assert(pointer_gesture_recognition_frame_effect(replaced, 0).kind == 1);
    pointer_gesture_recognition_frame_destroy(replaced);
    PointerGestureRecognitionConfig tuning = config();
    assert(pointer_gesture_recognition_engine_update_tuning(engine, tuning, 10, 0) == 0);
    tuning.button = 0;
    assert(pointer_gesture_recognition_engine_update_tuning(engine, tuning, 10, 0) == 2);
    tuning = config();
    tuning.change_distance = 30;
    assert(pointer_gesture_recognition_engine_update_tuning(engine, tuning, 10, 0) == 1);
    pointer_gesture_recognition_frame_destroy(stop_and_destroy(engine, 10));
    pointer_gesture_recognition_engine_destroy(NULL);
    pointer_gesture_recognition_matcher_destroy(NULL);
    pointer_gesture_recognition_frame_destroy(NULL);
    puts("{\"ownedFramesSurviveEngineDestruction\":true,\"matcherSnapshotAndInputCopy\":true,"
         "\"orderedEffects\":true,\"stopReplay\":true}");
}
