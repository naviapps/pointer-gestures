#include "pointer_gesture_rendering.h"
#include "../../../test-support/c/layout.h"
#ifdef _WIN32
#include "windows/pointer_gesture_rendering_windows.h"
#endif

int main(void) {
    CHECK_SIZE(PointerGestureRenderingPoint, 16);
    CHECK_LAYOUT(PointerGestureRenderingCommand, 56, 8);
    CHECK_LAYOUT(PointerGestureRenderingOverlayConfiguration, 16, 8);
    CHECK_OFFSET(PointerGestureRenderingOverlayConfiguration, maximum_point_lag, 8);
    CHECK_LAYOUT(PointerGestureRenderingTraceView, 56, 8);
    CHECK_OFFSET(PointerGestureRenderingTraceView, points, 16);
    CHECK_OFFSET(PointerGestureRenderingTraceView, tail, 32);
    CHECK_OFFSET(PointerGestureRenderingTraceView, tail_present, 48);
    CHECK_LAYOUT(PointerGestureRenderingTraceConversion, 16, 8);
    CHECK_OFFSET(PointerGestureRenderingTraceConversion, reused_geometry, 8);
    CHECK_LAYOUT(PointerGestureRenderingOverlayView, 24, 8);
    CHECK_OFFSET(PointerGestureRenderingOverlayView, deadline_ns, 16);

#ifdef _WIN32
    CHECK_LAYOUT(PointerGestureRenderingWindowsOverlayConfiguration, 40, 8);
    CHECK_OFFSET(PointerGestureRenderingWindowsOverlayConfiguration, line_width, 8);
    CHECK_OFFSET(PointerGestureRenderingWindowsOverlayConfiguration, timing, 24);
    CHECK_LAYOUT(PointerGestureRenderingWindowsOverlayError, 8, 4);
    CHECK_OFFSET(PointerGestureRenderingWindowsOverlayError, system, 4);
    CHECK_LAYOUT(PointerGestureRenderingWindowsOverlayView, 48, 8);
    CHECK_OFFSET(PointerGestureRenderingWindowsOverlayView, bounds, 16);
#endif
    return 0;
}
