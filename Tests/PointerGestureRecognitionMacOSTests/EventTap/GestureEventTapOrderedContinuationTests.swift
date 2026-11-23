import CoreGraphics
import XCTest

@testable import PointerGestureRecognition
@testable import PointerGestureRecognitionMacOS

@MainActor
final class GestureEventTapOrderedContinuationTests: XCTestCase {
  func testAcceptedRejectedDragContinuationKeepsPhysicalReleaseBehindOlderReplay() throws {
    let fixture = try OrderedContinuationFixture()
    defer { fixture.recognizer.stop() }

    try fixture.send(.rightMouseDown, x: 300)
    try fixture.send(.rightMouseUp, x: 300)
    try fixture.beginRejectedDrag()
    let move = try fixture.send(.rightMouseDragged, x: 100, count: 3, flags: .maskControl)
    let up = try fixture.send(.rightMouseUp, x: 80, count: 4, flags: .maskCommand)

    XCTAssertEqual(
      fixture.host.requests,
      [
        .click(button: .secondary, at: .init(x: 300, y: 30)),
        .dragStart(
          button: .secondary, points: [.init(x: 200, y: 30), .init(x: 120, y: 30)]),
      ])
    XCTAssertEqual(fixture.recognizer.snapshot.lastSessionReport?.outcome, .unmatchedPrefix)
    guard fixture.host.downstream.isEmpty else {
      XCTFail("ordered-continuation-physical-input-before-drain")
      return
    }
    fixture.host.drain()

    XCTAssertEqual(
      fixture.host.downstream.map(\.type),
      [
        .rightMouseDown, .rightMouseUp, .rightMouseDown, .rightMouseDragged, .rightMouseDragged,
        .rightMouseUp,
      ])
    XCTAssertEqual(fixture.host.downstream.map { $0.location.x }, [300, 300, 200, 120, 100, 80])
    XCTAssertTrue(fixture.host.downstream.allSatisfy(ReplayedEventMarker.isPresent(on:)))
    XCTAssertEqual(fixture.host.downstream.suffix(2).map(\.flags), [.maskControl, .maskCommand])
    XCTAssertEqual(
      fixture.host.downstream.suffix(2).map { $0.getIntegerValueField(.mouseEventClickState) },
      [3, 4])
    XCTAssertFalse(ReplayedEventMarker.isPresent(on: move))
    XCTAssertFalse(ReplayedEventMarker.isPresent(on: up))

    fixture.host.recognitionEnabled = false
    let ordinaryDown = try fixture.send(.rightMouseDown, x: 200)
    let ordinaryUp = try fixture.send(.rightMouseUp, x: 200)
    XCTAssertTrue(fixture.host.downstream.dropLast().last === ordinaryDown)
    XCTAssertTrue(fixture.host.downstream.last === ordinaryUp)
    XCTAssertFalse(ReplayedEventMarker.isPresent(on: ordinaryUp))
    XCTAssertTrue(fixture.host.pending.isEmpty)
  }

  func testOwnedContinuationCoalescesPendingMovementButKeepsTerminalRelease() throws {
    let fixture = try OrderedContinuationFixture()
    defer { fixture.recognizer.stop() }
    try fixture.beginRejectedDrag()
    for x in (40...110).reversed() {
      try fixture.send(.rightMouseDragged, x: CGFloat(x))
    }
    try fixture.send(.rightMouseUp, x: 39)
    XCTAssertTrue(fixture.host.downstream.isEmpty)

    fixture.host.drain()
    XCTAssertEqual(
      fixture.host.downstream.map(\.type),
      [.rightMouseDown, .rightMouseDragged, .rightMouseDragged, .rightMouseUp])
    XCTAssertEqual(fixture.host.downstream.map { $0.location.x }, [200, 120, 110, 39])
    XCTAssertTrue(fixture.host.downstream.allSatisfy(ReplayedEventMarker.isPresent(on:)))
  }

  func testStoppingWithAcceptedDragStartClosesTheDeferredPress() throws {
    for startAlreadyPosted in [false, true] {
      let fixture = try OrderedContinuationFixture()
      try fixture.beginRejectedDrag()
      if startAlreadyPosted { fixture.host.drain() }
      try fixture.send(.rightMouseDragged, x: 100)
      let eventsBeforeStop = fixture.host.downstream.map(\.type)

      fixture.recognizer.stop()
      XCTAssertEqual(fixture.host.downstream.map(\.type), eventsBeforeStop)
      fixture.host.drain()
      XCTAssertEqual(
        fixture.host.downstream.map(\.type),
        startAlreadyPosted
          ? [.rightMouseDown, .rightMouseDragged, .rightMouseDragged, .rightMouseUp] : [])
      XCTAssertTrue(fixture.host.downstream.allSatisfy(ReplayedEventMarker.isPresent(on:)))
    }
  }

  func testStoppingAfterPhysicalReleaseRetainsCompleteAcceptedPair() throws {
    let fixture = try OrderedContinuationFixture()
    try fixture.beginRejectedDrag()
    try fixture.send(.rightMouseDragged, x: 100)
    try fixture.send(.rightMouseUp, x: 80)
    fixture.recognizer.stop()
    XCTAssertTrue(fixture.host.downstream.isEmpty)

    fixture.host.drain()
    XCTAssertEqual(
      fixture.host.downstream.map(\.type),
      [.rightMouseDown, .rightMouseDragged, .rightMouseDragged, .rightMouseUp])
    XCTAssertEqual(fixture.host.downstream.last?.location.x, 80)
    XCTAssertTrue(fixture.host.downstream.allSatisfy(ReplayedEventMarker.isPresent(on:)))
  }

  func testTapTimeoutWithAcceptedDragStartClosesTheDeferredPress() throws {
    for startAlreadyPosted in [false, true] {
      let fixture = try OrderedContinuationFixture()
      defer { fixture.recognizer.stop() }
      try fixture.beginRejectedDrag()
      if startAlreadyPosted { fixture.host.drain() }
      try fixture.send(.rightMouseDragged, x: 100)
      let eventsBeforeTimeout = fixture.host.downstream.map(\.type)

      try fixture.tapTimeout()
      XCTAssertEqual(fixture.host.downstream.map(\.type), eventsBeforeTimeout)
      fixture.host.drain()
      XCTAssertEqual(
        fixture.host.downstream.map(\.type),
        startAlreadyPosted
          ? [.rightMouseDown, .rightMouseDragged, .rightMouseDragged, .rightMouseUp] : [])
      XCTAssertTrue(fixture.host.downstream.allSatisfy(ReplayedEventMarker.isPresent(on:)))
    }
  }

  func testReplacementDownClosesOlderFollowThroughWithoutOwningItsRelease() throws {
    for startAlreadyPosted in [false, true] {
      let fixture = try OrderedContinuationFixture()
      defer { fixture.recognizer.stop() }
      try fixture.beginRejectedDrag()
      if startAlreadyPosted { fixture.host.drain() }
      try fixture.send(.rightMouseDown, x: 300, count: 5)
      try fixture.send(.rightMouseUp, x: 300, count: 5)

      fixture.host.drain()
      XCTAssertEqual(
        fixture.host.downstream.map(\.type),
        startAlreadyPosted
          ? [.rightMouseDown, .rightMouseDragged, .rightMouseUp, .rightMouseDown, .rightMouseUp]
          : [.rightMouseDown, .rightMouseUp])
      XCTAssertEqual(fixture.host.downstream.suffix(2).map { $0.location.x }, [300, 300])
      XCTAssertEqual(
        fixture.host.downstream.suffix(2).map { $0.getIntegerValueField(.mouseEventClickState) },
        [5, 5])
      XCTAssertTrue(fixture.host.downstream.allSatisfy(ReplayedEventMarker.isPresent(on:)))
    }
  }
}

@MainActor
private final class OrderedContinuationHost {
  var recognitionEnabled = true
  var requests: [GestureReplayRequest] = []
  var downstream: [CGEvent] = []
  var pending: [@MainActor @Sendable () -> Void] = []

  func drain() {
    // A single main-actor turn can deliver all physical input before this FIFO drains.
    let operations = pending
    pending.removeAll()
    for operation in operations { operation() }
    XCTAssertTrue(pending.isEmpty, "Accepted replay must not produce additional work on drain.")
  }
}

@MainActor
private final class OrderedContinuationFixture {
  let host: OrderedContinuationHost
  let tap: GestureEventTap
  let recognizer: GestureRecognizer<String>
  private let source: CGEventSource
  private var inputs: [CGEvent] = []

  init() throws {
    let host = OrderedContinuationHost()
    let tap = GestureEventTap(capturedButtons: [.secondary]) { host.downstream.append($0) }
    var matcher = GesturePatternMatcher<String>()
    matcher.register(pattern: [.right], match: "right")
    let configuration = GestureRecognizerConfiguration(
      makeMatcher: { [matcher] _ in matcher },
      onReplayRequested: { request in
        host.requests.append(request)
        let accepted = tap.enqueueReplayFollowingThrough(
          request,
          inOrder: { operation in
            host.pending.append(operation)
          })
        XCTAssertTrue(accepted)
      },
      onMatch: { _ in XCTFail("Rejected input must not match the fixture pattern.") },
      recognitionButton: .secondary,
      isRecognitionEnabled: { _ in host.recognitionEnabled },
      tuning: try .validated(maximumGestureSessionDuration: nil))
    self.host = host
    self.tap = tap
    source = try XCTUnwrap(CGEventSource(stateID: .privateState))
    recognizer = GestureRecognizer(
      eventSource: OrderedContinuationTapSource(tap), configuration: configuration)
    recognizer.nowOverride = { 0 }
    recognizer.start()
  }

  func beginRejectedDrag() throws {
    try send(.rightMouseDown, x: 200, count: 2, flags: .maskShift)
    try send(.rightMouseDragged, x: 120, count: 2, flags: .maskAlternate)
  }

  @discardableResult
  func send(_ type: CGEventType, x: CGFloat, count: Int64 = 1, flags: CGEventFlags = []) throws
    -> CGEvent
  {
    let event = try XCTUnwrap(
      CGEvent(
        mouseEventSource: source, mouseType: type, mouseCursorPosition: CGPoint(x: x, y: 30),
        mouseButton: .right))
    event.flags = flags
    event.setIntegerValueField(.mouseEventClickState, value: count)
    inputs.append(event)
    if let output = tap.handleTapEvent(type: type, event: event) {
      host.downstream.append(output.takeUnretainedValue())
    }
    return event
  }

  func tapTimeout() throws {
    let event = try XCTUnwrap(CGEvent(source: source))
    inputs.append(event)
    XCTAssertTrue(
      tap.handleTapEvent(type: .tapDisabledByTimeout, event: event)?.takeUnretainedValue() === event
    )
  }
}

@MainActor
private final class OrderedContinuationTapSource: GestureEventSource {
  private let tap: GestureEventTap

  init(_ tap: GestureEventTap) { self.tap = tap }

  func start(
    handler: @escaping @MainActor @Sendable (GestureInputEvent) -> GestureEventDisposition
  ) -> Bool {
    tap.installHandler(handler)
    return true
  }

  func stop(recoveringInput: @MainActor () -> Void) {
    tap.stop(recoveringInput: recoveringInput)
  }
}
