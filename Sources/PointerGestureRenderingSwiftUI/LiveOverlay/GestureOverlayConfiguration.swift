import CoreGraphics
import Foundation
import PointerGestureRendering

/// Configuration for live gesture overlays.
public struct GestureOverlayConfiguration: Equatable, Sendable {
  /// The default configuration used by live gesture overlays.
  public static let liveOverlay = GestureOverlayConfiguration(
    minimumAnimationInterval: 1.0 / 60.0,
    maximumPointLagDistance: 24
  )

  let timing: GestureOverlayTiming

  /// The preferred minimum animation update interval used while a trace is renderable.
  ///
  /// Renderable point and tail updates are coalesced to this cadence unless a
  /// direction change or large point movement requires a lower-latency update.
  public let minimumAnimationInterval: TimeInterval

  /// The maximum displayed distance a fast endpoint or tail update can lag
  /// behind before bypassing the animation cadence.
  public let maximumPointLagDistance: CGFloat

  /// The stroke style used for overlay paths.
  public let strokeStyle: GestureStrokeStyle

  /// The turn style used when drawing live gesture path corners.
  public let turnStyle: GestureTurnStyle

  /// Creates live gesture overlay configuration.
  ///
  /// Non-positive, non-finite or unrepresentable animation intervals are replaced with the
  /// default live overlay animation interval. Non-finite or negative point lag
  /// distances are replaced with the default live overlay point lag distance.
  public init(
    minimumAnimationInterval: TimeInterval = 1.0 / 60.0,
    maximumPointLagDistance: CGFloat = 24,
    strokeStyle: GestureStrokeStyle = .liveOverlay,
    turnStyle: GestureTurnStyle = .rounded
  ) {
    timing = GestureOverlayTiming(
      minimumAnimationInterval: minimumAnimationInterval,
      maximumPointLagDistance: maximumPointLagDistance)
    self.minimumAnimationInterval = timing.minimumAnimationInterval
    self.maximumPointLagDistance = timing.maximumPointLagDistance
    self.strokeStyle = strokeStyle
    self.turnStyle = turnStyle
  }
}
