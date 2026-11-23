import XCTest

@testable import PointerGestureRecognition

final class GestureIdleExpirationTests: XCTestCase {

  func testIdleDurationRejectsNonPositiveAndNonFiniteValues() {
    for value: Double in [0, -1, .nan, .infinity, -.infinity] {
      XCTAssertThrowsError(try GestureRecognizerTuning.validated(maximumGestureIdleDuration: value))
      {
        XCTAssertEqual(
          $0 as? GestureRecognizerTuning.ValidationError, .invalidMaximumGestureIdleDuration)
      }
    }
    XCTAssertNil(GestureRecognizerTuning.standard.maximumGestureIdleDuration)
  }

  @MainActor
  func testIdleOnlyTimerExpiresPendingAndActiveInputWithOriginalReplay() async throws {
    for active in [false, true] {
      let source = GestureEventSourceDouble(startResult: true)
      var replays: [GestureReplayRequest] = []
      var matches = 0
      var matcher = GesturePatternMatcher<UUID>()
      matcher.register(pattern: [.right], match: UUID())
      let recognizer = GestureRecognizer<UUID>(
        eventSource: source,
        configuration: makeGestureRecognizerTestConfiguration(
          makeMatcher: { [matcher] _ in matcher }, onReplayRequested: { replays.append($0) },
          onMatch: { _ in matches += 1 },
          tuning: try .validated(
            maximumGestureSessionDuration: nil, maximumGestureIdleDuration: 0.01)))
      recognizer.start()
      _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
      if active {
        _ = source.send(
          makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 20, y: 0)))
      }
      await assertEventually(timeout: 1) {
        recognizer.snapshot.status.lastFailure == .gestureSessionExpired
      }
      XCTAssertEqual(matches, 0)
      XCTAssertEqual(
        replays,
        [
          active
            ? .release(button: .secondary, at: .init(x: 20, y: 0))
            : .click(button: .secondary, at: .zero)
        ])
      XCTAssertEqual(recognizer.inputGeneration, 0)
      XCTAssertFalse(recognizer.snapshot.trace.isVisible)
      XCTAssertEqual(
        source.send(makeGestureInputEvent(kind: .buttonUp(.secondary), location: .zero)),
        .passThrough)
      recognizer.stop()
    }
  }

  @MainActor
  func testDisablingIdleExpirationCancelsOldDeadlineAndStillCompletesMatch() async throws {
    let source = GestureEventSourceDouble(startResult: true)
    var matcher = GesturePatternMatcher<UUID>()
    matcher.register(pattern: [.right], match: UUID())
    var matches = 0
    let recognizer = GestureRecognizer<UUID>(
      eventSource: source,
      configuration: makeGestureRecognizerTestConfiguration(
        makeMatcher: { _ in matcher }, onMatch: { _ in matches += 1 },
        tuning: try .validated(
          maximumGestureSessionDuration: nil, maximumGestureIdleDuration: 0.01)))
    recognizer.start()
    _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    _ = source.send(
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 20, y: 0)))
    recognizer.updateTuning(try .validated(maximumGestureSessionDuration: nil))
    await assertNotEventually(timeout: 0.03) {
      recognizer.snapshot.status.lastFailure != nil
    }
    _ = source.send(
      makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 20, y: 0)))
    XCTAssertEqual(matches, 1)
    recognizer.stop()
  }

  @MainActor
  func testCancelStopAndReleaseDoNotLeaveAnIdleExpirationOrRetainRecognizer() async throws {
    let source = GestureEventSourceDouble(startResult: true)
    var replayCount = 0
    var recognizer: GestureRecognizer<UUID>? = GestureRecognizer<UUID>(
      eventSource: source,
      configuration: makeGestureRecognizerTestConfiguration(
        onReplayRequested: { _ in replayCount += 1 },
        tuning: try .validated(
          maximumGestureSessionDuration: nil, maximumGestureIdleDuration: 0.01)))
    recognizer?.isRecordingModeEnabled = true
    recognizer?.start()
    _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    recognizer?.cancelActiveGesture()
    XCTAssertEqual(replayCount, 1)
    _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    recognizer?.stop()
    XCTAssertEqual(replayCount, 2)
    await assertNotEventually(timeout: 0.03) { replayCount != 2 }
    weak var weakRecognizer = recognizer
    recognizer?.start()
    _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    recognizer = nil
    XCTAssertNil(weakRecognizer)
    weakRecognizer = nil
    XCTAssertEqual(replayCount, 3)
    await assertNotEventually(timeout: 0.03) { replayCount != 3 }
  }
}
