/// Owned preview geometry whose command values outlive the native planner result.
public struct GesturePatternGeometry: Equatable, Sendable {
  /// Stroke path, capped before an arrowhead's tip when a head fits.
  public let path: GesturePath
  /// Closed path to fill for the arrowhead, or `nil` when disabled or too small to fit.
  public let arrowhead: GesturePath?
}
