#ifndef POINTER_GESTURE_RENDERING_WINDOWS_H
#define POINTER_GESTURE_RENDERING_WINDOWS_H
#define POINTER_GESTURE_RENDERING_WINDOWS_ABI_VERSION 4U
#ifdef _WIN32
#include "../pointer_gesture_rendering.h"
#ifdef __cplusplus
extern "C" {
#endif
typedef struct PointerGestureRenderingWindowsOverlay PointerGestureRenderingWindowsOverlay;
/* Straight color bytes packed 0xRRGGBBAA. Width uses physical desktop pixels.
   No resources start at creation. Portable geometry APIs remain OS-independent. */
typedef struct {
    uint32_t rgba;
    double line_width, radius_scale;
    PointerGestureRenderingOverlayConfiguration timing;
} PointerGestureRenderingWindowsOverlayConfiguration;
/* kind: none=0, invalid/excessive input=1, inactive=2, Windows error=3,
   stopped worker=4. system is meaningful for kind=3. Error pointers may be null. */
typedef struct {
    uint32_t kind, system;
} PointerGestureRenderingWindowsOverlayError;
/* Borrowed HWND invalidated by worker failure, deactivate or destroy; do not mutate or destroy it.
   Bounds are physical desktop pixels, zeroed when hidden. Visibility means OS
   submission, not compositor completion. */
typedef struct {
    uintptr_t window;
    uint32_t visible;
    PointerGestureRenderingBounds bounds;
} PointerGestureRenderingWindowsOverlayView;
uint32_t pointer_gesture_rendering_windows_abi_version(void);
PointerGestureRenderingWindowsOverlay *pointer_gesture_rendering_windows_create(
    PointerGestureRenderingWindowsOverlayConfiguration configuration,
    PointerGestureRenderingWindowsOverlayError *error);
/* Replace appearance/cadence while retaining HWND and pending trace/final hide.
   Inactive owners retain settings without creating resources. Invalid values
   leave the current configuration/window intact. */
uint32_t pointer_gesture_rendering_windows_configure(
    PointerGestureRenderingWindowsOverlay *overlay,
    PointerGestureRenderingWindowsOverlayConfiguration configuration,
    PointerGestureRenderingWindowsOverlayError *error);
/* Serialize every call on one overlay. Methods wait for the dedicated owner;
   never call from an input hook. Activate creates a hidden, nonactivating,
   click-through topmost window; no host DPI setting or permission is changed. */
uint32_t
pointer_gesture_rendering_windows_activate(PointerGestureRenderingWindowsOverlay *overlay,
                                           PointerGestureRenderingWindowsOverlayError *error);
/* Trace is copied/shared during the call; null schedules final-frame hiding.
   Maximum 4096 directions; raster is clipped to the virtual desktop and bounded
   to 16M pixels. Worker schedules shared cadence itself. Failure hides stale data. */
uint32_t
pointer_gesture_rendering_windows_update(PointerGestureRenderingWindowsOverlay *overlay,
                                         const PointerGestureRenderingTrace *trace,
                                         PointerGestureRenderingWindowsOverlayError *error);
/* Immediately discard pending trace on cancellation, independent of cadence. */
uint32_t pointer_gesture_rendering_windows_clear(PointerGestureRenderingWindowsOverlay *overlay,
                                                 PointerGestureRenderingWindowsOverlayError *error);
uint32_t pointer_gesture_rendering_windows_view(PointerGestureRenderingWindowsOverlay *overlay,
                                                PointerGestureRenderingWindowsOverlayView *view,
                                                PointerGestureRenderingWindowsOverlayError *error);
/* Repeated deactivation is harmless. Destroy also joins; use deactivate first
   when failure reporting is required. A deactivated instance can activate again. */
uint32_t
pointer_gesture_rendering_windows_deactivate(PointerGestureRenderingWindowsOverlay *overlay,
                                             PointerGestureRenderingWindowsOverlayError *error);
void pointer_gesture_rendering_windows_destroy(PointerGestureRenderingWindowsOverlay *overlay);
#ifdef __cplusplus
}
#endif
#endif
#endif
