import PointerGestureRecognition
import XCTest

@MainActor
final class GestureRecognizerReplayTests: XCTestCase {
  func testStoppedUnmatchedGestureReplaysConsumedDragStartAndReleasesSession() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    var replayRequests: [GestureReplayRequest] = []

    let configuration: GestureRecognizerConfiguration<UUID> =
      makeGestureRecognizerTestConfiguration(
        makeMatcher: { _ in
          var matcher = GesturePatternMatcher<UUID>()
          matcher.register(pattern: [.left], match: UUID())
          return matcher
        },
        onReplayRequested: { replayRequests.append($0) },
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
        makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 40, y: 10))
      ),
      .consume
    )

    XCTAssertFalse(recognizer.snapshot.status.isCapturingGesture)
    XCTAssertFalse(recognizer.snapshot.trace.isVisible)
    XCTAssertEqual(
      replayRequests,
      [
        .dragStart(
          button: .secondary,
          points: [.init(x: 10, y: 10), .init(x: 40, y: 10)]
        )
      ]
    )

    XCTAssertEqual(
      eventSource.send(
        makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 45, y: 10))
      ),
      .passThrough
    )
    XCTAssertEqual(
      replayRequests,
      [
        .dragStart(
          button: .secondary,
          points: [.init(x: 10, y: 10), .init(x: 40, y: 10)]
        )
      ]
    )
  }

  func testUnmatchedGestureRequestsDragStartReplay() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    var replayRequests: [GestureReplayRequest] = []

    let configuration: GestureRecognizerConfiguration<UUID> =
      makeGestureRecognizerTestConfiguration(
        onReplayRequested: { replayRequests.append($0) },
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
    XCTAssertFalse(recognizer.snapshot.status.isCapturingGesture)
    XCTAssertFalse(recognizer.snapshot.trace.isVisible)

    let up = makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 120, y: 10))
    XCTAssertEqual(eventSource.send(up), .passThrough)

    XCTAssertEqual(
      replayRequests,
      [
        .dragStart(
          button: .secondary,
          points: [.init(x: 10, y: 10), .init(x: 100, y: 10)]
        )
      ]
    )
  }

  func testUnmatchedGestureReleasesSessionAfterDragStartReplay() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    var replayRequests: [GestureReplayRequest] = []

    let configuration: GestureRecognizerConfiguration<UUID> =
      makeGestureRecognizerTestConfiguration(
        onReplayRequested: { replayRequests.append($0) },
        areModifiersSatisfied: { _, _ in true },
        tuning: .testing(
          minimumGestureStartAxisDistance: 0,
          minimumDirectionChangeAxisDistance: 0,
          maximumRawPointCount: 3
        )
      )

    let recognizer = GestureRecognizer<UUID>(
      eventSource: eventSource,
      configuration: configuration
    )

    recognizer.start()
    _ = eventSource.send(
      makeGestureInputEvent(kind: .buttonDown(.secondary), location: .init(x: 0, y: 0))
    )

    for point in [
      GesturePoint(x: 20, y: 0),
      GesturePoint(x: 40, y: 0),
      GesturePoint(x: 60, y: 0),
      GesturePoint(x: 80, y: 0),
    ] {
      XCTAssertEqual(
        eventSource.send(makeGestureInputEvent(kind: .buttonMoved(.secondary), location: point)),
        point == GesturePoint(x: 20, y: 0) ? .consume : .passThrough
      )
    }
    XCTAssertEqual(
      eventSource.send(
        makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 100, y: 0))
      ),
      .passThrough
    )

    XCTAssertEqual(
      replayRequests,
      [
        .dragStart(
          button: .secondary,
          points: [.init(x: 0, y: 0), .init(x: 20, y: 0)]
        )
      ]
    )
  }

  func testUnmatchedPrimaryButtonGestureRequestsPrimaryButtonDragStartReplay() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    var replayRequests: [GestureReplayRequest] = []

    let configuration: GestureRecognizerConfiguration<UUID> =
      makeGestureRecognizerTestConfiguration(
        onReplayRequested: { replayRequests.append($0) },
        recognitionButton: .primary,
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
      makeGestureInputEvent(kind: .buttonDown(.primary), location: .init(x: 10, y: 10))
    )
    XCTAssertEqual(
      eventSource.send(
        makeGestureInputEvent(kind: .buttonMoved(.primary), location: .init(x: 100, y: 10))
      ),
      .consume
    )
    XCTAssertEqual(
      eventSource.send(
        makeGestureInputEvent(kind: .buttonUp(.primary), location: .init(x: 120, y: 10))
      ),
      .passThrough
    )

    XCTAssertEqual(
      replayRequests,
      [
        .dragStart(
          button: .primary,
          points: [.init(x: 10, y: 10), .init(x: 100, y: 10)]
        )
      ]
    )
  }

  func testIncompletePatternRequestsButtonDragReplay() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    var replayRequests: [GestureReplayRequest] = []

    let configuration: GestureRecognizerConfiguration<UUID> =
      makeGestureRecognizerTestConfiguration(
        makeMatcher: { _ in
          var matcher = GesturePatternMatcher<UUID>()
          matcher.register(pattern: [.right, .down], match: UUID())
          return matcher
        },
        onReplayRequested: { replayRequests.append($0) },
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
    XCTAssertEqual(eventSource.send(up), .consume)

    XCTAssertEqual(
      replayRequests,
      [
        .drag(
          button: .secondary,
          points: [.init(x: 10, y: 10), .init(x: 100, y: 10), .init(x: 120, y: 10)]
        )
      ]
    )
  }

  func testEqualAxisDragStaysPendingUntilButtonDragReplay() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    var replayRequests: [GestureReplayRequest] = []

    let configuration: GestureRecognizerConfiguration<UUID> =
      makeGestureRecognizerTestConfiguration(
        onReplayRequested: { replayRequests.append($0) },
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
      makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero)
    )
    XCTAssertEqual(
      eventSource.send(
        makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 40, y: 40))),
      .consume
    )
    XCTAssertFalse(recognizer.snapshot.status.isCapturingGesture)
    XCTAssertEqual(recognizer.snapshot.trace.directions, [])

    XCTAssertEqual(
      eventSource.send(
        makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 40, y: 40))),
      .consume
    )

    XCTAssertEqual(
      replayRequests,
      [.drag(button: .secondary, points: [.zero, .init(x: 40, y: 40)])]
    )
  }

  func testDominantDragAfterEqualAxisDragRequestsButtonReleaseReplay() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    var replayRequests: [GestureReplayRequest] = []

    let matchedID = UUID()
    let configuration: GestureRecognizerConfiguration<UUID> =
      makeGestureRecognizerTestConfiguration(
        makeMatcher: { _ in
          var matcher = GesturePatternMatcher<UUID>()
          matcher.register(pattern: [.right], match: matchedID)
          return matcher
        },
        onReplayRequested: { replayRequests.append($0) },
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
      makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero)
    )
    XCTAssertEqual(
      eventSource.send(
        makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 40, y: 40))),
      .consume
    )
    XCTAssertFalse(recognizer.snapshot.status.isCapturingGesture)

    XCTAssertEqual(
      eventSource.send(
        makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 80, y: 40))),
      .consume
    )
    XCTAssertEqual(recognizer.snapshot.trace.directions, [.right])

    XCTAssertEqual(
      eventSource.send(
        makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 100, y: 40))),
      .consume
    )
    XCTAssertEqual(replayRequests, [.release(button: .secondary, at: .init(x: 100, y: 40))])
  }

  func testExtremeFiniteEndpointsAndMeasurementReachTypedSnapshots() throws {
    let eventSource = GestureEventSourceDouble(startResult: true)
    let recognizer = GestureRecognizer<Int>(
      eventSource: eventSource,
      configuration: makeGestureRecognizerTestConfiguration(
        makeMatcher: { _ in
          var matcher = GesturePatternMatcher<Int>()
          matcher.register(pattern: [.right], match: 42)
          return matcher
        },
        tuning: .testing(maximumGestureSessionDuration: nil)))
    recognizer.start()

    let maximum = Double.greatestFiniteMagnitude
    let start = GesturePoint(x: -maximum, y: -maximum)
    let end = GesturePoint(x: maximum, y: maximum / 2)
    _ = eventSource.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: start))
    _ = eventSource.send(makeGestureInputEvent(kind: .buttonMoved(.secondary), location: end))
    let active = recognizer.snapshot.trace
    _ = eventSource.send(makeGestureInputEvent(kind: .buttonUp(.secondary), location: end))
    let report = recognizer.snapshot.lastSessionReport
    recognizer.stop()

    XCTAssertEqual(active.directionEndpoints, [start, end])
    XCTAssertEqual(try XCTUnwrap(report).maximumAxisDistance, maximum)
  }

  func testInvalidGestureRequestsDragStartReplayBeforeFurtherInput() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    var replayRequests: [GestureReplayRequest] = []

    let configuration: GestureRecognizerConfiguration<UUID> =
      makeGestureRecognizerTestConfiguration(
        makeMatcher: { _ in
          var matcher = GesturePatternMatcher<UUID>()
          matcher.register(pattern: [.right], match: UUID())
          return matcher
        },
        onReplayRequested: { replayRequests.append($0) },
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

    for event in [
      makeGestureInputEvent(kind: .buttonDown(.secondary), location: .init(x: 10, y: 10)),
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 10, y: 60)),
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 10, y: 80)),
    ] {
      _ = eventSource.send(event)
    }
    let result = eventSource.send(
      makeGestureInputEvent(
        kind: .buttonUp(.secondary),
        location: .init(x: 10, y: 100)
      ))

    XCTAssertEqual(result, .passThrough)
    XCTAssertEqual(
      replayRequests,
      [
        .dragStart(
          button: .secondary,
          points: [
            .init(x: 10, y: 10), .init(x: 10, y: 60),
          ]
        )
      ]
    )
    XCTAssertFalse(recognizer.snapshot.status.isCapturingGesture)
    XCTAssertFalse(recognizer.snapshot.trace.isVisible)
  }
}
