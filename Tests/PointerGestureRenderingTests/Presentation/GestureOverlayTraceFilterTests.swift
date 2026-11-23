import CoreGraphics
import XCTest

@testable import PointerGestureRendering

final class GestureOverlayTraceFilterTests: XCTestCase {
  func testFilterUsesTranslatedViewportAndStrokeWidth() throws {
    let filter = GestureOverlayTraceFilter(
      displayCoordinateFrame: CGRect(x: -200, y: 80, width: 50, height: 30), lineWidth: 4)
    let trace = try XCTUnwrap(
      GestureOverlayTrace(
        directions: [.right], points: [CGPoint(x: -148, y: 100), CGPoint(x: -130, y: 100)]))

    XCTAssertNil(filter.filteredTrace(nil))
    XCTAssertEqual(filter.filteredTrace(trace), trace)
  }

  func testTraceRetainsSwiftPointsAndNativeBoundsAfterSourceArrayEdits() throws {
    let filter = GestureOverlayTraceFilter(
      displayCoordinateFrame: CGRect(x: 0, y: 0, width: 100, height: 100), lineWidth: 4)
    var points = [CGPoint(x: 10, y: 10), CGPoint(x: 40, y: 10)]
    let retained = try XCTUnwrap(GestureOverlayTrace(directions: [.right], points: points))
    points[0] = CGPoint(x: 150, y: 10)
    points[1] = CGPoint(x: 180, y: 10)
    let moved = try XCTUnwrap(GestureOverlayTrace(directions: [.right], points: points))

    XCTAssertNil(filter.filteredTrace(moved))
    XCTAssertEqual(retained.points, [CGPoint(x: 10, y: 10), CGPoint(x: 40, y: 10)])
    XCTAssertEqual(filter.filteredTrace(retained), retained)
  }

  func testInvalidViewportRejectsTraces() throws {
    let filter = GestureOverlayTraceFilter(
      displayCoordinateFrame: CGRect(x: CGFloat.nan, y: 0, width: 100, height: 100), lineWidth: 4)
    let trace = try XCTUnwrap(
      GestureOverlayTrace(
        directions: [.right], points: [CGPoint(x: 10, y: 10), CGPoint(x: 40, y: 10)]))
    XCTAssertNil(filter.filteredTrace(trace))
  }
}
