import PointerGestureRecognition
import XCTest

@MainActor
final class GestureRecognizerObservationTests: XCTestCase {
  func testObserveEmitsInitialReadyAndStoppedSnapshots() async {
    let eventSource = GestureEventSourceDouble(startResult: true)
    let recognizer = makeRecognizer(eventSource: eventSource)

    var snapshots: [GestureRecognizerState] = []
    let token = recognizer.observe { snapshots.append($0) }
    XCTAssertEqual(snapshots.map(\.status.lifecycle), [.idle])

    recognizer.start()
    await assertEventually(timeout: 0.2) { snapshots.contains { $0.status.lifecycle == .ready } }
    let snapshotCountAfterStart = snapshots.count

    recognizer.stop()
    await assertEventually(timeout: 0.2) {
      snapshots.count > snapshotCountAfterStart
        && snapshots.last?.status.lifecycle != .ready
        && snapshots.last?.status.isCapturingGesture == false
    }

    XCTAssertNotEqual(snapshots.last?.status.lifecycle, .ready)
    XCTAssertEqual(snapshots.last?.status.isCapturingGesture, false)
    XCTAssertEqual(snapshots.map(\.status.lifecycle), [.idle, .ready, .idle])

    _ = token
  }

  func testObserveSkipsUnchangedStateAssignments() async {
    let eventSource = GestureEventSourceDouble(startResult: true)
    let recognizer = makeRecognizer(eventSource: eventSource)

    var callCount = 0
    let token = recognizer.observe { _ in
      callCount += 1
    }
    XCTAssertEqual(callCount, 1)

    recognizer.isRecordingModeEnabled = false
    await assertNotEventually(timeout: 0.05) { callCount > 1 }
    XCTAssertEqual(callCount, 1)

    recognizer.isRecordingModeEnabled = true
    await assertEventually(timeout: 0.2) { callCount >= 2 }
    XCTAssertEqual(callCount, 2)

    recognizer.isRecordingModeEnabled = true
    await assertNotEventually(timeout: 0.05) { callCount > 2 }
    XCTAssertEqual(callCount, 2)

    _ = token
  }

  func testCancelObservationStopsNotifications() async {
    let eventSource = GestureEventSourceDouble(startResult: true)
    let recognizer = makeRecognizer(eventSource: eventSource)

    var callCount = 0
    let token = recognizer.observe { _ in
      callCount += 1
    }

    recognizer.start()
    await assertEventually(timeout: 0.2) { callCount >= 2 }
    XCTAssertGreaterThanOrEqual(callCount, 2)

    token.cancel()
    let countAfterCancel = callCount

    recognizer.stop()
    await assertNotEventually(timeout: 0.2) { callCount > countAfterCancel }

    XCTAssertEqual(callCount, countAfterCancel)
  }

  func testCancelingObservationDuringNotificationDoesNotInterruptDelivery() async {
    let eventSource = GestureEventSourceDouble(startResult: true)
    let recognizer = makeRecognizer(eventSource: eventSource)

    var firstToken: GestureObservationToken?
    var firstCallCount = 0
    var secondCallCount = 0

    firstToken = recognizer.observe { _ in
      firstCallCount += 1
      if firstCallCount > 1 {
        firstToken?.cancel()
      }
    }
    let secondToken = recognizer.observe { _ in
      secondCallCount += 1
    }

    recognizer.start()
    await assertEventually(timeout: 0.2) {
      firstCallCount >= 2 && secondCallCount >= 2
    }

    XCTAssertEqual(firstCallCount, 2)
    XCTAssertEqual(secondCallCount, 2)

    recognizer.stop()
    await assertEventually(timeout: 0.2) {
      secondCallCount >= 3
    }

    XCTAssertEqual(firstCallCount, 2)
    XCTAssertEqual(secondCallCount, 3)

    _ = secondToken
  }

  func testCancellationSkipsUnstartedSnapshotAndStatusCallbacks() async {
    for statusOnly in [false, true] {
      let eventSource = GestureEventSourceDouble(startResult: true)
      let recognizer = makeRecognizer(eventSource: eventSource)
      let tokens = ObservationTokens()
      var replacement: GestureObservationToken?
      var replacementLifecycles: [GestureRecognizerState.Status.Lifecycle] = []
      var originalDeliveries = 0
      var cancellationFinished = false
      var runningCallbackFinished = false

      let receive: @MainActor @Sendable (GestureRecognizerState.Status) -> Void = { status in
        guard status.lifecycle == .ready else { return }
        originalDeliveries += 1
        guard originalDeliveries == 1 else { return }
        for token in tokens.values { token.cancel() }
        cancellationFinished = true
        replacement = recognizer.observe { replacementLifecycles.append($0.status.lifecycle) }
        runningCallbackFinished = true
      }
      for _ in 0..<2 {
        tokens.values.append(
          statusOnly ? recognizer.observeStatus(receive) : recognizer.observe { receive($0.status) }
        )
      }

      recognizer.start()
      await assertEventually { cancellationFinished }
      XCTAssertEqual(originalDeliveries, 1, "statusOnly=\(statusOnly)")
      XCTAssertTrue(runningCallbackFinished)
      XCTAssertEqual(replacementLifecycles, [.ready])

      recognizer.stop()
      await assertEventually { replacementLifecycles == [.ready, .idle] }
      XCTAssertEqual(originalDeliveries, 1)
      withExtendedLifetime((tokens, replacement)) {}
    }
  }

  func testTraceCancellationSkipsUnstartedCallbacksAndPreservesReplacement() {
    let eventSource = GestureEventSourceDouble(startResult: true)
    let recognizer = makeRecognizer(eventSource: eventSource)
    recognizer.isRecordingModeEnabled = true
    recognizer.start()
    let tokens = ObservationTokens()
    var replacement: GestureObservationToken?
    var replacementTraces: [GestureRecognizerState.Trace] = []
    var originalDeliveries = 0
    var runningCallbackFinished = false
    for _ in 0..<2 {
      tokens.values.append(
        recognizer.observeTrace { trace in
          guard trace.isVisible else { return }
          originalDeliveries += 1
          guard originalDeliveries == 1 else { return }
          for token in tokens.values { token.cancel() }
          replacement = recognizer.observeTrace { replacementTraces.append($0) }
          runningCallbackFinished = true
        })
    }

    _ = eventSource.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    _ = eventSource.send(
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 40, y: 0)))
    XCTAssertEqual(originalDeliveries, 1)
    XCTAssertTrue(runningCallbackFinished)
    XCTAssertEqual(replacementTraces.map(\.tailPoint), [.init(x: 40, y: 0)])

    _ = eventSource.send(
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 80, y: 0)))
    XCTAssertEqual(replacementTraces.map(\.tailPoint), [.init(x: 40, y: 0), .init(x: 80, y: 0)])
    XCTAssertEqual(originalDeliveries, 1)
    recognizer.stop()
    XCTAssertEqual(replacementTraces.last?.isVisible, false)
    withExtendedLifetime((tokens, replacement)) {}
  }

  func testStateChangeDuringObservationSchedulesFollowUpNotification() async {
    let eventSource = GestureEventSourceDouble(startResult: true)
    let recognizer = makeRecognizer(eventSource: eventSource)

    var snapshots: [GestureRecognizerState] = []
    let token = recognizer.observe { state in
      snapshots.append(state)
      if state.status.lifecycle == .ready && !state.status.isRecordingModeEnabled {
        recognizer.isRecordingModeEnabled = true
      }
    }

    recognizer.start()
    await assertEventually(timeout: 0.2) {
      snapshots.contains { $0.status.isRecordingModeEnabled }
    }

    XCTAssertTrue(snapshots.contains { $0.status.lifecycle == .ready })
    XCTAssertTrue(snapshots.contains { $0.status.isRecordingModeEnabled })

    _ = token
  }

  func testObserverAddedDuringPendingNotificationDoesNotReceiveDuplicateCurrentSnapshot() async {
    let eventSource = GestureEventSourceDouble(startResult: true)
    let recognizer = makeRecognizer(eventSource: eventSource)

    var firstObserverSnapshots: [GestureRecognizerState] = []
    let firstToken = recognizer.observe { state in
      firstObserverSnapshots.append(state)
    }

    recognizer.start()

    var secondObserverSnapshots: [GestureRecognizerState] = []
    let secondToken = recognizer.observe { state in
      secondObserverSnapshots.append(state)
    }

    XCTAssertEqual(secondObserverSnapshots.map(\.status.lifecycle), [.ready])

    await assertEventually(timeout: 0.2) {
      firstObserverSnapshots.map(\.status.lifecycle) == [.idle, .ready]
    }
    await assertNotEventually(timeout: 0.05) {
      secondObserverSnapshots.count > 1
    }

    XCTAssertEqual(secondObserverSnapshots.map(\.status.lifecycle), [.ready])

    _ = firstToken
    _ = secondToken
  }

  func testDroppingObservationTokenStopsNotifications() async {
    let eventSource = GestureEventSourceDouble(startResult: true)
    let recognizer = makeRecognizer(eventSource: eventSource)

    var callCount = 0
    var token: GestureObservationToken? = recognizer.observe { _ in
      callCount += 1
    }
    XCTAssertNotNil(token)
    XCTAssertEqual(callCount, 1)

    token = nil
    await Task.yield()
    recognizer.start()
    await assertNotEventually(timeout: 0.2) { callCount > 1 }

    XCTAssertEqual(callCount, 1)
  }

  private func makeRecognizer(eventSource: GestureEventSourceDouble) -> GestureRecognizer<UUID> {
    GestureRecognizer<UUID>(
      eventSource: eventSource,
      configuration: makeGestureRecognizerTestConfiguration()
    )
  }
}

@MainActor
private final class ObservationTokens {
  var values: [GestureObservationToken] = []
}
