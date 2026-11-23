#include "pointer_gesture_rendering.h"
#ifdef _WIN32
#include "windows/pointer_gesture_rendering_windows.h"
_Static_assert(sizeof(PointerGestureRenderingWindowsOverlayConfiguration) == 40,
               "Windows configuration layout");
_Static_assert(sizeof(PointerGestureRenderingWindowsOverlayError) == 8, "Windows error layout");
_Static_assert(sizeof(PointerGestureRenderingWindowsOverlayView) == 48, "Windows view layout");
#endif
_Static_assert(sizeof(PointerGestureRenderingPoint) == 16, "C ABI layout");
_Static_assert(sizeof(PointerGestureRenderingCommand) == 56, "C ABI layout");
_Static_assert(sizeof(PointerGestureRenderingOverlayConfiguration) == 16, "C ABI layout");
_Static_assert(sizeof(PointerGestureRenderingTraceView) == 56, "C ABI layout");
_Static_assert(sizeof(PointerGestureRenderingTraceConversion) == 16, "C ABI layout");
_Static_assert(sizeof(PointerGestureRenderingOverlayView) == 24, "C ABI layout");
