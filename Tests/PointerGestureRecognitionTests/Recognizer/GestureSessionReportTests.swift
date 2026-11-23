import XCTest

@testable import PointerGestureRecognition

@MainActor
final class GestureSessionReportTests: XCTestCase {
  func testReplacementPressRecoversOldInputBeforeResolvingNewContext() throws {
    let source = GestureEventSourceDouble(startResult: true)
    var order: [String] = []
    var recovery: [GestureReplayRequest] = []
    let recognizer = GestureRecognizer<Int>(
      eventSource: source,
      configuration: makeGestureRecognizerTestConfiguration(
        onReplayRequested: { request in
          order.append("recover")
          recovery.append(request)
        },
        recognitionContext: { _ in
          order.append("context")
          return nil
        },
        tuning: try .validated(maximumGestureSessionDuration: nil)))
    recognizer.isRecordingModeEnabled = true
    recognizer.start()
    let original = GesturePoint(x: 30, y: 0)
    XCTAssertEqual(
      source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero)), .consume)
    _ = source.send(makeGestureInputEvent(kind: .buttonMoved(.secondary), location: original))
    let originalGeneration = recognizer.inputGeneration
    let replacement = GesturePoint(x: 20, y: 40)
    XCTAssertEqual(
      source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: replacement)),
      .consume)

    let report = try XCTUnwrap(recognizer.snapshot.lastSessionReport)
    XCTAssertEqual(report.outcome, .superseded)
    XCTAssertEqual(report.generation, originalGeneration)
    XCTAssertEqual(report.directions, [.right])
    XCTAssertEqual(recovery, [.release(button: .secondary, at: original)])
    XCTAssertEqual(order, ["context", "recover", "context"])
    XCTAssertNotEqual(recognizer.inputGeneration, originalGeneration)
    XCTAssertFalse(recognizer.snapshot.trace.isVisible)

    recognizer.cancelActiveGesture()
    XCTAssertEqual(recognizer.snapshot.lastSessionReport?.outcome, .cancelled)
    XCTAssertEqual(recovery.last, .click(button: .secondary, at: replacement))
    recognizer.stop()
  }

  func testOwnedMatchReportSurvivesStopAndLaterInput() throws {
    let source = GestureEventSourceDouble(startResult: true)
    let clock = GestureRecognizerTestClock()
    let recognizer = GestureRecognizer<Int>(
      eventSource: source,
      configuration: makeGestureRecognizerTestConfiguration(
        makeMatcher: { _ in
          var matcher = GesturePatternMatcher<Int>()
          matcher.register(pattern: [.right], match: 0)
          return matcher
        }))
    recognizer.nowOverride = { clock.now }
    recognizer.start()
    XCTAssertNil(recognizer.snapshot.lastSessionReport)
    _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    clock.now = 10_000_000
    _ = source.send(
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 30, y: 0)))
    clock.now = 20_000_000
    _ = source.send(
      makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 30, y: 0)))
    let report = try XCTUnwrap(recognizer.snapshot.lastSessionReport)
    XCTAssertEqual(report.outcome, .matched)
    XCTAssertEqual(report.elapsedDuration, 0.02, accuracy: 0.000001)
    XCTAssertEqual(report.idleDuration, 0.01, accuracy: 0.000001)
    XCTAssertEqual(report.pointCount, 3)
    XCTAssertEqual(report.maximumAxisDistance, 30)
    XCTAssertEqual(report.directions, [.right])
    XCTAssertTrue(report.wasCaptured)
    XCTAssertFalse(report.wasRecording)
    XCTAssertNil(report.rejectedDirection)
    _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    XCTAssertEqual(recognizer.snapshot.lastSessionReport, report)
    recognizer.stop()
    XCTAssertEqual(recognizer.snapshot.lastSessionReport?.outcome, .stopped)
    XCTAssertNotEqual(recognizer.snapshot.lastSessionReport?.generation, report.generation)
    XCTAssertEqual(report.directions, [.right])
  }

  func testDirectionBudgetRecoversHeldInputWithoutAFalseMatch() throws {
    let source = GestureEventSourceDouble(startResult: true)
    var recovery: [GestureReplayRequest] = []
    let recognizer = GestureRecognizer<Int>(
      eventSource: source,
      configuration: makeGestureRecognizerTestConfiguration(
        onReplayRequested: { recovery.append($0) },
        tuning: try .validated(maximumGestureSessionDuration: nil, maximumDirectionCount: 1)))
    recognizer.isRecordingModeEnabled = true
    recognizer.start()
    _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    _ = source.send(
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 30, y: 0)))
    _ = source.send(
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 30, y: 30)))
    let report = try XCTUnwrap(recognizer.snapshot.lastSessionReport)
    XCTAssertEqual(report.outcome, .directionLimit)
    XCTAssertEqual(report.directions, [.right])
    XCTAssertEqual(report.rejectedDirection, .down)
    XCTAssertEqual(recovery.count, 1)
    if case .dragStart = recovery.first {} else { XCTFail("Expected held-input recovery") }
    XCTAssertFalse(recognizer.snapshot.trace.isVisible)
    XCTAssertEqual(
      source.send(
        makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 30, y: 30))),
      .passThrough)
    recognizer.stop()
    XCTAssertEqual(recognizer.snapshot.lastSessionReport, report)
    for invalid in [0, 4097, Int.max] {
      XCTAssertThrowsError(try GestureRecognizerTuning.validated(maximumDirectionCount: invalid))
    }
  }
}
