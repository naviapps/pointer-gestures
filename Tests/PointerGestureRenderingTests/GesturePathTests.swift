import CPointerGestureRendering
import Foundation
import XCTest

@testable import PointerGestureRendering

final class GesturePathTests: XCTestCase {
  func testImportedCommandAndTraceLayout() {
    typealias Command = MemoryLayout<PointerGestureRenderingCommand>
    XCTAssertEqual(Command.stride, 56)
    XCTAssertEqual(Command.alignment, 8)
    XCTAssertEqual(Command.offset(of: \.point), 8)

    typealias Trace = MemoryLayout<PointerGestureRenderingTraceView>
    XCTAssertEqual(Trace.stride, 56)
    XCTAssertEqual(Trace.alignment, 8)
    XCTAssertEqual(Trace.offset(of: \.points), 16)
    XCTAssertEqual(Trace.offset(of: \.tail), 32)
    XCTAssertEqual(Trace.offset(of: \.tail_present), 48)
  }

  func testNativeResultWithoutGeometryReturnsTypedFailure() {
    XCTAssertEqual(
      GesturePath.decode(.init(code: 5, geometry: nil)), .failure(.invalidDirection))
    for status in [UInt32(0), 4, .max] {
      let result = GesturePath.decode(PointerGestureRenderingResult(code: status, geometry: nil))
      XCTAssertEqual(result, .failure(.invalidNativeResult))
      XCTAssertNil(try? result.get())
    }
  }

  func testLivePlanRejectsInvalidTracesAndPlansSharpTurns() throws {
    XCTAssertNil(GesturePath.liveTrace(directions: [], points: [.zero]))
    XCTAssertEqual(
      GesturePath.planLiveTrace(directions: [.right], points: [.zero]), .failure(.pointCount))
    let maximum = CGFloat.greatestFiniteMagnitude
    XCTAssertEqual(
      GesturePath.planLiveTrace(
        directions: [.right], points: [CGPoint(x: -maximum, y: 0), CGPoint(x: maximum, y: 0)]),
      .failure(.nonFinite))
    let plan = try GesturePath.planLiveTrace(
      directions: [.right, .down], points: [.zero, CGPoint(x: 80, y: 0), CGPoint(x: 80, y: 80)],
      cornerRadiusScale: 0
    ).get()
    XCTAssertEqual(
      plan.commands,
      [
        .move(to: .zero), .line(to: CGPoint(x: 80, y: 0)),
        .line(to: CGPoint(x: 80, y: 80)),
      ])
  }

  func testLivePlanCopiesTheOptionalTailAndNativeCoordinates() throws {
    let origin = CGPoint(x: 0, y: -CGFloat.greatestFiniteMagnitude)
    let tail = CGPoint(x: 50, y: 20)
    let path = try XCTUnwrap(
      GesturePath.liveTrace(directions: [.right], points: [origin, origin], tailPoint: tail))
    XCTAssertEqual(path.commands, [.move(to: origin), .line(to: .init(x: tail.x, y: origin.y))])
  }

  func testPreviewGeometryOwnsASeparateClosedArrowhead() throws {
    XCTAssertNil(GesturePath.pattern([], canvasSize: CGSize(width: 96, height: 64)))
    let canvasSize = CGSize(width: 96, height: 64)
    XCTAssertEqual(GesturePath.planPattern([], canvasSize: canvasSize), .failure(.empty))
    let plan = try GesturePath.planPattern([.right, .down], canvasSize: canvasSize).get()
    XCTAssertTrue(
      plan.path.commands.contains {
        if case .cubic = $0 { return true }
        return false
      })
    XCTAssertEqual(plan.arrowhead?.commands.last, .close)
    XCTAssertEqual(
      plan, GesturePath.pattern([.right, .down], canvasSize: CGSize(width: 96, height: 64)))
    let without = try XCTUnwrap(
      GesturePath.pattern(
        [.right, .down],
        canvasSize: CGSize(width: 96, height: 64), showsArrowhead: false))
    XCTAssertEqual(plan.path, without.path)
    XCTAssertNil(without.arrowhead)
    XCTAssertNotEqual(plan, without)
  }
}
