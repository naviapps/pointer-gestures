import CoreGraphics
import XCTest

@testable import PointerGestureRendering

final class RenderingInputNormalizerTests: XCTestCase {
  func testCGFloatValuesCrossBothNativeNormalizationFunctions() {
    for (input, finite, nonnegative): (CGFloat, CGFloat, CGFloat) in [
      (-12, -12, 0), (12, 12, 12), (.nan, 0, 0),
    ] {
      XCTAssertEqual(RenderingInputNormalizer.finiteOrZero(input), finite)
      XCTAssertEqual(RenderingInputNormalizer.nonNegativeFiniteOrZero(input), nonnegative)
    }
  }
}
