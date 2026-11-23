#include <CPointerGestureRecognition/pointer_gesture_recognition.h>
#include <assert.h>
#include <stdio.h>

int main(void) {
    _Static_assert(POINTER_GESTURE_RECOGNITION_ABI_VERSION == 8U,
                   "The installed recognition header must declare ABI 8");
    assert(pointer_gesture_recognition_abi_version() == POINTER_GESTURE_RECOGNITION_ABI_VERSION);

    enum { UP = 1, DOWN = 2, RIGHT = 4 };
    uint32_t supplied[] = {RIGHT, DOWN};
    const uint32_t exact[] = {RIGHT, DOWN};
    const size_t count = sizeof exact / sizeof exact[0];

    PointerGestureRecognitionMatcher *matcher = pointer_gesture_recognition_matcher_create();
    assert(matcher != NULL);
    const PointerGestureRecognitionPatternValidation registered =
        pointer_gesture_recognition_matcher_register(matcher, supplied, count, 0);
    assert(registered.code == 0 && registered.index == UINT32_MAX);
    supplied[1] = UP;

    PointerGestureRecognitionMatcher *snapshot = pointer_gesture_recognition_matcher_clone(matcher);
    assert(snapshot != NULL);
    pointer_gesture_recognition_matcher_destroy(matcher);

    const PointerGestureRecognitionLookup matched =
        pointer_gesture_recognition_matcher_match(snapshot, exact, count);
    const PointerGestureRecognitionLookup prefix =
        pointer_gesture_recognition_matcher_match(snapshot, exact, count - 1);
    const PointerGestureRecognitionLookup changed =
        pointer_gesture_recognition_matcher_match(snapshot, supplied, count);
    pointer_gesture_recognition_matcher_destroy(snapshot);

    assert(matched.present == 1 && matched.value == 0);
    assert(prefix.present == 0 && changed.present == 0);
    puts("Standalone installed C PointerGestureRecognition consumer passed exact matcher "
         "ownership.");
    return 0;
}
