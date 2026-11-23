import CoreGraphics
import PointerGestureRendering

/// Configuration for static gesture pattern previews.
public struct GesturePatternConfiguration: Equatable, Sendable {
  /// The default configuration used by static gesture pattern previews.
  public static let patternPreview = GesturePatternConfiguration(
    frameSize: CGSize(width: 96, height: 64),
    strokeStyle: .patternPreview,
    showsArrowhead: true,
    padding: 0
  )

  /// The SwiftUI frame size used by the rendered preview.
  public let frameSize: CGSize

  /// The stroke style used for the preview path.
  public let strokeStyle: GestureStrokeStyle

  /// Corner rounding, relative to the preview canvas.
  public let turnStyle: GestureTurnStyle

  /// Whether the preview draws the package default arrowhead at the final segment.
  public let showsArrowhead: Bool

  /// The inset applied around the generated pattern path.
  public let padding: CGFloat

  /// Creates gesture pattern preview configuration.
  ///
  /// Non-finite or negative frame size dimensions and padding are clamped to zero.
  public init(
    frameSize: CGSize = GesturePatternConfiguration.patternPreview.frameSize,
    strokeStyle: GestureStrokeStyle = GesturePatternConfiguration.patternPreview.strokeStyle,
    showsArrowhead: Bool = GesturePatternConfiguration.patternPreview.showsArrowhead,
    padding: CGFloat = GesturePatternConfiguration.patternPreview.padding,
    turnStyle: GestureTurnStyle = .rounded
  ) {
    self.frameSize = CGSize(
      width: RenderingInputNormalizer.nonNegativeFiniteOrZero(
        frameSize.width),
      height: RenderingInputNormalizer.nonNegativeFiniteOrZero(
        frameSize.height)
    )
    self.strokeStyle = strokeStyle
    self.turnStyle = turnStyle
    self.showsArrowhead = showsArrowhead
    self.padding = RenderingInputNormalizer.nonNegativeFiniteOrZero(padding)
  }
}
