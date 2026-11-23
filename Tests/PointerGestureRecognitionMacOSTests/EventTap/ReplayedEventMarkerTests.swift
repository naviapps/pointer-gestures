import CoreGraphics
import XCTest

@testable import PointerGestureRecognitionMacOS

final class ReplayedEventMarkerTests: XCTestCase {
  func testMarkerSurvivesRepeatedDetectionWithoutAQueueOrChangingInputFields() throws {
    let event = try XCTUnwrap(
      CGEvent(
        mouseEventSource: nil, mouseType: .rightMouseUp,
        mouseCursorPosition: CGPoint(x: 15, y: 25), mouseButton: .right))
    XCTAssertFalse(ReplayedEventMarker.isPresent(on: event))
    ReplayedEventMarker.mark(event)
    for _ in 0..<3 { XCTAssertTrue(ReplayedEventMarker.isPresent(on: event)) }
    XCTAssertEqual(event.type, .rightMouseUp)
    XCTAssertEqual(event.location, CGPoint(x: 15, y: 25))
    XCTAssertEqual(event.getIntegerValueField(.mouseEventButtonNumber), 1)
    let unrelated = try XCTUnwrap(CGEvent(source: nil))
    unrelated.setIntegerValueField(.eventSourceUserData, value: 42)
    XCTAssertFalse(ReplayedEventMarker.isPresent(on: unrelated))
  }
}
