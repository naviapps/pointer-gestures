import PointerGestureRecognition
import XCTest

final class GestureRecognizerStateTests: XCTestCase {
  @MainActor
  func testSnapshotExposesObservableStateToClients() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    let configuration: GestureRecognizerConfiguration<String> =
      makeGestureRecognizerTestConfiguration()

    let recognizer = GestureRecognizer<String>(
      eventSource: eventSource,
      configuration: configuration
    )

    recognizer.isRecordingModeEnabled = true
    recognizer.start()

    let state = recognizer.snapshot
    XCTAssertEqual(state.status.lifecycle, .ready)
    XCTAssertFalse(state.status.isCapturingGesture)
    XCTAssertTrue(state.status.isRecordingModeEnabled)
    XCTAssertTrue(state.status.lastRecordedDirections.isEmpty)
    XCTAssertNil(state.status.lastFailure)
    XCTAssertFalse(state.trace.isVisible)
    XCTAssertTrue(state.trace.rawPoints.isEmpty)
    XCTAssertTrue(state.trace.directions.isEmpty)
    XCTAssertTrue(state.trace.directionEndpoints.isEmpty)
  }

  @MainActor
  func testSnapshotDefaultsToIdleObservableState() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    let configuration: GestureRecognizerConfiguration<String> =
      makeGestureRecognizerTestConfiguration()

    let recognizer = GestureRecognizer<String>(
      eventSource: eventSource,
      configuration: configuration
    )

    let state = recognizer.snapshot
    XCTAssertEqual(state.status.lifecycle, .idle)
    XCTAssertFalse(state.status.isCapturingGesture)
    XCTAssertFalse(state.status.isRecordingModeEnabled)
    XCTAssertTrue(state.status.lastRecordedDirections.isEmpty)
    XCTAssertNil(state.status.lastFailure)
    XCTAssertFalse(state.trace.isVisible)
    XCTAssertTrue(state.trace.rawPoints.isEmpty)
    XCTAssertTrue(state.trace.directions.isEmpty)
    XCTAssertTrue(state.trace.directionEndpoints.isEmpty)
  }

  func testTraceInitializerPreservesProvidedGeometry() {
    let rawPoints = [
      GesturePoint(x: 0, y: 0),
      GesturePoint(x: 10, y: 0),
      GesturePoint(x: 20, y: 0),
      GesturePoint(x: 20, y: 20),
    ]

    let trimmedTrace = GestureRecognizerState.Trace(
      rawPoints: rawPoints,
      directions: [.right, .down],
      directionEndpoints: rawPoints,
      tailPoint: rawPoints[3]
    )
    let paddedTrace = GestureRecognizerState.Trace(
      rawPoints: rawPoints,
      directions: [.right, .down],
      directionEndpoints: [rawPoints[0]],
      tailPoint: GesturePoint(x: 30, y: 20)
    )

    XCTAssertEqual(trimmedTrace.directionEndpoints, rawPoints)
    XCTAssertEqual(trimmedTrace.tailPoint, rawPoints[3])
    XCTAssertEqual(paddedTrace.directionEndpoints, [rawPoints[0]])
    XCTAssertEqual(paddedTrace.tailPoint, GesturePoint(x: 30, y: 20))
  }

  @MainActor
  func testPublishedSnapshotsKeepIndependentValuesAfterCompletionAndStop() {
    let source = GestureEventSourceDouble(startResult: true)
    let recognizer = GestureRecognizer<Int>(
      eventSource: source,
      configuration: .init(
        makeMatcher: { _ in .init() }, onReplayRequested: { _ in }, onMatch: { _ in }))
    var published: [GestureRecognizerState.Trace] = []
    let token = recognizer.observeTrace { published.append($0) }
    defer { token.cancel() }
    recognizer.isRecordingModeEnabled = true
    recognizer.start()
    _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    _ = source.send(
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 20, y: 0)))
    let active = recognizer.snapshot
    _ = source.send(
      makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 20, y: 30)))
    let completed = recognizer.snapshot
    recognizer.stop()

    let visible = published.filter(\.isVisible)
    XCTAssertEqual(visible.map(\.directions), [[.right], [.right, .down]])
    XCTAssertEqual(visible.first?.directionEndpoints, [.zero, .init(x: 20, y: 0)])
    XCTAssertEqual(
      visible.last?.directionEndpoints, [.zero, .init(x: 20, y: 0), .init(x: 20, y: 30)])
    XCTAssertEqual(active.trace.directions, [.right])
    XCTAssertTrue(active.status.lastRecordedDirections.isEmpty)
    XCTAssertEqual(completed.status.lastRecordedDirections, [.right, .down])
    XCTAssertFalse(completed.trace.isVisible)
    XCTAssertTrue(completed.trace.rawPoints.isEmpty)
    XCTAssertFalse(published.last?.isVisible ?? true)

    var editedCopy = active.trace.rawPoints
    editedCopy[0] = .init(x: 999, y: 999)
    XCTAssertEqual(editedCopy.first, .init(x: 999, y: 999))
    XCTAssertEqual(active.trace.rawPoints.first, .zero)
    XCTAssertEqual(visible.first?.rawPoints.first, .zero)
  }

}
