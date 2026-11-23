import CPointerGestureRendering
import CoreGraphics
import Foundation

/// Portable cadence settings normalized by Rust. Styling belongs to the renderer.
public struct GestureOverlayTiming: Equatable, Sendable {
  let native: PointerGestureRenderingOverlayConfiguration
  /// Animation interval in seconds after normalization, defaulting to 1/60 second.
  public var minimumAnimationInterval: TimeInterval { native.interval_seconds }
  /// Distance in trace coordinate units allowing a rate-limited immediate update.
  public var maximumPointLagDistance: CGFloat { native.maximum_point_lag }

  /// Creates cadence settings using the shared Rust normalization rules.
  ///
  /// Non-positive, non-finite or unrepresentable intervals become 1/60 second.
  /// Negative or non-finite lag distances become 24 coordinate units; zero is valid.
  public init(
    minimumAnimationInterval: TimeInterval = 1.0 / 60.0, maximumPointLagDistance: CGFloat = 24
  ) {
    precondition(
      pointer_gesture_rendering_abi_version() == POINTER_GESTURE_RENDERING_ABI_VERSION,
      "Unsupported native rendering ABI")
    native = pointer_gesture_rendering_overlay_configuration(
      minimumAnimationInterval, maximumPointLagDistance)
  }
  package static func elapsedNanoseconds(seconds: Int64, attoseconds: Int64) -> UInt64 {
    pointer_gesture_rendering_elapsed_nanoseconds(seconds, attoseconds)
  }

  public static func == (lhs: Self, rhs: Self) -> Bool {
    lhs.minimumAnimationInterval == rhs.minimumAnimationInterval
      && lhs.maximumPointLagDistance == rhs.maximumPointLagDistance
  }
}
