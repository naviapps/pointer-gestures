import PointerGestureRecognition
import XCTest

final class GestureRecognizerTuningTests: XCTestCase {
  func testValidatedPreservesCustomValues() throws {
    let tuning = try GestureRecognizerTuning.validated(
      minimumGestureStartAxisDistance: 1.5,
      minimumDirectionChangeAxisDistance: 2.5,
      maximumGestureSessionDuration: nil,
      maximumRawPointCount: 42,
      eventSourceStartRetryDelays: [0, 0.125, 3]
    )

    XCTAssertEqual(tuning.minimumGestureStartAxisDistance, 1.5)
    XCTAssertEqual(tuning.minimumDirectionChangeAxisDistance, 2.5)
    XCTAssertNil(tuning.maximumGestureSessionDuration)
    XCTAssertEqual(tuning.maximumRawPointCount, 42)
    XCTAssertEqual(tuning.eventSourceStartRetryDelays, [0, 0.125, 3])
  }

  func testValidatedRejectsInvalidDistances() {
    XCTAssertThrowsError(
      try GestureRecognizerTuning.validated(minimumGestureStartAxisDistance: -1)
    ) { error in
      XCTAssertEqual(
        error as? GestureRecognizerTuning.ValidationError,
        .invalidMinimumGestureStartAxisDistance
      )
    }
    XCTAssertThrowsError(
      try GestureRecognizerTuning.validated(minimumDirectionChangeAxisDistance: .nan)
    ) { error in
      XCTAssertEqual(
        error as? GestureRecognizerTuning.ValidationError,
        .invalidMinimumDirectionChangeAxisDistance
      )
    }
  }

  func testValidatedRejectsInvalidDurationAndPointCount() {
    XCTAssertThrowsError(
      try GestureRecognizerTuning.validated(maximumGestureSessionDuration: 0)
    ) { error in
      XCTAssertEqual(
        error as? GestureRecognizerTuning.ValidationError,
        .invalidMaximumGestureSessionDuration
      )
    }
    XCTAssertThrowsError(try GestureRecognizerTuning.validated(maximumRawPointCount: 0)) { error in
      XCTAssertEqual(
        error as? GestureRecognizerTuning.ValidationError,
        .invalidMaximumRawPointCount
      )
    }
  }

  func testValidatedReportsInvalidRetryDelayIndex() {
    XCTAssertThrowsError(
      try GestureRecognizerTuning.validated(eventSourceStartRetryDelays: [0.1, .infinity])
    ) { error in
      XCTAssertEqual(
        error as? GestureRecognizerTuning.ValidationError,
        .invalidEventSourceStartRetryDelay(index: 1)
      )
    }
  }

  func testValidatedPreservesRetryOrderDuplicatesAndZero() throws {
    let tuning = try GestureRecognizerTuning.validated(
      eventSourceStartRetryDelays: [1, 0.25, 1, 0]
    )
    XCTAssertEqual(tuning.eventSourceStartRetryDelays, [1, 0.25, 1, 0])
  }

  func testStandardValuesAreExpected() {
    let tuning = GestureRecognizerTuning.standard
    XCTAssertEqual(tuning.minimumGestureStartAxisDistance, 10)
    XCTAssertEqual(tuning.minimumDirectionChangeAxisDistance, 25)
    XCTAssertEqual(tuning.maximumGestureSessionDuration, 1)
    XCTAssertEqual(tuning.maximumRawPointCount, 256)
    XCTAssertEqual(tuning.eventSourceStartRetryDelays, [0.25, 0.5, 1, 2, 4])
  }

  @MainActor
  func testUpdateTuningAppliesToFutureGestureSessions() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    var matcher = GesturePatternMatcher<UUID>()
    matcher.register(pattern: [.right], match: UUID())
    let recognizer = GestureRecognizer<UUID>(
      eventSource: eventSource,
      configuration: makeGestureRecognizerTestConfiguration(
        makeMatcher: { _ in matcher },
        tuning: .testing(
          minimumGestureStartAxisDistance: 100,
          minimumDirectionChangeAxisDistance: 0
        )
      )
    )

    recognizer.start()
    _ = eventSource.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    _ = eventSource.send(
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 50, y: 0)))
    XCTAssertFalse(recognizer.snapshot.status.isCapturingGesture)

    recognizer.cancelActiveGesture()
    recognizer.updateTuning(
      .testing(
        minimumGestureStartAxisDistance: 10,
        minimumDirectionChangeAxisDistance: 0
      ))
    _ = eventSource.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    _ = eventSource.send(
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 50, y: 0)))

    XCTAssertTrue(recognizer.snapshot.status.isCapturingGesture)
    XCTAssertEqual(recognizer.snapshot.trace.directions, [.right])
  }

  @MainActor
  func testUpdateTuningReportsWhetherTuningChanged() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    let initialTuning = GestureRecognizerTuning.testing(
      minimumGestureStartAxisDistance: 100,
      minimumDirectionChangeAxisDistance: 0
    )
    let recognizer = GestureRecognizer<UUID>(
      eventSource: eventSource,
      configuration: makeGestureRecognizerTestConfiguration(tuning: initialTuning)
    )

    XCTAssertFalse(recognizer.updateTuning(initialTuning))
    XCTAssertTrue(
      recognizer.updateTuning(
        .testing(
          minimumGestureStartAxisDistance: 10,
          minimumDirectionChangeAxisDistance: 0
        )))
  }

  @MainActor
  func testTuningUpdatePublishesAnOwnedTraceAndDeliversReplay() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    var replays: [GestureReplayRequest] = []
    let recognizer = GestureRecognizer<UUID>(
      eventSource: eventSource,
      configuration: makeGestureRecognizerTestConfiguration(
        onReplayRequested: { replays.append($0) },
        tuning: .testing(maximumGestureSessionDuration: nil, maximumRawPointCount: 8)))
    recognizer.isRecordingModeEnabled = true
    recognizer.start()
    defer { recognizer.stop() }
    _ = eventSource.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    for x in [20.0, 22.0] {
      _ = eventSource.send(
        makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: x, y: 0)))
    }
    let before = recognizer.snapshot.trace
    let tail = GesturePoint(x: 22, y: 0)
    XCTAssertTrue(
      recognizer.updateTuning(.testing(maximumGestureSessionDuration: nil, maximumRawPointCount: 1))
    )
    XCTAssertEqual(recognizer.snapshot.trace.rawPoints, [tail])
    XCTAssertEqual(before.rawPoints, [.zero, .init(x: 20, y: 0)])

    assertDisposition(
      eventSource.send(makeGestureInputEvent(kind: .buttonUp(.secondary), location: tail)), .consume
    )
    assertReplayRequests(replays, [.drag(button: .secondary, points: [.zero, tail])])
  }
}
