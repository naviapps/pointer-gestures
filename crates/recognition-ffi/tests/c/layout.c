#include "pointer_gesture_recognition.h"
#ifdef _WIN32
#include "windows/pointer_gesture_recognition_windows.h"
_Static_assert(sizeof(PointerGestureRecognitionWindowsError) == 16, "Windows error ABI layout");
_Static_assert(sizeof(PointerGestureRecognitionWindowsPressContext) == 24,
               "Windows press context ABI layout");
_Static_assert(sizeof(PointerGestureRecognitionWindowsEpoch) == 16, "Windows epoch ABI layout");
#endif
_Static_assert(sizeof(PointerGestureRecognitionStartupView) == 16, "Startup ABI layout");
_Static_assert(sizeof(PointerGestureRecognitionPoint) == 16, "C ABI layout");
_Static_assert(sizeof(PointerGestureRecognitionConfig) == 56, "C ABI layout");
_Static_assert(sizeof(PointerGestureRecognitionSessionReportView) == 80,
               "Session report ABI layout");
_Static_assert(sizeof(PointerGestureRecognitionFrameView) == 160, "C ABI layout");
_Static_assert(sizeof(PointerGestureRecognitionEffectView) == 144, "C ABI layout");

_Static_assert(sizeof(PointerGestureRecognitionPatternValidation) == 8,
               "Pattern validation layout");
