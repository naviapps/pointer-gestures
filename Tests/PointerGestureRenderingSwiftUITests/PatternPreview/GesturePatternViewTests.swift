import CoreGraphics
import PointerGestureRenderingSwiftUI
import XCTest

@testable import PointerGestureRendering

@MainActor
final class GesturePatternViewTests: XCTestCase {
  func testBodyRendersWithDefaultConfiguration() {
    let image = renderedImage(GesturePatternView(pattern: [.right, .down]))

    XCTAssertEqual(image?.width, Int(GesturePatternConfiguration.patternPreview.frameSize.width))
    XCTAssertEqual(image?.height, Int(GesturePatternConfiguration.patternPreview.frameSize.height))
    assertRenderedImageContainsVisiblePixel(image)
  }

  func testBodyRendersVisiblePatternWhenArrowheadIsDisabled() {
    let image = renderedImage(
      GesturePatternView(
        pattern: [.up, .right],
        configuration: .init(frameSize: CGSize(width: 120, height: 80), showsArrowhead: false)
      )
    )

    assertRenderedImageContainsVisiblePixel(image)
  }

  func testBodyUsesCustomStrokeStyleColor() {
    let image = renderedImage(
      GesturePatternView(
        pattern: [.right],
        configuration: .init(
          frameSize: CGSize(width: 80, height: 40),
          strokeStyle: GestureStrokeStyle(color: .red, lineWidth: 8),
          showsArrowhead: false,
          padding: 6
        )
      )
    )

    assertRenderedImageContainsRedPixel(image)
  }

  func testBodyUsesConfiguredFrameSize() throws {
    let image = try XCTUnwrap(
      renderedImage(
        GesturePatternView(
          pattern: [.right],
          configuration: .init(frameSize: CGSize(width: 90, height: 50))
        )
      ))

    XCTAssertEqual(image.width, 90)
    XCTAssertEqual(image.height, 50)
  }

  func testBodyRendersNoVisiblePixelsWhenPatternIsEmpty() {
    let image = renderedImage(
      GesturePatternView(
        pattern: [],
        configuration: .init(frameSize: CGSize(width: 120, height: 80))
      )
    )

    assertRenderedImageContainsNoVisiblePixels(image)
  }
}
