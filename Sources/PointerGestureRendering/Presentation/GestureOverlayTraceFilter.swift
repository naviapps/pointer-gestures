import CoreGraphics

/// Filters immutable Rust traces against a viewport in their coordinate space.
public struct GestureOverlayTraceFilter: Sendable {
  private let displayCoordinateFrame: CGRect
  private let lineWidth: CGFloat
  /// Creates a viewport filter using the same display space as its traces.
  ///
  /// Stroke width uses that space's units. Negative or non-finite widths use zero padding.
  public init(displayCoordinateFrame: CGRect, lineWidth: CGFloat) {
    self.displayCoordinateFrame = displayCoordinateFrame
    self.lineWidth = lineWidth
  }
  /// Retains the original immutable trace when it can intersect the padded viewport.
  ///
  /// Hidden, definitely outside or non-finite geometry returns `nil`. Bounds are
  /// conservative and include the tail; this is not a pixel-level hit test.
  public func filteredTrace(_ trace: GestureOverlayTrace?) -> GestureOverlayTrace? {
    guard let trace, trace.intersects(viewport: displayCoordinateFrame, lineWidth: lineWidth) else {
      return nil
    }
    return trace
  }
}
