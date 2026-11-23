#ifndef POINTER_GESTURE_RENDERING_H
#define POINTER_GESTURE_RENDERING_H
#include <stddef.h>
#define POINTER_GESTURE_RENDERING_ABI_VERSION 8U
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

/* Input buffers are borrowed only during a call. Non-null buffers must be
   readable for their counts. No callbacks, OS operations or input capture occur.
   Directions use recognition's codes: up=1, down=2, left=3, right=4. */
typedef struct {
    double x, y;
} PointerGestureRenderingPoint;
/* kind: move=1, line=2, cubic=3, close=4. Unused points are zero. */
typedef struct {
    uint32_t kind;
    PointerGestureRenderingPoint point, control1, control2;
} PointerGestureRenderingCommand;
typedef struct PointerGestureRenderingGeometry PointerGestureRenderingGeometry;
/* code: success=0, empty=1, point-count=2, non-finite=3, buffer=4, direction=5.
   An error always returns a null geometry. Each success owns one geometry. */
typedef struct {
    uint32_t code;
    PointerGestureRenderingGeometry *geometry;
} PointerGestureRenderingResult;
typedef struct {
    const PointerGestureRenderingCommand *path;
    size_t path_count;
    const PointerGestureRenderingCommand *arrowhead;
    size_t arrowhead_count;
} PointerGestureRenderingGeometryView;
typedef struct {
    double min_x, min_y, max_x, max_y;
} PointerGestureRenderingBounds;
typedef struct {
    uint32_t valid;
    PointerGestureRenderingBounds bounds;
} PointerGestureRenderingBoundsResult;

uint32_t pointer_gesture_rendering_abi_version(void);
/* Shared elapsed-component conversion: clamp negative values, truncate fractions, saturate
 * overflow. */
uint64_t pointer_gesture_rendering_elapsed_nanoseconds(int64_t seconds, int64_t attoseconds);
PointerGestureRenderingResult
pointer_gesture_rendering_live_trace(const uint32_t *directions, size_t count,
                                     const PointerGestureRenderingPoint *points, size_t point_count,
                                     double radius_scale, PointerGestureRenderingPoint tail,
                                     uint32_t tail_present);
PointerGestureRenderingResult
pointer_gesture_rendering_pattern(const uint32_t *directions, size_t count, double width,
                                  double height, double padding, double line_width,
                                  uint32_t shows_arrowhead, double corner_radius_scale);
/* Views are immutable and valid until geometry_destroy. Readers may share a
   geometry; destruction must not overlap a reader. Copy before host callbacks. */
PointerGestureRenderingGeometryView
pointer_gesture_rendering_geometry_view(const PointerGestureRenderingGeometry *geometry);
void pointer_gesture_rendering_geometry_destroy(PointerGestureRenderingGeometry *geometry);
PointerGestureRenderingBoundsResult
pointer_gesture_rendering_bounds(const PointerGestureRenderingPoint *points, size_t count);
uint32_t pointer_gesture_rendering_intersects(PointerGestureRenderingBounds bounds,
                                              PointerGestureRenderingPoint tail,
                                              uint32_t tail_present,
                                              PointerGestureRenderingBounds viewport,
                                              double line_width);

/* Immutable traces own copied coordinates, directions and cached bounds. Clone
   shares Rust data. Views borrow their trace; destroy each owned handle once. */
typedef struct PointerGestureRenderingTrace PointerGestureRenderingTrace;
typedef struct {
    const uint32_t *directions;
    size_t count;
    const PointerGestureRenderingPoint *points;
    size_t point_count;
    PointerGestureRenderingPoint tail;
    uint32_t tail_present;
} PointerGestureRenderingTraceView;
typedef struct {
    PointerGestureRenderingTrace *trace;
    uint32_t reused_geometry;
} PointerGestureRenderingTraceConversion;
/* Convert a recognition snapshot without retaining raw history. Hidden or invalid
   input returns a null handle and zero reuse. A non-null result is independently
   owned. Reuse means directions and points share the previous trace's storage. */
PointerGestureRenderingTraceConversion pointer_gesture_rendering_trace_from_snapshot(
    const PointerGestureRenderingTrace *previous, uint32_t visible, const uint32_t *directions,
    size_t count, const PointerGestureRenderingPoint *points, size_t point_count,
    PointerGestureRenderingPoint tail, uint32_t tail_present);
PointerGestureRenderingTrace *pointer_gesture_rendering_trace_create(
    const uint32_t *directions, size_t count, const PointerGestureRenderingPoint *points,
    size_t point_count, PointerGestureRenderingPoint tail, uint32_t tail_present);
/* Independently owned immutable tail replacement sharing coordinates/bounds.
   Invalid/non-finite tail or tail_present outside0/1 returns NULL. */
PointerGestureRenderingTrace *
pointer_gesture_rendering_trace_with_tail(const PointerGestureRenderingTrace *trace,
                                          PointerGestureRenderingPoint tail, uint32_t tail_present);
PointerGestureRenderingTrace *
pointer_gesture_rendering_trace_clone(const PointerGestureRenderingTrace *trace);
PointerGestureRenderingTraceView
pointer_gesture_rendering_trace_view(const PointerGestureRenderingTrace *trace);
void pointer_gesture_rendering_trace_destroy(PointerGestureRenderingTrace *trace);
uint32_t pointer_gesture_rendering_trace_intersects(const PointerGestureRenderingTrace *trace,
                                                    PointerGestureRenderingBounds viewport,
                                                    double line_width);
typedef struct {
    double interval_seconds;
    double maximum_point_lag;
} PointerGestureRenderingOverlayConfiguration;
PointerGestureRenderingOverlayConfiguration
pointer_gesture_rendering_overlay_configuration(double interval_seconds, double maximum_point_lag);
double pointer_gesture_rendering_finite_or_zero(double value);
double pointer_gesture_rendering_non_negative(double value);
/* Configuration normalizes invalid intervals to 1/60 second and invalid lag to
   24 coordinate units; zero lag is valid. Creation normalizes the interval from
   interval_seconds. Boolean outputs use 0/1 and null traces mean hidden.
   Serial reducer: host supplies monotonic nanoseconds, schedules ticks, publishes
   changed frames. Clock regressions are clamped.
   Update/tick return owned frames independent of later calls and destruction.
   The frame's trace is borrowed; clone it before destroying the frame. */
typedef struct PointerGestureRenderingOverlay PointerGestureRenderingOverlay;
typedef struct PointerGestureRenderingOverlayFrame PointerGestureRenderingOverlayFrame;
typedef struct {
    const PointerGestureRenderingTrace *trace;
    uint32_t changed, deadline_present;
    uint64_t deadline_ns;
} PointerGestureRenderingOverlayView;
PointerGestureRenderingOverlay *
pointer_gesture_rendering_overlay_create(PointerGestureRenderingOverlayConfiguration configuration,
                                         const PointerGestureRenderingTrace *trace,
                                         uint64_t now_ns);
PointerGestureRenderingOverlayFrame *
pointer_gesture_rendering_overlay_update(PointerGestureRenderingOverlay *overlay,
                                         const PointerGestureRenderingTrace *trace,
                                         uint64_t now_ns);
/* Immediate cancellation discards visible/pending geometry and all deadlines. */
PointerGestureRenderingOverlayFrame *
pointer_gesture_rendering_overlay_clear(PointerGestureRenderingOverlay *overlay, uint64_t now_ns);
PointerGestureRenderingOverlayFrame *
pointer_gesture_rendering_overlay_tick(PointerGestureRenderingOverlay *overlay, uint64_t now_ns);
/* Recompute pending cadence from the last actual presentation, preserving trace
   and pending final hiding. No clock/window is created; returned frame is owned. */
PointerGestureRenderingOverlayFrame *pointer_gesture_rendering_overlay_configure(
    PointerGestureRenderingOverlay *overlay,
    PointerGestureRenderingOverlayConfiguration configuration, uint64_t now_ns);
void pointer_gesture_rendering_overlay_destroy(PointerGestureRenderingOverlay *overlay);
PointerGestureRenderingOverlayView
pointer_gesture_rendering_overlay_frame_view(const PointerGestureRenderingOverlayFrame *frame);
void pointer_gesture_rendering_overlay_frame_destroy(PointerGestureRenderingOverlayFrame *frame);

#ifdef __cplusplus
}
#endif
#endif
