#include "pointer_gesture_rendering.h"
#ifdef _WIN32
#include "windows/pointer_gesture_rendering_windows.h"
#endif
#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdio.h>

/* The caller is ASan-instrumented; the prebuilt Rust libraries are not. */
_Static_assert(sizeof(PointerGestureRenderingPoint) == 16, "point layout");
_Static_assert(offsetof(PointerGestureRenderingCommand, point) == 8, "command point offset");
_Static_assert(sizeof(PointerGestureRenderingCommand) == 56, "command layout");

int main(void) {
    assert(pointer_gesture_rendering_abi_version() == POINTER_GESTURE_RENDERING_ABI_VERSION);
#ifdef _WIN32
    {
        assert(pointer_gesture_rendering_windows_abi_version() ==
               POINTER_GESTURE_RENDERING_WINDOWS_ABI_VERSION);
        PointerGestureRenderingWindowsOverlayError overlay_error = {0};
        PointerGestureRenderingWindowsOverlayConfiguration configuration = {
            .rgba = 0xF01E32FF,
            .line_width = 4,
            .radius_scale = 1,
            .timing = pointer_gesture_rendering_overlay_configuration(1.0 / 60, 24)};
        PointerGestureRenderingWindowsOverlay *overlay =
            pointer_gesture_rendering_windows_create(configuration, &overlay_error);
        assert(overlay != NULL && overlay_error.kind == 0);
        configuration.line_width = 8;
        assert(
            pointer_gesture_rendering_windows_configure(overlay, configuration, &overlay_error) &&
            overlay_error.kind == 0);
        configuration.line_width = NAN;
        assert(
            !pointer_gesture_rendering_windows_configure(overlay, configuration, &overlay_error) &&
            overlay_error.kind == 1);
        PointerGestureRenderingWindowsOverlayView overlay_view = {0};
        assert(!pointer_gesture_rendering_windows_view(overlay, &overlay_view, &overlay_error) &&
               overlay_error.kind == 2);
        assert(pointer_gesture_rendering_windows_deactivate(overlay, &overlay_error) &&
               overlay_error.kind == 0);
        pointer_gesture_rendering_windows_destroy(overlay);
        pointer_gesture_rendering_windows_destroy(NULL);
        configuration.line_width = NAN;
        assert(pointer_gesture_rendering_windows_create(configuration, &overlay_error) == NULL &&
               overlay_error.kind == 1);
    }
#endif
    uint32_t directions[] = {4, 2};
    PointerGestureRenderingPoint points[] = {{0, 0}, {40, 0}, {40, 40}};
    PointerGestureRenderingPoint tail = {50, 45};
    PointerGestureRenderingResult live =
        pointer_gesture_rendering_live_trace(directions, 2, points, 3, 1, tail, 1);
    assert(live.code == 0 && live.geometry != NULL);
    PointerGestureRenderingGeometryView view =
        pointer_gesture_rendering_geometry_view(live.geometry);
    assert(view.path_count >= 4 && view.arrowhead_count == 0);
    const uint32_t projected_direction = 4;
    PointerGestureRenderingPoint projected_points[] = {{0, -DBL_MAX}, {40, DBL_MAX}};
    PointerGestureRenderingResult projected = pointer_gesture_rendering_live_trace(
        &projected_direction, 1, projected_points, 2, 1, (PointerGestureRenderingPoint){0}, 0);
    assert(projected.code == 0 && projected.geometry != NULL);
    PointerGestureRenderingGeometryView projected_view =
        pointer_gesture_rendering_geometry_view(projected.geometry);
    assert(projected_view.path_count == 2 && projected_view.path[1].kind == 2 &&
           projected_view.path[1].point.x == 40 && projected_view.path[1].point.y == -DBL_MAX);
    pointer_gesture_rendering_geometry_destroy(projected.geometry);
    directions[0] = 0;
    points[0].x = NAN;
    PointerGestureRenderingResult invalid =
        pointer_gesture_rendering_pattern(directions, 2, 96, 64, 8, 3.0, 1, 1.0);
    assert(invalid.code == 5 && invalid.geometry == NULL);
    invalid = pointer_gesture_rendering_pattern(NULL, 1, 96, 64, 8, 3.0, 1, 1.0);
    assert(invalid.code == 4 && invalid.geometry == NULL);
    invalid = pointer_gesture_rendering_pattern(NULL, 0, 96, 64, 8, 3.0, 1, 1.0);
    assert(invalid.code == 1 && invalid.geometry == NULL);
    for (int i = 0; i < 100; ++i) {
        uint32_t pattern[] = {1, 4, 2, 3};
        PointerGestureRenderingResult other =
            pointer_gesture_rendering_pattern(pattern, 4, 96, 64, 8, 3.0, 1, 1.0);
        assert(other.code == 0);
        PointerGestureRenderingGeometryView other_view =
            pointer_gesture_rendering_geometry_view(other.geometry);
        assert(other_view.arrowhead_count == 4 && other_view.arrowhead[3].kind == 4);
        pointer_gesture_rendering_geometry_destroy(other.geometry);
    }
    /* Retained views remain owned by their original geometry after all input
       arrays change and unrelated allocations are created and destroyed. */
    assert(view.path[0].point.x == 0 && view.path[0].point.y == 0);
    assert(view.path[view.path_count - 1].point.x == tail.x);
    assert(view.path[view.path_count - 1].point.y == tail.y);
    pointer_gesture_rendering_geometry_destroy(live.geometry);
    pointer_gesture_rendering_geometry_destroy(NULL);
    uint32_t right[] = {4};
    PointerGestureRenderingPoint line[] = {{0, 0}, {1, 0}};
    PointerGestureRenderingTrace *trace =
        pointer_gesture_rendering_trace_create(right, 1, line, 2, tail, 0);
    assert(trace != NULL);
    PointerGestureRenderingOverlay *overlay = pointer_gesture_rendering_overlay_create(
        pointer_gesture_rendering_overlay_configuration(0.01, 24), NULL, 0);
    PointerGestureRenderingOverlayFrame *frame =
        pointer_gesture_rendering_overlay_update(overlay, trace, 0);
    PointerGestureRenderingOverlayView snapshot =
        pointer_gesture_rendering_overlay_frame_view(frame);
    assert(snapshot.changed == 1 && snapshot.trace != NULL);
    PointerGestureRenderingTrace *retained = pointer_gesture_rendering_trace_clone(snapshot.trace);
    pointer_gesture_rendering_overlay_frame_destroy(
        pointer_gesture_rendering_overlay_update(overlay, NULL, 1));
    PointerGestureRenderingOverlayFrame *configured = pointer_gesture_rendering_overlay_configure(
        overlay, pointer_gesture_rendering_overlay_configuration(0.02, 24), 2);
    PointerGestureRenderingOverlayView changed =
        pointer_gesture_rendering_overlay_frame_view(configured);
    assert(!changed.changed && changed.trace != NULL && changed.deadline_present == 1);
    pointer_gesture_rendering_overlay_frame_destroy(configured);
    assert(pointer_gesture_rendering_overlay_configure(
               NULL, pointer_gesture_rendering_overlay_configuration(0.01, 24), 0) == NULL);
    PointerGestureRenderingOverlayFrame *cleared =
        pointer_gesture_rendering_overlay_clear(overlay, 3);
    PointerGestureRenderingOverlayView cleared_view =
        pointer_gesture_rendering_overlay_frame_view(cleared);
    assert(cleared_view.changed && cleared_view.trace == NULL && !cleared_view.deadline_present);
    pointer_gesture_rendering_overlay_frame_destroy(cleared);
    pointer_gesture_rendering_overlay_frame_destroy(
        pointer_gesture_rendering_overlay_tick(overlay, UINT64_MAX));
    pointer_gesture_rendering_trace_destroy(trace);
    pointer_gesture_rendering_overlay_frame_destroy(frame);
    pointer_gesture_rendering_overlay_destroy(overlay);
    assert(pointer_gesture_rendering_trace_view(retained).point_count == 2);
    assert(pointer_gesture_rendering_trace_intersects(
               retained, (PointerGestureRenderingBounds){0, 0, 1, 1}, 0) == 1);
    PointerGestureRenderingTrace *updated = pointer_gesture_rendering_trace_with_tail(
        retained, (PointerGestureRenderingPoint){200, 20}, 1);
    assert(updated != NULL);
    assert(pointer_gesture_rendering_trace_view(updated).points ==
           pointer_gesture_rendering_trace_view(retained).points);
    assert(pointer_gesture_rendering_trace_with_tail(
               retained, (PointerGestureRenderingPoint){NAN, 0}, 1) == NULL);
    assert(pointer_gesture_rendering_trace_with_tail(retained, tail, 2) == NULL);
    PointerGestureRenderingTraceConversion converted =
        pointer_gesture_rendering_trace_from_snapshot(retained, 1, right, 1, line, 2, tail, 1);
    assert(converted.trace != NULL && converted.reused_geometry == 1);
    assert(pointer_gesture_rendering_trace_view(converted.trace).points ==
           pointer_gesture_rendering_trace_view(retained).points);
    right[0] = 3;
    line[1].x = 12;
    PointerGestureRenderingTraceView reused = pointer_gesture_rendering_trace_view(converted.trace);
    assert(reused.directions[0] == 4 && reused.points[1].x == 1);
    PointerGestureRenderingTraceConversion hidden =
        pointer_gesture_rendering_trace_from_snapshot(retained, 0, NULL, 1, NULL, 2, tail, 1);
    assert(hidden.trace == NULL && hidden.reused_geometry == 0);
    PointerGestureRenderingTraceConversion malformed =
        pointer_gesture_rendering_trace_from_snapshot(retained, 1, NULL, 1, line, 2, tail, 1);
    assert(malformed.trace == NULL && malformed.reused_geometry == 0);
    pointer_gesture_rendering_trace_destroy(retained);
    assert(pointer_gesture_rendering_trace_view(converted.trace).tail.x == tail.x);
    pointer_gesture_rendering_trace_destroy(converted.trace);
    assert(pointer_gesture_rendering_trace_view(updated).point_count == 2);
    assert(pointer_gesture_rendering_trace_view(updated).tail.x == 200);
    pointer_gesture_rendering_trace_destroy(updated);
    puts("Native geometry and overlay buffers, errors and lifetimes passed.");
}
