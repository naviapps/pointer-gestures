import CoreGraphics
import PointerGestureRendering

/// Turn rendering style used by gesture paths.
public struct GestureTurnStyle: Equatable, Sendable {
  /// Rounded gesture turns using the standard corner radius scale.
  public static let rounded = GestureTurnStyle(cornerRadiusScale: 1.0)

  /// Sharp right-angle gesture turns.
  public static let rightAngle = GestureTurnStyle(cornerRadiusScale: 0)

  /// The rounded-corner radius multiplier used when drawing turns.
  ///
  /// A value of `1` uses the standard rounded turn radius, smaller positive
  /// values tighten the rounded corner, values greater than `1` increase the
  /// rounded radius, and `0` draws sharp right-angle turns. Non-finite or
  /// negative values are clamped to zero.
  public let cornerRadiusScale: CGFloat

  /// Creates a gesture turn style.
  ///
  /// Pass `1` for the standard rounded radius, a smaller positive scale for
  /// tighter rounded corners, a larger scale for rounder corners, or `0` for
  /// sharp right-angle turns. Non-finite or negative corner radius scales are
  /// clamped to zero.
  public init(cornerRadiusScale: CGFloat) {
    self.cornerRadiusScale =
      RenderingInputNormalizer
      .nonNegativeFiniteOrZero(cornerRadiusScale)
  }
}

extension GestureTurnStyle {
  var lineJoin: CGLineJoin {
    cornerRadiusScale == 0 ? .miter : .round
  }
}
