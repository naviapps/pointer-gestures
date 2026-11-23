import CPointerGestureRecognition

/// A platform-neutral request to replay consumed pointer-button input.
public enum GestureReplayRequest: Equatable, Sendable {
  /// A request to replay a full pointer-button click at the provided location.
  case click(button: PointerButton, at: GesturePoint)
  /// A request to replay consumed pointer-button locations as a drag sequence.
  ///
  /// Platform adapters decide how to map the ordered locations, including empty and single-point
  /// requests, to concrete events.
  case drag(button: PointerButton, points: [GesturePoint])
  /// A request to replay the beginning of a consumed pointer-button drag without a release.
  ///
  /// Use this when recognition stops before the physical button is released. Platform adapters
  /// should replay the button-down and consumed drag movement, then allow later real events through.
  case dragStart(button: PointerButton, points: [GesturePoint])
  /// A request to replay only the pointer-button release at the provided location.
  case release(button: PointerButton, at: GesturePoint)
}

/// Shared replay sampling used by native adapters in this package.
package func coalescedReplayDragPoints(_ points: [GesturePoint], maximumPointCount: Int)
  -> [GesturePoint]
{
  let input = points.map { PointerGestureRecognitionPoint(x: $0.x, y: $0.y) }
  let maximum = max(0, maximumPointCount)
  let count = pointer_gesture_recognition_replay_points(nil, input.count, maximum, nil, 0)
  guard count > 0 else { return [] }
  var output = [PointerGestureRecognitionPoint](repeating: .init(x: 0, y: 0), count: count)
  input.withUnsafeBufferPointer { source in
    output.withUnsafeMutableBufferPointer { destination in
      _ = pointer_gesture_recognition_replay_points(
        source.baseAddress, source.count, maximum, destination.baseAddress, destination.count)
    }
  }
  return output.map { GesturePoint(x: $0.x, y: $0.y) }
}
