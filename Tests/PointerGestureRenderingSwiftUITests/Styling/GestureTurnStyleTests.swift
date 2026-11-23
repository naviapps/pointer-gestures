import XCTest

@testable import PointerGestureRendering
@testable import PointerGestureRenderingSwiftUI

final class GestureTurnStyleTests: XCTestCase {
  func testRoundedTurnStyleUsesStandardCornerRadiusScale() {
    XCTAssertEqual(GestureTurnStyle.rounded.cornerRadiusScale, 1.0, accuracy: 0.0001)
  }

  func testRightAngleTurnStyleUsesZeroCornerRadiusScale() {
    XCTAssertEqual(GestureTurnStyle.rightAngle.cornerRadiusScale, 0, accuracy: 0.0001)
  }

  func testInitializerPreservesCustomCornerRadiusScale() {
    let turnStyle = GestureTurnStyle(cornerRadiusScale: 0.25)

    XCTAssertEqual(turnStyle.cornerRadiusScale, 0.25, accuracy: 0.0001)
  }

  func testInitializerNormalizesInvalidCornerRadiusScale() {
    assertInvalidCornerRadiusScalesNormalizeToZero([-1, .infinity, -.infinity, .nan])
  }

  func testTurnStyleEqualityIncludesCornerRadiusScale() {
    let custom = GestureTurnStyle(cornerRadiusScale: 0.25)

    XCTAssertEqual(custom, custom)
    XCTAssertNotEqual(custom, .rounded)
    XCTAssertNotEqual(custom, .rightAngle)
  }

  func testLineJoinMatchesNormalizedCornerRadiusScale() {
    XCTAssertEqual(GestureTurnStyle.rounded.lineJoin, .round)
    XCTAssertEqual(GestureTurnStyle.rightAngle.lineJoin, .miter)
    XCTAssertEqual(GestureTurnStyle(cornerRadiusScale: 0).lineJoin, .miter)
    XCTAssertEqual(GestureTurnStyle(cornerRadiusScale: -1).lineJoin, .miter)
    XCTAssertEqual(GestureTurnStyle(cornerRadiusScale: 0.25).lineJoin, .round)
  }

  private func assertInvalidCornerRadiusScalesNormalizeToZero(
    _ cornerRadiusScales: [CGFloat],
    file: StaticString = #filePath,
    line: UInt = #line
  ) {
    for cornerRadiusScale in cornerRadiusScales {
      let turnStyle = GestureTurnStyle(cornerRadiusScale: cornerRadiusScale)

      XCTAssertEqual(turnStyle.cornerRadiusScale, 0, accuracy: 0.0001, file: file, line: line)
    }
  }
}
