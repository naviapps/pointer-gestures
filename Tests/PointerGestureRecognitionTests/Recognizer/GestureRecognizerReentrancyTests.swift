import XCTest

@testable import PointerGestureRecognition

@MainActor
final class GestureRecognizerReentrancyTests: XCTestCase {
  func testStopDuringSourceStartupCannotResurrectTheRecognizer() {
    for succeeds in [true, false] {
      let source = GestureEventSourceDouble(startResult: succeeds)
      let recognizer = GestureRecognizer<UUID>(
        eventSource: source, configuration: makeGestureRecognizerTestConfiguration())
      source.onStart = { [weak recognizer] in recognizer?.stop() }
      recognizer.start()
      XCTAssertEqual(recognizer.snapshot.status.lifecycle, .idle)
      XCTAssertNil(recognizer.snapshot.status.lastFailure)
      XCTAssertNil(recognizer.startupRetryTask)
      recognizer.stop()
    }
  }

  func testRestartFromStopRecoveryKeepsTheNewSourceRunning() {
    let source = GestureEventSourceDouble(startResult: true)
    weak var current: GestureRecognizer<UUID>?
    let recognizer = GestureRecognizer<UUID>(
      eventSource: source,
      configuration: makeGestureRecognizerTestConfiguration(onReplayRequested: { _ in
        current?.start()
      }))
    current = recognizer
    recognizer.isRecordingModeEnabled = true
    recognizer.start()
    _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    recognizer.stop()
    XCTAssertEqual(recognizer.snapshot.status.lifecycle, .ready)
    XCTAssertEqual(
      source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero)),
      .consume)
    current = nil
    recognizer.stop()
  }

  func testStopFromInitialTraceDeliveryDoesNotLeaveTheNewObserverVisible() {
    let source = GestureEventSourceDouble(startResult: true)
    let recognizer = GestureRecognizer<UUID>(
      eventSource: source, configuration: makeGestureRecognizerTestConfiguration())
    recognizer.isRecordingModeEnabled = true
    recognizer.start()
    _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    _ = source.send(
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 20, y: 0)))
    var visibility: [Bool] = []
    let token = recognizer.observeTrace { [weak recognizer] trace in
      visibility.append(trace.isVisible)
      if trace.isVisible { recognizer?.stop() }
    }
    XCTAssertEqual(visibility, [true, false])
    XCTAssertFalse(recognizer.snapshot.trace.isVisible)
    token.cancel()
  }

  func testStopOrCancelFromFinalVisibleTraceSuppressesMatchAndHidesTrace() {
    for stop in [false, true] {
      let source = GestureEventSourceDouble(startResult: true)
      var matcher = GesturePatternMatcher<UUID>()
      matcher.register(pattern: [.right], match: UUID())
      var matches = 0
      var replays: [GestureReplayRequest] = []
      var visible: [Bool] = []
      let recognizer = GestureRecognizer<UUID>(
        eventSource: source,
        configuration: makeGestureRecognizerTestConfiguration(
          makeMatcher: { [matcher] _ in matcher }, onReplayRequested: { replays.append($0) },
          onMatch: { _ in matches += 1 }))
      let token = recognizer.observeTrace { [weak recognizer] trace in
        visible.append(trace.isVisible)
        if trace.isVisible && trace.tailPoint?.x == 30 {
          if stop { recognizer?.stop() } else { recognizer?.cancelActiveGesture() }
        }
      }
      recognizer.start()
      _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
      _ = source.send(
        makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 20, y: 0)))
      _ = source.send(
        makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 30, y: 0)))
      XCTAssertEqual(matches, 0)
      XCTAssertEqual(visible.last, false)
      XCTAssertEqual(replays, [.release(button: .secondary, at: .init(x: 30, y: 0))])
      XCTAssertFalse(recognizer.snapshot.trace.isVisible)
      token.cancel()
      recognizer.stop()
    }
  }

  func testExpirationReplayStopPreventsFurtherHostPolicyCalls() throws {
    let source = GestureEventSourceDouble(startResult: true)
    weak var current: GestureRecognizer<UUID>?
    let clock = GestureRecognizerTestClock()
    var contexts = 0
    let recognizer = GestureRecognizer<UUID>(
      eventSource: source,
      configuration: makeGestureRecognizerTestConfiguration(
        onReplayRequested: { _ in current?.stop() },
        recognitionContext: { _ in
          contexts += 1
          return nil
        },
        tuning: try .validated(maximumGestureSessionDuration: 1)))
    current = recognizer
    recognizer.nowOverride = { clock.now }
    recognizer.isRecordingModeEnabled = true
    recognizer.start()
    _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    clock.now = 2_000_000_000
    XCTAssertEqual(
      source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero)),
      .passThrough)
    XCTAssertEqual(contexts, 1)
    XCTAssertEqual(recognizer.snapshot.status.lifecycle, .idle)
    XCTAssertEqual(recognizer.inputGeneration, 0)
  }

  func testCancelAtDeadlinePreservesInputStartedDuringRecovery() throws {
    for finiteLocation in [false, true] {
      for idleDeadline in [false, true] {
        for originalActive in [false, true] {
          for replacementActive in [false, true] {
            let scenario =
              "finite=\(finiteLocation), idle=\(idleDeadline), originalActive=\(originalActive), replacementActive=\(replacementActive)"
            let source = GestureEventSourceDouble(startResult: true)
            let clock = GestureRecognizerTestClock()
            weak var current: GestureRecognizer<String>?
            var replays: [GestureReplayRequest] = []
            var matches: [String] = []
            var replacementGeneration: UInt64 = 0
            var replacementDeadline: UInt64?
            var replacementState: GestureRecognizerState?
            let recognizer = GestureRecognizer<String>(
              eventSource: source,
              configuration: makeGestureRecognizerTestConfiguration(
                makeMatcher: { _ in
                  var matcher = GesturePatternMatcher<String>()
                  matcher.register(pattern: [.right], match: "right")
                  return matcher
                },
                onReplayRequested: { request in
                  replays.append(request)
                  guard replays.count == 1, let recognizer = current else { return }
                  XCTAssertEqual(
                    source.send(
                      makeGestureInputEvent(
                        kind: .buttonDown(.secondary), location: .init(x: 100, y: 0))),
                    .consume)
                  if replacementActive {
                    XCTAssertEqual(
                      source.send(
                        makeGestureInputEvent(
                          kind: .buttonMoved(.secondary), location: .init(x: 140, y: 0))),
                      .consume)
                  }
                  replacementGeneration = recognizer.inputGeneration
                  replacementDeadline = recognizer.nextDeadline
                  replacementState = recognizer.snapshot
                },
                onMatch: { matches.append($0) },
                tuning: try .validated(
                  maximumGestureSessionDuration: idleDeadline ? nil : 1,
                  maximumGestureIdleDuration: idleDeadline ? 1 : nil)))
            current = recognizer
            recognizer.nowOverride = { clock.now }
            recognizer.start()
            defer { recognizer.stop() }
            _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
            if originalActive {
              clock.now = 250_000_000
              _ = source.send(
                makeGestureInputEvent(
                  kind: .buttonMoved(.secondary), location: .init(x: 40, y: 0)))
            }
            let originalGeneration = recognizer.inputGeneration
            clock.now = try XCTUnwrap(recognizer.nextDeadline)
            let disposition = source.send(
              makeCancelGestureInputEvent(
                location: finiteLocation ? .zero : .init(x: .nan, y: .infinity)))

            // Expiration ends the original input before its recovery callback starts another.
            XCTAssertEqual(disposition, .passThrough, scenario)
            let originalRecovery: GestureReplayRequest =
              originalActive
              ? .release(button: .secondary, at: .init(x: 40, y: 0))
              : .click(button: .secondary, at: .zero)
            XCTAssertEqual(replays, [originalRecovery], scenario)
            XCTAssertNotEqual(replacementGeneration, 0, scenario)
            XCTAssertNotEqual(replacementGeneration, originalGeneration, scenario)
            XCTAssertEqual(recognizer.inputGeneration, replacementGeneration, scenario)
            XCTAssertNotNil(replacementDeadline, scenario)
            XCTAssertEqual(recognizer.nextDeadline, replacementDeadline, scenario)
            XCTAssertEqual(recognizer.snapshot, replacementState, scenario)
            XCTAssertEqual(
              recognizer.snapshot.status.isCapturingGesture, replacementActive, scenario)
            XCTAssertEqual(recognizer.snapshot.trace.isVisible, replacementActive, scenario)
            XCTAssertTrue(matches.isEmpty, scenario)

            // Only the replacement's own release may complete or recover it.
            XCTAssertEqual(
              source.send(
                makeGestureInputEvent(
                  kind: .buttonUp(.secondary),
                  location: .init(x: replacementActive ? 180 : 100, y: 0))),
              .consume)
            let replacementRecovery: GestureReplayRequest =
              replacementActive
              ? .release(button: .secondary, at: .init(x: 180, y: 0))
              : .click(button: .secondary, at: .init(x: 100, y: 0))
            XCTAssertEqual(replays, [originalRecovery, replacementRecovery], scenario)
            XCTAssertEqual(matches, replacementActive ? ["right"] : [], scenario)
            XCTAssertEqual(recognizer.inputGeneration, 0, scenario)
            XCTAssertNil(recognizer.nextDeadline, scenario)
            XCTAssertFalse(recognizer.snapshot.trace.isVisible, scenario)
            recognizer.stop()
            XCTAssertEqual(replays.count, 2, scenario)
          }
        }
      }
    }
  }

  func testFiniteReleaseAtDeadlineKeepsInputStartedDuringRecovery() throws {
    for releasedButton in [PointerButton.secondary, .primary] {
      let source = GestureEventSourceDouble(startResult: true)
      let clock = GestureRecognizerTestClock()
      var replays: [GestureReplayRequest] = []
      var matches: [String] = []
      let recognizer = GestureRecognizer<String>(
        eventSource: source,
        configuration: makeGestureRecognizerTestConfiguration(
          makeMatcher: { _ in
            var matcher = GesturePatternMatcher<String>()
            matcher.register(pattern: [.right], match: "right")
            return matcher
          },
          onReplayRequested: { request in
            replays.append(request)
            guard replays.count == 1 else { return }
            XCTAssertEqual(
              source.send(
                makeGestureInputEvent(
                  kind: .buttonDown(.secondary), location: .init(x: 100, y: 0))),
              .consume)
            XCTAssertEqual(
              source.send(
                makeGestureInputEvent(
                  kind: .buttonMoved(.secondary), location: .init(x: 140, y: 0))),
              .consume)
          },
          onMatch: { matches.append($0) },
          tuning: try .validated(maximumGestureSessionDuration: 1)))
      recognizer.nowOverride = { clock.now }
      recognizer.start()
      defer { recognizer.stop() }
      XCTAssertEqual(
        source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero)),
        .consume)
      let originalGeneration = recognizer.inputGeneration
      clock.now = try XCTUnwrap(recognizer.nextDeadline)

      let end = GesturePoint(x: 180, y: 0)
      XCTAssertEqual(
        source.send(makeGestureInputEvent(kind: .buttonUp(releasedButton), location: end)),
        .passThrough)
      XCTAssertEqual(replays, [.click(button: .secondary, at: .zero)])
      XCTAssertTrue(matches.isEmpty)
      XCTAssertNotEqual(recognizer.inputGeneration, 0)
      XCTAssertNotEqual(recognizer.inputGeneration, originalGeneration)
      XCTAssertTrue(recognizer.snapshot.status.isCapturingGesture)

      XCTAssertEqual(
        source.send(makeGestureInputEvent(kind: .buttonUp(.secondary), location: end)),
        .consume)
      XCTAssertEqual(matches, ["right"])
      XCTAssertEqual(
        replays, [.click(button: .secondary, at: .zero), .release(button: .secondary, at: end)])
      XCTAssertEqual(recognizer.inputGeneration, 0)
      XCTAssertNil(recognizer.nextDeadline)
    }
  }

  func testMoveAtDeadlineDoesNotCaptureInputStartedDuringRecovery() throws {
    let source = GestureEventSourceDouble(startResult: true)
    let clock = GestureRecognizerTestClock()
    let replacement = GesturePoint(x: 100, y: 0)
    var replays: [GestureReplayRequest] = []
    let recognizer = GestureRecognizer<String>(
      eventSource: source,
      configuration: makeGestureRecognizerTestConfiguration(
        onReplayRequested: { request in
          replays.append(request)
          guard replays.count == 1 else { return }
          XCTAssertEqual(
            source.send(
              makeGestureInputEvent(kind: .buttonDown(.secondary), location: replacement)),
            .consume)
        },
        tuning: try .validated(maximumGestureSessionDuration: 1)))
    recognizer.nowOverride = { clock.now }
    recognizer.isRecordingModeEnabled = true
    recognizer.start()
    defer { recognizer.stop() }
    XCTAssertEqual(
      source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero)),
      .consume)
    let originalGeneration = recognizer.inputGeneration
    clock.now = try XCTUnwrap(recognizer.nextDeadline)

    XCTAssertEqual(
      source.send(
        makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 140, y: 0))),
      .passThrough)
    XCTAssertEqual(replays, [.click(button: .secondary, at: .zero)])
    XCTAssertNotEqual(recognizer.inputGeneration, originalGeneration)
    XCTAssertFalse(recognizer.snapshot.trace.isVisible)

    let nextLocation = GesturePoint(x: 160, y: 0)
    XCTAssertEqual(
      source.send(makeGestureInputEvent(kind: .buttonMoved(.secondary), location: nextLocation)),
      .consume)
    XCTAssertEqual(recognizer.snapshot.trace.directionEndpoints, [replacement, nextLocation])
  }

  func testReplacementPressKeepsInputStartedDuringRecovery() throws {
    for expired in [false, true] {
      let source = GestureEventSourceDouble(startResult: true)
      let clock = GestureRecognizerTestClock()
      let reentrantPress = GesturePoint(x: 100, y: 0)
      var replays: [GestureReplayRequest] = []
      var contextLocations: [GesturePoint] = []
      let recognizer = GestureRecognizer<String>(
        eventSource: source,
        configuration: makeGestureRecognizerTestConfiguration(
          onReplayRequested: { request in
            replays.append(request)
            guard replays.count == 1 else { return }
            XCTAssertEqual(
              source.send(
                makeGestureInputEvent(kind: .buttonDown(.secondary), location: reentrantPress)),
              .consume)
          },
          recognitionContext: { location in
            contextLocations.append(location)
            return nil
          },
          tuning: try .validated(maximumGestureSessionDuration: 1)))
      recognizer.nowOverride = { clock.now }
      recognizer.isRecordingModeEnabled = true
      recognizer.start()
      defer { recognizer.stop() }
      XCTAssertEqual(
        source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero)),
        .consume)
      clock.now = expired ? try XCTUnwrap(recognizer.nextDeadline) : 1

      XCTAssertEqual(
        source.send(
          makeGestureInputEvent(kind: .buttonDown(.secondary), location: .init(x: 60, y: 0))),
        .passThrough)
      XCTAssertEqual(replays, [.click(button: .secondary, at: .zero)])
      XCTAssertEqual(contextLocations, [.zero, reentrantPress])
      XCTAssertEqual(
        recognizer.snapshot.lastSessionReport?.outcome, expired ? .totalDeadline : .superseded)

      let nextLocation = GesturePoint(x: 140, y: 0)
      XCTAssertEqual(
        source.send(makeGestureInputEvent(kind: .buttonMoved(.secondary), location: nextLocation)),
        .consume)
      XCTAssertEqual(recognizer.snapshot.trace.directionEndpoints, [reentrantPress, nextLocation])
    }
  }

  func testReplacementPressDoesNotResumeAfterNestedInputCompletes() {
    let source = GestureEventSourceDouble(startResult: true)
    let nestedPress = GesturePoint(x: 100, y: 0)
    var replays: [GestureReplayRequest] = []
    let recognizer = GestureRecognizer<String>(
      eventSource: source,
      configuration: makeGestureRecognizerTestConfiguration(
        onReplayRequested: { request in
          replays.append(request)
          guard replays.count == 1 else { return }
          XCTAssertEqual(
            source.send(
              makeGestureInputEvent(kind: .buttonDown(.secondary), location: nestedPress)),
            .consume)
          XCTAssertEqual(
            source.send(makeGestureInputEvent(kind: .buttonUp(.secondary), location: nestedPress)),
            .consume)
        }))
    recognizer.start()
    defer { recognizer.stop() }
    XCTAssertEqual(
      source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero)),
      .consume)
    XCTAssertEqual(
      source.send(
        makeGestureInputEvent(kind: .buttonDown(.secondary), location: .init(x: 60, y: 0))),
      .passThrough)
    XCTAssertEqual(recognizer.inputGeneration, 0)
    XCTAssertEqual(recognizer.snapshot.lastSessionReport?.outcome, .belowThreshold)
    XCTAssertEqual(
      replays, [.click(button: .secondary, at: .zero), .click(button: .secondary, at: nestedPress)])
  }

  func testReplacementPressUsesRecordingModeChangedDuringRecovery() {
    let source = GestureEventSourceDouble(startResult: true)
    weak var current: GestureRecognizer<String>?
    let recognizer = GestureRecognizer<String>(
      eventSource: source,
      configuration: makeGestureRecognizerTestConfiguration(
        onReplayRequested: { _ in current?.isRecordingModeEnabled = true }))
    current = recognizer
    recognizer.start()
    defer { recognizer.stop() }
    XCTAssertEqual(
      source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero)),
      .consume)
    XCTAssertEqual(
      source.send(
        makeGestureInputEvent(kind: .buttonDown(.secondary), location: .init(x: 60, y: 0))),
      .consume)
    XCTAssertEqual(
      source.send(
        makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 100, y: 0))),
      .consume)
    XCTAssertTrue(recognizer.snapshot.trace.isVisible)
    XCTAssertEqual(recognizer.snapshot.lastSessionReport?.outcome, .superseded)
  }

  func testReplacementPressDoesNotEnterTheSourceRestartedDuringRecovery() {
    let source = GestureEventSourceDouble(startResult: true)
    weak var current: GestureRecognizer<String>?
    var contextLocations: [GesturePoint] = []
    let recognizer = GestureRecognizer<String>(
      eventSource: source,
      configuration: makeGestureRecognizerTestConfiguration(
        onReplayRequested: { _ in
          current?.stop()
          current?.start()
        },
        recognitionContext: { location in
          contextLocations.append(location)
          return nil
        }))
    current = recognizer
    recognizer.start()
    defer { recognizer.stop() }
    XCTAssertEqual(
      source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero)),
      .consume)
    XCTAssertEqual(
      source.send(
        makeGestureInputEvent(kind: .buttonDown(.secondary), location: .init(x: 60, y: 0))),
      .passThrough)
    XCTAssertEqual(source.startCount, 2)
    XCTAssertEqual(recognizer.snapshot.status.lifecycle, .ready)
    XCTAssertEqual(recognizer.inputGeneration, 0)
    XCTAssertEqual(contextLocations, [.zero])
  }

  func testTraceObserverStopOrCancelCannotRestoreStaleStateOrLeaveInputHeld() {
    for stop in [false, true] {
      let source = GestureEventSourceDouble(startResult: true)
      var replays: [GestureReplayRequest] = []
      let recognizer = GestureRecognizer<UUID>(
        eventSource: source,
        configuration: makeGestureRecognizerTestConfiguration(onReplayRequested: {
          replays.append($0)
        }))
      recognizer.isRecordingModeEnabled = true
      var secondVisibility: [Bool] = []
      let first = recognizer.observeTrace { [weak recognizer] trace in
        guard trace.isVisible else { return }
        if stop { recognizer?.stop() } else { recognizer?.cancelActiveGesture() }
      }
      let second = recognizer.observeTrace { secondVisibility.append($0.isVisible) }
      recognizer.start()
      _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
      _ = source.send(
        makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 20, y: 0)))
      XCTAssertFalse(recognizer.snapshot.trace.isVisible)
      XCTAssertFalse(recognizer.snapshot.status.isCapturingGesture)
      XCTAssertEqual(secondVisibility.last, false)
      XCTAssertEqual(replays, [.release(button: .secondary, at: .init(x: 20, y: 0))])
      XCTAssertEqual(recognizer.inputGeneration, 0)
      first.cancel()
      second.cancel()
      recognizer.stop()
    }
  }

  func testStopFromCompletedReplayDoesNotDeliverTheOldMatchOrDoubleRelease() {
    let source = GestureEventSourceDouble(startResult: true)
    weak var current: GestureRecognizer<UUID>?
    var matcher = GesturePatternMatcher<UUID>()
    matcher.register(pattern: [.right], match: UUID())
    var matches = 0
    var releases = 0
    let recognizer = GestureRecognizer<UUID>(
      eventSource: source,
      configuration: makeGestureRecognizerTestConfiguration(
        makeMatcher: { [matcher] _ in matcher },
        onReplayRequested: { _ in
          releases += 1
          current?.stop()
        }, onMatch: { _ in matches += 1 }))
    current = recognizer
    recognizer.start()
    _ = source.send(makeGestureInputEvent(kind: .buttonDown(.secondary), location: .zero))
    _ = source.send(
      makeGestureInputEvent(kind: .buttonMoved(.secondary), location: .init(x: 20, y: 0)))
    _ = source.send(
      makeGestureInputEvent(kind: .buttonUp(.secondary), location: .init(x: 20, y: 0)))
    XCTAssertEqual(matches, 0)
    XCTAssertEqual(releases, 1)
    XCTAssertEqual(recognizer.snapshot.status.lifecycle, .idle)
    XCTAssertFalse(recognizer.snapshot.trace.isVisible)
    recognizer.stop()
  }
}
