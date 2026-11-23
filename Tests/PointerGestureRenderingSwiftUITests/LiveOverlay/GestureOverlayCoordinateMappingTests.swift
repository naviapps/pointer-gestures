import CoreGraphics
import XCTest

@testable import PointerGestureRendering
@testable import PointerGestureRenderingSwiftUI

final class GestureOverlayCoordinateMappingTests: XCTestCase {
  func testConvertMapsDisplayPointIntoWindowCanvas() {
    let mapping = GestureOverlayCoordinateMapping(
      windowOrigin: CGPoint(x: 20, y: 30),
      displayCoordinateMaxY: 180
    )

    let converted = mapping.convert(
      CGPoint(x: 25, y: 90),
      canvasSize: CGSize(width: 200, height: 120)
    )

    assertEqualPoint(converted, CGPoint(x: 5, y: 60), accuracy: 0.0001)
  }

  func testInitializerPreservesNegativeFiniteCoordinates() {
    let mapping = GestureOverlayCoordinateMapping(
      windowOrigin: CGPoint(x: -200, y: -100),
      displayCoordinateMaxY: -50
    )

    let converted = mapping.convert(.zero, canvasSize: CGSize(width: 80, height: 60))

    assertEqualPoint(converted, CGPoint(x: 200, y: 10), accuracy: 0.0001)
  }

  func testInitializerNormalizesNonFiniteValues() {
    let mapping = GestureOverlayCoordinateMapping(
      windowOrigin: CGPoint(x: CGFloat.nan, y: CGFloat.infinity),
      displayCoordinateMaxY: -CGFloat.infinity
    )

    let converted = mapping.convert(CGPoint.zero, canvasSize: CGSize.zero)

    assertEqualPoint(converted, CGPoint.zero, accuracy: 0.0001)
  }

  func testEqualityIncludesMappingInputs() {
    let mapping = GestureOverlayCoordinateMapping(
      windowOrigin: CGPoint(x: 12, y: 34),
      displayCoordinateMaxY: 1_020
    )

    XCTAssertEqual(mapping, mapping)
    XCTAssertNotEqual(
      mapping,
      GestureOverlayCoordinateMapping(
        windowOrigin: CGPoint(x: 13, y: 34),
        displayCoordinateMaxY: 1_020
      )
    )
    XCTAssertNotEqual(
      mapping,
      GestureOverlayCoordinateMapping(
        windowOrigin: CGPoint(x: 12, y: 34),
        displayCoordinateMaxY: 1_021
      )
    )
  }
}
