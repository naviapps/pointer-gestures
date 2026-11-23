import CoreGraphics
import PointerGestureRendering

/// Mapping from global display-space gesture points into an overlay canvas.
public struct GestureOverlayCoordinateMapping: Equatable, Sendable {
  private let windowOrigin: CGPoint

  private let displayCoordinateMaxY: CGFloat

  /// Creates a mapping for an overlay window in the host's display coordinate space.
  ///
  /// `displayCoordinateMaxY` is the vertical origin used by the top-left-based
  /// gesture points. Non-finite values are replaced with zero.
  public init(
    windowOrigin: CGPoint,
    displayCoordinateMaxY: CGFloat
  ) {
    self.windowOrigin = CGPoint(
      x: RenderingInputNormalizer.finiteOrZero(windowOrigin.x),
      y: RenderingInputNormalizer.finiteOrZero(windowOrigin.y)
    )
    self.displayCoordinateMaxY = RenderingInputNormalizer.finiteOrZero(displayCoordinateMaxY)
  }

  func traceViewport(canvasSize: CGSize) -> CGRect {
    CGRect(
      x: windowOrigin.x, y: displayCoordinateMaxY - (windowOrigin.y + canvasSize.height),
      width: canvasSize.width, height: canvasSize.height)
  }

  func convert(
    _ point: CGPoint,
    canvasSize: CGSize
  ) -> CGPoint {
    let overlayWindowTopY =
      displayCoordinateMaxY
      - (windowOrigin.y + canvasSize.height)

    return CGPoint(
      x: point.x - windowOrigin.x,
      y: point.y - overlayWindowTopY
    )
  }
}
