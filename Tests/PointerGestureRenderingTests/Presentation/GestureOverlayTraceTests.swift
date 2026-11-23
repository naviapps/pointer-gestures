import CoreGraphics
import PointerGestureRecognition
import XCTest

@testable import PointerGestureRendering

final class GestureOverlayTraceTests: XCTestCase {
  func testTailReplacementPreservesPriorSnapshotsAndRejectsInvalidTail() throws {
    let trace = try XCTUnwrap(
      GestureOverlayTrace(directions: [.right], points: [.zero, .init(x: 20, y: 0)]))
    let next = try XCTUnwrap(trace.replacingTailPoint(with: .init(x: 80, y: 3)))
    XCTAssertNil(trace.tailPoint)
    XCTAssertEqual(next.tailPoint, CGPoint(x: 80, y: 3))
    XCTAssertEqual(next.points, trace.points)
    XCTAssertEqual(next.replacingTailPoint(with: nil), trace)
    XCTAssertNil(trace.replacingTailPoint(with: .init(x: CGFloat.nan, y: 0)))
  }

  func testInitializerAcceptsValidTrace() throws {
    let trace = try XCTUnwrap(
      GestureOverlayTrace(
        directions: [.right, .down],
        points: [.zero, CGPoint(x: 20, y: 0), CGPoint(x: 20, y: 30)],
        tailPoint: CGPoint(x: 22, y: 34)
      )
    )

    XCTAssertEqual(trace.directions, [.right, .down])
    XCTAssertEqual(trace.points, [.zero, CGPoint(x: 20, y: 0), CGPoint(x: 20, y: 30)])
    XCTAssertEqual(trace.tailPoint, CGPoint(x: 22, y: 34))
  }

  func testInitializerReturnsNilForNativeRejection() {
    XCTAssertNil(GestureOverlayTrace(directions: [.right], points: [.zero]))
  }

  func testIndependentTracesAndPresentationUpdatesCompareByValue() throws {
    let trace = try XCTUnwrap(
      GestureOverlayTrace(directions: [.right], points: [.zero, CGPoint(x: 20, y: 0)]))
    let independent = try XCTUnwrap(
      GestureOverlayTrace(
        directions: [.right],
        points: [CGPoint(x: -CGFloat.zero, y: 0), CGPoint(x: 20, y: -CGFloat.zero)]))
    XCTAssertEqual(trace, independent)
    XCTAssertNotEqual(
      trace,
      GestureOverlayTrace(directions: [.left], points: trace.points, tailPoint: trace.tailPoint))
    XCTAssertNotEqual(
      trace,
      GestureOverlayTrace(
        directions: trace.directions, points: [.zero, CGPoint(x: 21, y: 0)],
        tailPoint: trace.tailPoint))
    XCTAssertNotEqual(
      trace, trace.replacingTailPoint(with: .zero))

    let first = GestureOverlayState(nowNanoseconds: 0)
    let second = GestureOverlayState(nowNanoseconds: 0)
    XCTAssertEqual(
      first.update(trace, nowNanoseconds: 0), second.update(independent, nowNanoseconds: 0))
  }
}
