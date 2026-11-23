import XCTest

@testable import PointerGestureRecognition

@MainActor
final class GestureRecognizerDeadlineTests: XCTestCase {

  func testExpiredInputCannotPromoteOrMatchBeforeTheTimerRuns() throws {
    for active in [false, true] {
      let clock = GestureRecognizerTestClock()
      let source = GestureEventSourceDouble(startResult: true)
      var matcher = GesturePatternMatcher<UUID>()
      matcher.register(pattern: [.right], match: UUID())
      var matches = 0
      var replays: [GestureReplayRequest] = []
      let recognizer = GestureRecognizer<UUID>(
        eventSource: source,
        configuration: makeGestureRecognizerTestConfiguration(
          makeMatcher: { [matcher] _ in matcher }, onReplayRequested: { replays.append($0) },
          onMatch: { _ in matches += 1 }, tuning: try .validated(maximumGestureSessionDuration: 1)))
      recognizer.nowOverride = { clock.now }
      recognizer.start()
      _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
      if active {
        _ = source.send(
          makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 20, y: 0)))
      }
      clock.now = 2_000_000_000
      _ = source.send(
        makeGestureInputEvent(
          kind: active ? .buttonUp(.secondary) : .buttonMoved(.secondary),
          location: .init(x: 20, y: 0)))
      XCTAssertEqual(matches, 0)
      XCTAssertEqual(replays.count, 1)
      XCTAssertEqual(recognizer.snapshot.status.lastFailure, .gestureSessionExpired)
      XCTAssertFalse(recognizer.snapshot.status.isCapturingGesture)
      XCTAssertEqual(recognizer.inputGeneration, 0)
      recognizer.stop()
    }
  }

  func testChangingOnlyRetryTuningStillRestartsActiveInputDeadline() throws {
    let clock = GestureRecognizerTestClock()
    let source = GestureEventSourceDouble(startResult: true)
    let recognizer = GestureRecognizer<UUID>(
      eventSource: source,
      configuration: makeGestureRecognizerTestConfiguration(
        tuning: try .validated(maximumGestureSessionDuration: 1, eventSourceStartRetryDelays: [])))
    recognizer.nowOverride = { clock.now }
    recognizer.isRecordingModeEnabled = true
    recognizer.start()
    _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    clock.now = 750_000_000
    XCTAssertTrue(
      recognizer.updateTuning(
        try .validated(
          maximumGestureSessionDuration: 1, eventSourceStartRetryDelays: [0.1])))
    XCTAssertEqual(recognizer.nextDeadline, 1_750_000_000)
    clock.now = 1_250_000_000
    _ = source.send(
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 20, y: 0)))
    XCTAssertNil(recognizer.snapshot.status.lastFailure)
    recognizer.stop()
  }
}
