import CPointerGestureRecognition

/// Rust owns startup policy; Swift owns tasks and event-source resources.
@MainActor
final class RustStartup {
  enum Request: UInt32 {
    case start = 1
    case stop, retryNow, scheduledRetry, succeeded, failed
  }

  private let handle: OpaquePointer

  init(delays: [Double]) {
    handle = pointer_gesture_recognition_startup_create()!
    precondition(setDelays(delays) != 0)
  }

  isolated deinit { pointer_gesture_recognition_startup_destroy(handle) }

  var view: PointerGestureRecognitionStartupView {
    pointer_gesture_recognition_startup_view(handle)
  }

  var lifecycle: GestureRecognizerState.Status.Lifecycle {
    switch view.lifecycle {
    case 0: .idle
    case 1: .starting
    case 2: .ready
    case 3: .retrying
    case 4: .failed
    default: preconditionFailure("Unknown startup lifecycle")
    }
  }

  func request(_ request: Request, revision: UInt64) -> Bool {
    pointer_gesture_recognition_startup_request(handle, request.rawValue, revision) == 1
  }

  /// 0 is invalid, 1 preserves current task work, 2 replaces pending work.
  func setDelays(_ delays: [Double]) -> UInt32 {
    delays.withUnsafeBufferPointer {
      pointer_gesture_recognition_startup_set_delays(handle, $0.baseAddress, $0.count)
    }
  }

  func nextRetry() -> (revision: UInt64, delay: UInt64)? {
    var delay: UInt64 = 0
    guard pointer_gesture_recognition_startup_next_delay(handle, &delay) == 1 else { return nil }
    return (view.revision, delay)
  }
}
