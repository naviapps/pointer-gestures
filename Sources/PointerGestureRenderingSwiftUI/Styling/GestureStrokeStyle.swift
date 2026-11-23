import PointerGestureRendering
import SwiftUI

/// Stroke attributes used by rendered gesture paths.
public struct GestureStrokeStyle: Equatable, Sendable {
  /// The default stroke style used by live gesture overlays.
  public static let liveOverlay = GestureStrokeStyle(
    color: .blue,
    lineWidth: 4.0
  )

  /// The default stroke style used by static gesture pattern previews.
  public static let patternPreview = GestureStrokeStyle(
    color: .accentColor,
    lineWidth: 3
  )

  /// The stroke color.
  public let color: Color

  /// The stroke width in user-space points, independent of backing pixel scale.
  public let lineWidth: CGFloat

  /// Creates a gesture stroke style.
  ///
  /// Non-finite or negative line widths are clamped to zero.
  public init(
    color: Color = GestureStrokeStyle.liveOverlay.color,
    lineWidth: CGFloat = GestureStrokeStyle.liveOverlay.lineWidth
  ) {
    self.color = color
    self.lineWidth = RenderingInputNormalizer.nonNegativeFiniteOrZero(
      lineWidth)
  }
}

extension GestureStrokeStyle {
  func makeStrokeStyle() -> StrokeStyle {
    makeStrokeStyle(lineJoin: .round)
  }

  func makeStrokeStyle(lineJoin: CGLineJoin) -> StrokeStyle {
    StrokeStyle(lineWidth: lineWidth, lineCap: .round, lineJoin: lineJoin)
  }
}
