import CoreGraphics
import XCTest

@testable import PointerGestureRendering

final class GestureOverlayStateTests: XCTestCase {
  func testFinalSnapshotOutlivesReducerAndPendingTrace() throws {
    let first = try XCTUnwrap(
      GestureOverlayTrace(directions: [.right], points: [.zero, CGPoint(x: 1, y: 0)]))
    let final = try XCTUnwrap(
      GestureOverlayTrace(directions: [.right], points: [.zero, CGPoint(x: 2, y: 0)]))
    let update: GestureOverlayUpdate
    do {
      let state = GestureOverlayState(
        timing: .init(minimumAnimationInterval: 0.01), trace: first, nowNanoseconds: 0)
      XCTAssertFalse(state.update(final, nowNanoseconds: 1).changed)
      update = state.update(nil, nowNanoseconds: 2)
      let configured = state.configure(
        timing: .init(minimumAnimationInterval: 0.02), nowNanoseconds: 3)
      let deadline = try XCTUnwrap(configured.deadlineNanoseconds)
      XCTAssertGreaterThan(deadline, try XCTUnwrap(update.deadlineNanoseconds))
      let hidden = state.tick(nowNanoseconds: deadline)
      XCTAssertTrue(hidden.changed)
      XCTAssertNil(hidden.trace)
      XCTAssertNil(hidden.deadlineNanoseconds)
    }
    XCTAssertEqual(update.trace, final)
    XCTAssertTrue(
      try XCTUnwrap(update.trace).intersects(viewport: CGRect(x: 1, y: -1, width: 2, height: 2)))
  }
}
