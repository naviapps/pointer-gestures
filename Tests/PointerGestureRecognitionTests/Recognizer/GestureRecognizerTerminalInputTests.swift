import XCTest

@testable import PointerGestureRecognition

@MainActor
final class GestureRecognizerTerminalInputTests: XCTestCase {
  func testMalformedReleaseRoutesToTheHeldButtonAndRecoversOnceAtDeadline() throws {
    let clock = GestureRecognizerTestClock()
    let source = GestureEventSourceDouble(startResult: true)
    var replays: [GestureReplayRequest] = []
    let recognizer = GestureRecognizer<String>(
      eventSource: source,
      configuration: makeGestureRecognizerTestConfiguration(
        onReplayRequested: { replays.append($0) },
        recognitionButton: .primary,
        tuning: try .validated(maximumGestureSessionDuration: 1)))
    recognizer.nowOverride = { clock.now }
    recognizer.isRecordingModeEnabled = true
    recognizer.start()
    _ = source.send(makeGestureInputEvent(kind: .buttonDown(.primary), location: .zero))
    _ = source.send(
      makeGestureInputEvent(kind: .buttonMoved(.primary), location: .init(x: 40, y: 0)))
    let before = recognizer.snapshot
    let generation = recognizer.inputGeneration
    let deadline = try XCTUnwrap(recognizer.nextDeadline)

    clock.now = deadline
    assertDisposition(
      source.send(
        makeGestureInputEvent(
          kind: .buttonUp(.secondary), location: .init(x: .nan, y: .infinity))),
      .passThrough)
    XCTAssertEqual(recognizer.snapshot, before)
    XCTAssertEqual(recognizer.inputGeneration, generation)
    XCTAssertEqual(recognizer.nextDeadline, deadline)
    XCTAssertTrue(replays.isEmpty)
    assertDisposition(
      source.send(
        makeGestureInputEvent(
          kind: .buttonUp(.primary), location: .init(x: .nan, y: .infinity))),
      .consume)
    XCTAssertEqual(recognizer.snapshot.status.lastFailure, .gestureSessionExpired)
    XCTAssertFalse(recognizer.snapshot.trace.isVisible)
    XCTAssertEqual(recognizer.inputGeneration, 0)
    XCTAssertNil(recognizer.nextDeadline)
    recognizer.stop()
    assertReplayRequests(replays, [.release(button: .primary, at: .init(x: 40, y: 0))])
  }

  func testTerminalEventsDeliverTypedRecoveryAndObserverUpdates() {
    for (kind, active) in [(GestureInputEvent.Kind.cancel, false), (.buttonUp(.secondary), true)] {
      let source = GestureEventSourceDouble(startResult: true)
      var replays: [GestureReplayRequest] = []
      var matches: [String] = []
      var visibility: [Bool] = []
      let recognizer = GestureRecognizer<String>(
        eventSource: source,
        configuration: makeGestureRecognizerTestConfiguration(
          makeMatcher: { _ in
            var matcher = GesturePatternMatcher<String>()
            matcher.register(pattern: [.right], match: "right")
            return matcher
          },
          onReplayRequested: { replays.append($0) },
          onMatch: { matches.append($0) }
        ))
      let observation = recognizer.observeTrace { visibility.append($0.isVisible) }
      recognizer.start()
      _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
      if active {
        _ = source.send(
          makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 40, y: 0)))
      }
      assertDisposition(
        source.send(makeGestureInputEvent(kind: kind, location: .init(x: .nan, y: .infinity))),
        .consume)
      XCTAssertFalse(recognizer.snapshot.status.isCapturingGesture)
      XCTAssertFalse(recognizer.snapshot.trace.isVisible)
      XCTAssertEqual(visibility, active ? [false, true, false] : [false, false])
      XCTAssertTrue(recognizer.snapshot.trace.discardsPendingPresentation)
      assertReplayRequests(
        replays,
        active
          ? [.release(button: .secondary, at: .init(x: 40, y: 0))]
          : [.click(button: .secondary, at: .zero)])
      assertDisposition(
        source.send(
          makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 80, y: 0))),
        .passThrough)
      XCTAssertTrue(matches.isEmpty)
      recognizer.stop()
      XCTAssertEqual(replays.count, 1)
      observation.cancel()
    }
  }
}
