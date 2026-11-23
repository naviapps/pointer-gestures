import XCTest

@testable import PointerGestureRendering
@testable import PointerGestureRenderingSwiftUI

final class GestureStrokeStyleTests: XCTestCase {
  func testLiveOverlayStrokeStyleReturnsExpectedConstants() {
    let style = GestureStrokeStyle.liveOverlay

    XCTAssertEqual(style.color, .blue)
    XCTAssertEqual(style.lineWidth, 4.0, accuracy: 0.0001)
  }

  func testPatternPreviewStrokeStyleReturnsExpectedConstants() {
    let style = GestureStrokeStyle.patternPreview

    XCTAssertEqual(style.color, .accentColor)
    XCTAssertEqual(style.lineWidth, 3, accuracy: 0.0001)
  }

  func testInitializerUsesLiveOverlayDefaults() {
    XCTAssertEqual(GestureStrokeStyle(), .liveOverlay)
  }

  func testInitializerPreservesCustomValues() {
    let style = GestureStrokeStyle(
      color: .red,
      lineWidth: 9.0
    )

    XCTAssertEqual(style.color, .red)
    XCTAssertEqual(style.lineWidth, 9.0, accuracy: 0.0001)
  }

  func testInitializerNormalizesInvalidLineWidth() {
    assertInvalidLineWidthsNormalizeToZero([-1, .infinity, -.infinity, .nan])
  }

  func testStrokeStyleEqualityIncludesStoredValues() {
    let base = GestureStrokeStyle(color: .red, lineWidth: 9.0)
    let differentColor = GestureStrokeStyle(color: .blue, lineWidth: 9.0)
    let differentLineWidth = GestureStrokeStyle(color: .red, lineWidth: 4.0)

    XCTAssertEqual(base, base)
    XCTAssertNotEqual(base, differentColor)
    XCTAssertNotEqual(base, differentLineWidth)
  }

  func testGeneratedStrokeStyleUsesRoundCapsAndJoins() {
    let strokeStyle = GestureStrokeStyle(lineWidth: 7).makeStrokeStyle()

    XCTAssertEqual(strokeStyle.lineWidth, 7, accuracy: 0.0001)
    XCTAssertEqual(strokeStyle.lineCap, .round)
    XCTAssertEqual(strokeStyle.lineJoin, .round)
  }

  func testGeneratedStrokeStyleAcceptsRendererSpecificLineJoin() {
    let strokeStyle = GestureStrokeStyle(lineWidth: 7).makeStrokeStyle(lineJoin: .miter)

    XCTAssertEqual(strokeStyle.lineWidth, 7, accuracy: 0.0001)
    XCTAssertEqual(strokeStyle.lineCap, .round)
    XCTAssertEqual(strokeStyle.lineJoin, .miter)
  }

  private func assertInvalidLineWidthsNormalizeToZero(
    _ lineWidths: [CGFloat],
    file: StaticString = #filePath,
    line: UInt = #line
  ) {
    for lineWidth in lineWidths {
      let style = GestureStrokeStyle(lineWidth: lineWidth)

      XCTAssertEqual(style.lineWidth, 0, accuracy: 0.0001, file: file, line: line)
    }
  }
}
