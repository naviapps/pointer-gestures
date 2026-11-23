import PointerGestureRecognition
import SwiftUI

/// SwiftUI rendering of a gesture direction pattern, intended for small previews.
public struct GesturePatternView: View {
  private let pattern: [GestureDirection]
  private let renderer: GesturePatternRenderer
  private let frameSize: CGSize

  /// Creates a static gesture pattern preview.
  ///
  /// Empty patterns render the configured frame without a path.
  public init(
    pattern: [GestureDirection],
    configuration: GesturePatternConfiguration = .patternPreview
  ) {
    self.pattern = pattern
    renderer = GesturePatternRenderer(configuration: configuration)
    frameSize = configuration.frameSize
  }

  /// The rendered gesture pattern preview.
  public var body: some View {
    Canvas { context, canvasSize in
      renderer.drawPattern(in: context, canvasSize: canvasSize, pattern: pattern)
    }
    .frame(width: frameSize.width, height: frameSize.height)
  }
}
