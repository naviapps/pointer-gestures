import CoreGraphics
import PointerGestureRecognition
import XCTest

@testable import PointerGestureRecognitionMacOS

@MainActor
final class GestureEventTapUsageTests: XCTestCase {
  func testSendableContractAcceptsEventTap() {
    let sendableEventTap: any Sendable = GestureEventTap()
    _ = sendableEventTap
  }

  func testStopIsIdempotentWhenTapIsNotStarted() {
    let eventTap = GestureEventTap()

    eventTap.stop()
    eventTap.stop()
  }

  func testStopDisconnectsBeforeRecoveryAndKeepsAReentrantHandler() throws {
    let tap = GestureEventTap()
    tap.installHandler { _ in .consume }
    let event = try XCTUnwrap(
      CGEvent(
        mouseEventSource: nil, mouseType: .rightMouseDown,
        mouseCursorPosition: .zero, mouseButton: .right))
    var recoveryCount = 0
    tap.stop {
      recoveryCount += 1
      XCTAssertNotNil(tap.handleTapEvent(type: .rightMouseDown, event: event))
      tap.installHandler { _ in .consume }
    }
    XCTAssertEqual(recoveryCount, 1)
    XCTAssertNil(tap.handleTapEvent(type: .rightMouseDown, event: event))
    tap.stop { recoveryCount += 1 }
    tap.stop { recoveryCount += 1 }
    XCTAssertEqual(recoveryCount, 3)
  }

  func testStartReturnsFalseWhenCapturedButtonsAreEmpty() {
    let eventTap = GestureEventTap(capturedButtons: [])

    let didStart = eventTap.start { _ in .consume }

    XCTAssertFalse(didStart)
  }

  func testFinalReleaseFromDetachedTaskClearsHandlerOnMainThread() async {
    let released = expectation(description: "Event tap releases its captured handler")
    let task = Task.detached { [tap = makeTapWithReleaseProbe(released)] in
      withExtendedLifetime(tap) {}
    }
    await task.value
    await fulfillment(of: [released], timeout: 2)
  }

  func testDisabledTapDeliversCancelSynchronouslyOnMainActor() {
    let tap = GestureEventTap()
    var handledKinds: [GestureInputEvent.Kind] = []
    tap.installHandler { input in
      handledKinds.append(input.kind)
      return .consume
    }

    tap.recoverFromTapDisabled(
      input: GestureInputEvent(
        kind: .cancel,
        location: .zero,
        modifiers: []
      )
    )

    XCTAssertEqual(handledKinds, [.cancel])
  }

  func testNonMainCallbacksPassThroughWithoutQueuingRecovery() async throws {
    var handledKinds: [GestureInputEvent.Kind] = []
    let tap = GestureEventTap()
    tap.installHandler { input in
      handledKinds.append(input.kind)
      return .consume
    }

    let passedThrough = await Task.detached {
      guard let event = CGEvent(source: nil) else { return false }
      return [CGEventType.tapDisabledByTimeout, .tapDisabledByUserInput, .rightMouseDown]
        .allSatisfy { tap.handleTapEvent(type: $0, event: event) != nil }
    }.value

    XCTAssertTrue(passedThrough)
    try await Task.sleep(for: .milliseconds(10))
    XCTAssertTrue(handledKinds.isEmpty)
  }

  func testStoppedTapDropsHandlerAndPassesThroughInput() throws {
    let tap = GestureEventTap()
    var handledKinds: [GestureInputEvent.Kind] = []
    tap.installHandler { input in
      handledKinds.append(input.kind)
      return .consume
    }

    tap.stop()
    let event = try XCTUnwrap(
      CGEvent(
        mouseEventSource: nil, mouseType: .rightMouseDown,
        mouseCursorPosition: .zero, mouseButton: .right))
    XCTAssertTrue(tap.handleTapEvent(type: .rightMouseDown, event: event) != nil)
    XCTAssertTrue(handledKinds.isEmpty)
  }

  func testTapPassesThroughReplayedInputWithoutInvokingHandler() throws {
    let tap = GestureEventTap()
    var handledKinds: [GestureInputEvent.Kind] = []
    tap.installHandler { input in
      handledKinds.append(input.kind)
      return .consume
    }
    let event = try XCTUnwrap(
      CGEvent(
        mouseEventSource: nil, mouseType: .rightMouseUp,
        mouseCursorPosition: .zero, mouseButton: .right))
    ReplayedEventMarker.mark(event)
    XCTAssertTrue(tap.handleTapEvent(type: .rightMouseUp, event: event) != nil)
    XCTAssertTrue(handledKinds.isEmpty)
  }

  func testConsumedEscapeKeepsRepeatsAndReleaseFromForegroundUntilNextIdlePress() throws {
    let tap = GestureEventTap()
    var cancelModifiers: [GestureModifierFlags] = []
    tap.installHandler { input in
      XCTAssertEqual(input.kind, .cancel)
      cancelModifiers.append(input.modifiers)
      return cancelModifiers.count == 1 ? .consume : .passThrough
    }
    let down = try escapeEvent(.keyDown, at: 10)
    down.flags = [.maskControl]
    let repeated = try escapeEvent(.keyDown, at: 11, repeated: true)
    let release = try escapeEvent(.keyUp, at: 12)

    XCTAssertNil(tap.handleTapEvent(type: .keyDown, event: down))
    XCTAssertNil(tap.handleTapEvent(type: .keyDown, event: repeated))
    XCTAssertNil(tap.handleTapEvent(type: .keyUp, event: release))

    let idleDown = try escapeEvent(.keyDown, at: 13)
    let idleRepeated = try escapeEvent(.keyDown, at: 14, repeated: true)
    let idleRelease = try escapeEvent(.keyUp, at: 15)
    assertPassThrough(idleDown, on: tap)
    assertPassThrough(idleRepeated, on: tap)
    assertPassThrough(idleRelease, on: tap)
    XCTAssertEqual(cancelModifiers, [[.control], []])
  }

  func testEscapeLeavesOtherKeyboardTypesAndStaleOrReplayedReleasesAlone() throws {
    let tap = GestureEventTap()
    var cancels = 0
    tap.installHandler { _ in
      cancels += 1
      return .consume
    }
    XCTAssertNil(tap.handleTapEvent(type: .keyDown, event: try escapeEvent(.keyDown, at: 10)))
    let otherRepeat = try escapeEvent(.keyDown, at: 11, keyboardType: 41, repeated: true)
    let otherRelease = try escapeEvent(.keyUp, at: 12, keyboardType: 41)
    let staleRelease = try escapeEvent(.keyUp, at: 9)
    assertPassThrough(otherRepeat, on: tap)
    assertPassThrough(otherRelease, on: tap)
    assertPassThrough(staleRelease, on: tap)

    let marker = try XCTUnwrap(CGEvent(source: nil))
    ReplayedEventMarker.mark(marker)
    let userData = marker.getIntegerValueField(.eventSourceUserData)
    let replayDown = try escapeEvent(.keyDown, at: 13, userData: userData)
    let replayUp = try escapeEvent(.keyUp, at: 14, userData: userData)
    XCTAssertTrue(ReplayedEventMarker.isPresent(on: replayDown))
    XCTAssertTrue(ReplayedEventMarker.isPresent(on: replayUp))
    assertPassThrough(replayDown, on: tap)
    assertPassThrough(replayUp, on: tap)
    XCTAssertNil(tap.handleTapEvent(type: .keyUp, event: try escapeEvent(.keyUp, at: 15)))
    XCTAssertEqual(cancels, 1)
  }

  func testTapDisableAndReentrantStopDoNotCarryConsumedEscapeIntoNewCapture() throws {
    let tap = GestureEventTap()
    tap.installHandler { _ in .consume }
    XCTAssertNil(tap.handleTapEvent(type: .keyDown, event: try escapeEvent(.keyDown, at: 10)))
    tap.recoverFromTapDisabled(
      input: GestureInputEvent(kind: .cancel, location: .zero, modifiers: []))
    assertPassThrough(try escapeEvent(.keyUp, at: 11), on: tap)

    tap.installHandler { [weak tap] _ in
      tap?.stop { tap?.installHandler { _ in .passThrough } }
      return .consume
    }
    XCTAssertNil(tap.handleTapEvent(type: .keyDown, event: try escapeEvent(.keyDown, at: 12)))
    assertPassThrough(try escapeEvent(.keyUp, at: 13), on: tap)
    assertPassThrough(try escapeEvent(.keyDown, at: 14), on: tap)
    assertPassThrough(try escapeEvent(.keyUp, at: 15), on: tap)
  }

  func testNewEscapeDownSupersedesMissingReleaseWithoutSwallowingIdleEvents() throws {
    let tap = GestureEventTap()
    var cancels = 0
    tap.installHandler { _ in
      cancels += 1
      return cancels == 1 ? .consume : .passThrough
    }
    XCTAssertNil(tap.handleTapEvent(type: .keyDown, event: try escapeEvent(.keyDown, at: 10)))
    assertPassThrough(try escapeEvent(.keyDown, at: 11), on: tap)
    let repeated = try escapeEvent(.keyDown, at: 12, repeated: true)
    assertPassThrough(repeated, on: tap)
    assertPassThrough(try escapeEvent(.keyUp, at: 13), on: tap)
    XCTAssertEqual(cancels, 2)
  }

  func testReadmeStyleDefaultButtonControllerTypeChecks() {
    let controller = ReadmeStyleGestureController(makeGestureMatcher: Self.makeGestureMatcher)

    controller.configureDefaultButtonRecognizer()

    XCTAssertEqual(controller.recognizer?.snapshot.status.lifecycle, .idle)
  }

  func testReadmeStylePrimaryButtonConfigurationTypeChecks() {
    let eventTap = GestureEventTap(capturedButtons: [.primary])
    let configuration = GestureRecognizerConfiguration<AppCommand>(
      makeMatcher: { _ in Self.makeGestureMatcher() },
      onReplayRequested: { request in eventTap.replay(request) },
      onMatch: { _ in },
      recognitionButton: .primary
    )

    let recognizer = GestureRecognizer(
      eventSource: eventTap,
      configuration: configuration
    )

    XCTAssertEqual(recognizer.snapshot.status.lifecycle, .idle)
  }

  private static func makeGestureMatcher() -> GesturePatternMatcher<AppCommand> {
    var matcher = GesturePatternMatcher<AppCommand>()
    matcher.register(pattern: [.down, .right], match: .showInspector)
    matcher.register(pattern: [.up, .left], match: .focusSearch)
    return matcher
  }
}

private func escapeEvent(
  _ type: CGEventType, at timestamp: CGEventTimestamp, keyboardType: CGEventSourceKeyboardType = 40,
  repeated: Bool = false, userData: Int64 = 0
) throws -> CGEvent {
  let source = try XCTUnwrap(CGEventSource(stateID: .privateState))
  source.keyboardType = keyboardType
  source.userData = userData
  let event = try XCTUnwrap(
    CGEvent(keyboardEventSource: source, virtualKey: 53, keyDown: type == .keyDown))
  event.timestamp = timestamp
  event.setIntegerValueField(.keyboardEventAutorepeat, value: repeated ? 1 : 0)
  XCTAssertEqual(event.getIntegerValueField(.keyboardEventKeyboardType), Int64(keyboardType))
  return event
}

@MainActor
private func assertPassThrough(
  _ event: CGEvent, on tap: GestureEventTap, file: StaticString = #filePath, line: UInt = #line
) {
  let delivered = tap.handleTapEvent(type: event.type, event: event)?.takeUnretainedValue()
  XCTAssertTrue(delivered === event, file: file, line: line)
}

@MainActor
private func makeTapWithReleaseProbe(_ released: XCTestExpectation) -> GestureEventTap {
  let probe = HandlerReleaseProbe {
    XCTAssertTrue(Thread.isMainThread)
    released.fulfill()
  }
  let tap = GestureEventTap()
  tap.installHandler { [probe] _ in
    withExtendedLifetime(probe) {}
    return .passThrough
  }
  return tap
}

private final class HandlerReleaseProbe: Sendable {
  private let onRelease: @Sendable () -> Void

  init(_ onRelease: @escaping @Sendable () -> Void) { self.onRelease = onRelease }

  deinit { onRelease() }
}

private enum AppCommand: Sendable {
  case showInspector
  case focusSearch
}

@MainActor
private final class ReadmeStyleGestureController {
  var recognizer: GestureRecognizer<AppCommand>?

  private let makeGestureMatcher: @MainActor () -> GesturePatternMatcher<AppCommand>

  init(makeGestureMatcher: @escaping @MainActor () -> GesturePatternMatcher<AppCommand>) {
    self.makeGestureMatcher = makeGestureMatcher
  }

  func configureDefaultButtonRecognizer() {
    let eventTap = GestureEventTap()
    let configuration = GestureRecognizerConfiguration<AppCommand>(
      makeMatcher: { [makeGestureMatcher] _ in makeGestureMatcher() },
      onReplayRequested: { request in eventTap.replay(request) },
      onMatch: { _ in },
      areModifiersSatisfied: { modifiers, _ in
        modifiers.contains(.command)
      }
    )

    recognizer = GestureRecognizer(
      eventSource: eventTap,
      configuration: configuration
    )
  }
}
