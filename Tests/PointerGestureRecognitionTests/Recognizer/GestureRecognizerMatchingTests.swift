import PointerGestureRecognition
import XCTest

@MainActor
final class GestureRecognizerMatchingTests: XCTestCase {
  func testDefaultEmptyMatcherPassesThroughInputAndAllowsRecording() throws {
    let eventSource = GestureEventSourceDouble(startResult: true)
    var matcherRequests = 0
    var replayRequests: [GestureReplayRequest] = []
    let recognizer = GestureRecognizer<UUID>(
      eventSource: eventSource,
      configuration: .init(
        makeMatcher: { _ in
          matcherRequests += 1
          return .init()
        },
        onReplayRequested: { replayRequests.append($0) },
        onMatch: { _ in XCTFail("An empty matcher cannot produce a match") },
        tuning: try .validated(maximumGestureSessionDuration: nil)))
    recognizer.start()
    defer { recognizer.stop() }

    let end = GesturePoint(x: 40, y: 0)
    func drag(_ expected: GestureEventDisposition) {
      XCTAssertEqual(
        eventSource.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero)),
        expected)
      XCTAssertEqual(
        eventSource.send(makeGestureInputEvent(kind: .buttonMoved(.secondary), location: end)),
        expected)
      XCTAssertEqual(
        eventSource.send(makeGestureInputEvent(kind: .buttonUp(.secondary), location: end)),
        expected)
    }

    drag(.passThrough)
    XCTAssertFalse(recognizer.snapshot.status.isCapturingGesture)
    XCTAssertFalse(recognizer.snapshot.trace.isVisible)
    XCTAssertNil(recognizer.snapshot.lastSessionReport)
    XCTAssertTrue(replayRequests.isEmpty)

    recognizer.isRecordingModeEnabled = true
    drag(.consume)
    XCTAssertEqual(matcherRequests, 1)
    XCTAssertEqual(recognizer.snapshot.status.lastRecordedDirections, [.right])
    XCTAssertEqual(recognizer.snapshot.lastSessionReport?.outcome, .recorded)
    XCTAssertEqual(replayRequests, [.drag(button: .secondary, points: [.zero, end])])
  }

  func testMatchCallsOnMatchAndConsumesButtonUp() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    var releaseRequests: [GesturePoint] = []

    let matchedID = UUID()
    let makeMatcher: @MainActor (GestureRecognitionContext?) -> GesturePatternMatcher<UUID> = {
      _ in
      var matcher = GesturePatternMatcher<UUID>()
      matcher.register(pattern: [.right], match: matchedID)
      return matcher
    }

    var calledIDs: [UUID] = []
    var clickRequestCount = 0
    let configuration: GestureRecognizerConfiguration<UUID> =
      makeGestureRecognizerTestConfiguration(
        makeMatcher: makeMatcher,
        onReplayRequested: { request in
          switch request {
          case .click:
            clickRequestCount += 1
          case .drag, .dragStart:
            XCTFail("Matched gestures should not replay the consumed sequence.")
          case .release(_, let point):
            releaseRequests.append(point)
          }
        },
        onMatch: { calledIDs.append($0) },
        areModifiersSatisfied: { _, _ in true },
        tuning: .testing(
          minimumGestureStartAxisDistance: 0,
          minimumDirectionChangeAxisDistance: 0
        )
      )

    let recognizer = GestureRecognizer<UUID>(
      eventSource: eventSource,
      configuration: configuration
    )

    recognizer.start()

    let down = makeGestureInputEvent(kind: .buttonDown(.secondary), location: .init(x: 10, y: 10))
    _ = eventSource.send(down)

    let drag = makeGestureInputEvent(
      kind: .buttonMoved(.secondary), location: .init(x: 100, y: 10))
    XCTAssertEqual(eventSource.send(drag), .consume)

    let up = makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 120, y: 10))
    let result = eventSource.send(up)

    XCTAssertEqual(result, .consume)
    XCTAssertEqual(calledIDs, [matchedID])
    XCTAssertEqual(clickRequestCount, 0)
    XCTAssertEqual(releaseRequests, [.init(x: 120, y: 10)])
  }

  func testMatchedGestureRequestsReplayBeforeMatchCallback() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    let matchedID = UUID()
    var callbacks: [String] = []

    let configuration: GestureRecognizerConfiguration<UUID> =
      makeGestureRecognizerTestConfiguration(
        makeMatcher: { _ in
          var matcher = GesturePatternMatcher<UUID>()
          matcher.register(pattern: [.right], match: matchedID)
          return matcher
        },
        onReplayRequested: { _ in callbacks.append("replay") },
        onMatch: { _ in callbacks.append("match") },
        areModifiersSatisfied: { _, _ in true },
        tuning: .testing(
          minimumGestureStartAxisDistance: 0,
          minimumDirectionChangeAxisDistance: 0
        )
      )

    let recognizer = GestureRecognizer<UUID>(
      eventSource: eventSource,
      configuration: configuration
    )

    recognizer.start()
    _ = eventSource.send(
      makeGestureInputEvent(kind: .buttonDown(.secondary), location: .init(x: 10, y: 10))
    )
    XCTAssertEqual(
      eventSource.send(
        makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 100, y: 10))
      ),
      .consume
    )
    XCTAssertEqual(
      eventSource.send(
        makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 120, y: 10))
      ),
      .consume
    )

    XCTAssertEqual(callbacks, ["replay", "match"])
  }

  func testRecognitionSessionKeepsReleaseOnlyReplayWhenRecordingModeIsEnabledMidGesture() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    let matchedID = UUID()
    var calledIDs: [UUID] = []
    var replayRequests: [GestureReplayRequest] = []

    let configuration: GestureRecognizerConfiguration<UUID> =
      makeGestureRecognizerTestConfiguration(
        makeMatcher: { _ in
          var matcher = GesturePatternMatcher<UUID>()
          matcher.register(pattern: [.right], match: matchedID)
          return matcher
        },
        onReplayRequested: { replayRequests.append($0) },
        onMatch: { calledIDs.append($0) },
        areModifiersSatisfied: { _, _ in true },
        tuning: .testing(
          minimumGestureStartAxisDistance: 0,
          minimumDirectionChangeAxisDistance: 0
        )
      )

    let recognizer = GestureRecognizer<UUID>(
      eventSource: eventSource,
      configuration: configuration
    )

    recognizer.start()
    _ = eventSource.send(
      makeGestureInputEvent(kind: .buttonDown(.secondary), location: .init(x: 10, y: 10))
    )
    _ = eventSource.send(
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 100, y: 10))
    )

    recognizer.isRecordingModeEnabled = true

    _ = eventSource.send(
      makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 120, y: 10))
    )

    XCTAssertEqual(calledIDs, [matchedID])
    XCTAssertEqual(replayRequests, [.release(button: .secondary, at: .init(x: 120, y: 10))])
  }

  func testMatchCallbackReceivesCompletedSnapshot() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    let matchedID = UUID()
    let snapshotProbe = MatchingSnapshotProbe()

    let configuration: GestureRecognizerConfiguration<UUID> =
      makeGestureRecognizerTestConfiguration(
        makeMatcher: { _ in
          var matcher = GesturePatternMatcher<UUID>()
          matcher.register(pattern: [.right], match: matchedID)
          return matcher
        },
        onMatch: { _ in
          snapshotProbe.record()
        },
        areModifiersSatisfied: { _, _ in true },
        tuning: .testing(
          minimumGestureStartAxisDistance: 0,
          minimumDirectionChangeAxisDistance: 0
        )
      )

    let recognizer = GestureRecognizer<UUID>(
      eventSource: eventSource,
      configuration: configuration
    )
    snapshotProbe.recognizer = recognizer

    recognizer.start()
    _ = eventSource.send(
      makeGestureInputEvent(kind: .buttonDown(.secondary), location: .init(x: 10, y: 10))
    )
    _ = eventSource.send(
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 100, y: 10))
    )
    XCTAssertEqual(
      eventSource.send(
        makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 120, y: 10))
      ),
      .consume
    )

    XCTAssertEqual(snapshotProbe.snapshot?.status.lastRecordedDirections, [.right])
    XCTAssertEqual(snapshotProbe.snapshot?.status.isCapturingGesture, false)
    XCTAssertEqual(snapshotProbe.snapshot?.trace.isVisible, false)
  }

}

@MainActor
private final class MatchingSnapshotProbe {
  var recognizer: GestureRecognizer<UUID>?
  var snapshot: GestureRecognizerState?

  func record() {
    snapshot = recognizer?.snapshot
  }
}
