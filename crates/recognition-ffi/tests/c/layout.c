#include "pointer_gesture_recognition.h"
#include "../../../test-support/c/layout.h"
#ifdef _WIN32
#include "windows/pointer_gesture_recognition_windows.h"
#endif

int main(void) {
    CHECK_SIZE(PointerGestureRecognitionStartupView, 16);
    CHECK_SIZE(PointerGestureRecognitionPatternValidation, 8);
    CHECK_LAYOUT(PointerGestureRecognitionPoint, 16, 8);
    CHECK_LAYOUT(PointerGestureRecognitionConfig, 56, 8);
    CHECK_OFFSET(PointerGestureRecognitionConfig, maximum_points, 32);
    CHECK_OFFSET(PointerGestureRecognitionConfig, pass_empty, 44);
    CHECK_OFFSET(PointerGestureRecognitionConfig, maximum_directions, 48);
    CHECK_LAYOUT(PointerGestureRecognitionFrameView, 160, 8);
    CHECK_OFFSET(PointerGestureRecognitionFrameView, input_generation, 32);
    CHECK_OFFSET(PointerGestureRecognitionFrameView, directions, 48);
    CHECK_OFFSET(PointerGestureRecognitionFrameView, tail, 120);
    CHECK_OFFSET(PointerGestureRecognitionFrameView, deadline_ns, 152);
    CHECK_LAYOUT(PointerGestureRecognitionEffectView, 144, 8);
    CHECK_OFFSET(PointerGestureRecognitionEffectView, trace, 32);
    CHECK_OFFSET(PointerGestureRecognitionEffectView, ready, 112);
    CHECK_OFFSET(PointerGestureRecognitionEffectView, recorded, 128);
    CHECK_LAYOUT(PointerGestureRecognitionSessionReportView, 80, 8);
    CHECK_OFFSET(PointerGestureRecognitionSessionReportView, generation, 24);
    CHECK_OFFSET(PointerGestureRecognitionSessionReportView, directions, 64);

#ifdef _WIN32
    CHECK_LAYOUT(PointerGestureRecognitionWindowsError, 16, 4);
    CHECK_OFFSET(PointerGestureRecognitionWindowsError, expected, 12);
    CHECK_LAYOUT(PointerGestureRecognitionWindowsPressContext, 24, 8);
    CHECK_OFFSET(PointerGestureRecognitionWindowsPressContext, foreground_process_id, 20);
    CHECK_LAYOUT(PointerGestureRecognitionWindowsEpoch, 16, 8);
    CHECK_OFFSET(PointerGestureRecognitionWindowsEpoch, present, 8);
#endif
    return 0;
}
