import CPointerGestureRecognition
import Foundation

enum GestureRecognizerTiming {
  static func taskSleepNanoseconds(for interval: TimeInterval) -> UInt64 {
    precondition(
      pointer_gesture_recognition_abi_version() == POINTER_GESTURE_RECOGNITION_ABI_VERSION,
      "Unsupported native recognition ABI")
    return pointer_gesture_recognition_duration_nanoseconds(interval)
  }
}
