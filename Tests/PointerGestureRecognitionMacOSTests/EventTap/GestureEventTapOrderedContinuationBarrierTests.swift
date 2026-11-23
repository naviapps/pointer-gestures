import CoreGraphics
import PointerGestureRecognition
import XCTest

@testable import PointerGestureRecognitionMacOS

@MainActor
final class GestureEventTapOrderedContinuationBarrierTests: XCTestCase {
  func testNewPressCannotPassAheadOfAnOwnedReleaseAwaitingDelivery() throws {
    let fifo = BarrierReplayQueue()
    let tap = GestureEventTap(capturedButtons: [.secondary]) { fifo.posted.append($0) }
    tap.installHandler { [weak tap] input in
      switch input.kind {
      case .buttonDown(.secondary) where input.location.x == 200:
        return .consume
      case .buttonMoved(.secondary) where input.location.x == 120:
        let request = GestureReplayRequest.dragStart(
          button: .secondary, points: [.init(x: 200, y: 30), input.location])
        let accepted = tap?.enqueueReplayFollowingThrough(
          request, inOrder: { fifo.pending.append($0) })
        XCTAssertEqual(accepted, true)
        return .consume
      default:
        return .passThrough
      }
    }
    defer { tap.stop() }

    let source = try XCTUnwrap(CGEventSource(stateID: .privateState))
    let physical: [(CGEventType, CGFloat)] = [
      (.rightMouseDown, 200), (.rightMouseDragged, 120), (.rightMouseUp, 100),
      (.rightMouseDown, 300), (.rightMouseDragged, 320), (.rightMouseUp, 340),
    ]
    let inputs = try physical.map { try makeBarrierInput($0.0, x: $0.1, source: source) }
    for event in inputs {
      XCTAssertNil(tap.handleTapEvent(type: event.type, event: event))
    }
    XCTAssertTrue(fifo.posted.isEmpty)

    let accepted = fifo.pending
    fifo.pending.removeAll()
    for delivery in accepted { delivery() }

    XCTAssertEqual(fifo.posted.map(\.type), physical.map { $0.0 })
    XCTAssertEqual(fifo.posted.map { $0.location.x }, physical.map { $0.1 })
    XCTAssertTrue(fifo.posted.allSatisfy(ReplayedEventMarker.isPresent(on:)))
    XCTAssertTrue(fifo.pending.isEmpty)

    let ordinary = try makeBarrierInput(.rightMouseDown, x: 400, source: source)
    XCTAssertTrue(
      tap.handleTapEvent(type: ordinary.type, event: ordinary)?.takeUnretainedValue()
        === ordinary)
    XCTAssertTrue(fifo.pending.isEmpty)
  }
}

@MainActor
private final class BarrierReplayQueue {
  var pending: [@MainActor @Sendable () -> Void] = []
  var posted: [CGEvent] = []
}

private func makeBarrierInput(_ type: CGEventType, x: CGFloat, source: CGEventSource) throws
  -> CGEvent
{
  try XCTUnwrap(
    CGEvent(
      mouseEventSource: source, mouseType: type, mouseCursorPosition: CGPoint(x: x, y: 30),
      mouseButton: .right))
}
