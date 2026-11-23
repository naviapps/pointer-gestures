import AppKit
import XCTest

@testable import PointerGestureRecognition
@testable import PointerGestureRecognitionMacOS

@MainActor
final class GestureEventTapReplayTests: XCTestCase {
  func testExactDeadlineImmediateReplayReplacesOriginalUp() throws {
    for active in [false, true] {
      var posted: [CGEvent] = []
      let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
      let recognizer = makeRecognizer(tap)
      defer { recognizer.stop() }
      let clock = ReplayClock()
      recognizer.nowOverride = { clock.now }
      let down = try deadlineMouseEvent(.leftMouseDown, count: 2, flags: .maskShift)
      let up = try deadlineMouseEvent(
        .leftMouseUp, count: 2, location: CGPoint(x: 100, y: 30), flags: .maskControl)

      XCTAssertNil(tap.handleTapEvent(type: .leftMouseDown, event: down))
      if active {
        clock.now = 250_000_000
        let move = try deadlineMouseEvent(
          .leftMouseDragged, count: 2, location: CGPoint(x: 60, y: 30))
        XCTAssertNil(tap.handleTapEvent(type: .leftMouseDragged, event: move))
        XCTAssertTrue(recognizer.snapshot.status.isCapturingGesture)
      }
      XCTAssertTrue(posted.isEmpty)
      clock.now = try XCTUnwrap(recognizer.nextDeadline)

      XCTAssertNil(
        tap.handleTapEvent(type: .leftMouseUp, event: up),
        "A complete synchronous replay already posted this Up (active: \(active))")
      XCTAssertEqual(posted.map(\.type), active ? [.leftMouseUp] : [.leftMouseDown, .leftMouseUp])
      XCTAssertTrue(posted.allSatisfy(ReplayedEventMarker.isPresent(on:)))
      XCTAssertEqual(
        posted.map { $0.getIntegerValueField(.mouseEventClickState) }, active ? [2] : [2, 2])
      XCTAssertEqual(posted.map(\.flags), active ? [.maskControl] : [.maskShift, .maskControl])
      XCTAssertEqual(recognizer.snapshot.status.lastFailure, .gestureSessionExpired)
      XCTAssertTrue(
        tap.handleTapEvent(type: .leftMouseUp, event: up)?.takeUnretainedValue() === up)
      XCTAssertEqual(posted.filter { $0.type == .leftMouseUp }.count, 1)
    }
  }

  func testExactDeadlinePreparedReplayDoesNotReplaceOriginalUp() throws {
    for deliver in [false, true] {
      var posted: [CGEvent] = []
      var requests = 0
      let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
      let recognizer = makeRecognizer(tap) {
        requests += 1
        let operation = tap.prepareReplay($0)
        if deliver { operation() }
      }
      defer { recognizer.stop() }
      let clock = ReplayClock()
      recognizer.nowOverride = { clock.now }

      let down = try deadlineMouseEvent(.leftMouseDown, count: 2)
      XCTAssertNil(tap.handleTapEvent(type: .leftMouseDown, event: down))
      clock.now = try XCTUnwrap(recognizer.nextDeadline)
      let up = try deadlineMouseEvent(.leftMouseUp, count: 2)
      XCTAssertTrue(tap.handleTapEvent(type: .leftMouseUp, event: up)?.takeUnretainedValue() === up)
      XCTAssertEqual(requests, 1)
      XCTAssertEqual(posted.map(\.type), deliver ? [.leftMouseDown, .leftMouseUp] : [])
    }
  }

  func testExactDeadlineQueuedReplayReplacesOriginalUpOnlyWhenAccepted() throws {
    for accepts in [false, true] {
      var posted: [CGEvent] = []
      var pending: [@MainActor @Sendable () -> Void] = []
      var offers = 0
      let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
      let recognizer = makeRecognizer(tap) { request in
        let accepted = tap.enqueueReplay(request) { deliver in
          offers += 1
          if accepts { pending.append(deliver) }
          return accepts
        }
        XCTAssertEqual(accepted, accepts)
      }
      defer { recognizer.stop() }
      let clock = ReplayClock()
      recognizer.nowOverride = { clock.now }

      let down = try deadlineMouseEvent(.leftMouseDown, count: 2)
      XCTAssertNil(tap.handleTapEvent(type: .leftMouseDown, event: down))
      clock.now = try XCTUnwrap(recognizer.nextDeadline)
      let up = try deadlineMouseEvent(.leftMouseUp, count: 2, flags: .maskControl)
      let originalWasConsumed = tap.handleTapEvent(type: .leftMouseUp, event: up) == nil
      XCTAssertEqual(originalWasConsumed, accepts)
      XCTAssertEqual(offers, 1)
      XCTAssertEqual(pending.count, accepts ? 1 : 0)
      XCTAssertTrue(posted.isEmpty)

      recognizer.stop()
      for deliver in pending { deliver() }
      XCTAssertEqual(posted.map(\.type), accepts ? [.leftMouseDown, .leftMouseUp] : [])
      if accepts { XCTAssertEqual(posted.last?.flags, .maskControl) }
    }
  }

  func testRefusedReplayCanFallBackToImmediateReplayWithOriginalPress() throws {
    for active in [false, true] {
      var posted: [CGEvent] = []
      let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
      let recognizer = makeRecognizer(tap) { request in
        XCTAssertFalse(tap.enqueueReplay(request) { _ in false })
        XCTAssertTrue(posted.isEmpty)
        tap.replay(request)
      }
      defer { recognizer.stop() }
      let clock = ReplayClock()
      recognizer.nowOverride = { clock.now }
      let down = try mouseEvent(.leftMouseDown, count: 2, flags: .maskShift)
      let up = try mouseEvent(
        .leftMouseUp, count: 2, location: CGPoint(x: 100, y: 30), flags: .maskControl)
      XCTAssertNil(tap.handleTapEvent(type: .leftMouseDown, event: down))
      if active {
        clock.now = 250_000_000
        let move = try mouseEvent(.leftMouseDragged, count: 2, location: CGPoint(x: 60, y: 30))
        XCTAssertNil(tap.handleTapEvent(type: .leftMouseDragged, event: move))
        XCTAssertTrue(recognizer.snapshot.status.isCapturingGesture)
      }
      clock.now = try XCTUnwrap(recognizer.nextDeadline)

      XCTAssertNil(tap.handleTapEvent(type: .leftMouseUp, event: up))
      XCTAssertEqual(posted.map(\.type), active ? [.leftMouseUp] : [.leftMouseDown, .leftMouseUp])
      XCTAssertEqual(
        posted.map { $0.getIntegerValueField(.mouseEventClickState) }, active ? [2] : [2, 2])
      XCTAssertEqual(posted.map(\.flags), active ? [.maskControl] : [.maskShift, .maskControl])
      XCTAssertEqual(recognizer.snapshot.status.lastFailure, .gestureSessionExpired)
    }
  }

  func testExactDeadlineInvalidReplayDoesNotReplaceOriginalUp() throws {
    var posted: [CGEvent] = []
    var requests = 0
    let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
    let recognizer = makeRecognizer(tap) { _ in
      requests += 1
      tap.replay(.release(button: .primary, at: .init(x: .nan, y: 0)))
    }
    defer { recognizer.stop() }
    let clock = ReplayClock()
    recognizer.nowOverride = { clock.now }

    let down = try deadlineMouseEvent(.leftMouseDown, count: 2)
    XCTAssertNil(tap.handleTapEvent(type: .leftMouseDown, event: down))
    clock.now = try XCTUnwrap(recognizer.nextDeadline)
    let up = try deadlineMouseEvent(.leftMouseUp, count: 2)
    XCTAssertTrue(tap.handleTapEvent(type: .leftMouseUp, event: up)?.takeUnretainedValue() === up)
    XCTAssertEqual(requests, 1)
    XCTAssertTrue(posted.isEmpty)
  }

  func testExactDeadlineNonterminalReplayDoesNotReplaceOriginalUp() throws {
    var posted: [CGEvent] = []
    let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
    let recognizer = makeRecognizer(tap) { _ in
      tap.replay(.dragStart(button: .primary, points: [.zero]))
    }
    defer { recognizer.stop() }
    let clock = ReplayClock()
    recognizer.nowOverride = { clock.now }

    let down = try deadlineMouseEvent(.leftMouseDown, count: 2)
    XCTAssertNil(tap.handleTapEvent(type: .leftMouseDown, event: down))
    clock.now = try XCTUnwrap(recognizer.nextDeadline)
    let up = try deadlineMouseEvent(.leftMouseUp, count: 2)
    XCTAssertTrue(tap.handleTapEvent(type: .leftMouseUp, event: up)?.takeUnretainedValue() === up)
    XCTAssertEqual(posted.map(\.type), [.leftMouseDown])
  }

  func testExactDeadlineReplayCannotReplaceAnotherButtonUp() throws {
    var posted: [CGEvent] = []
    let tap = GestureEventTap(capturedButtons: [.primary, .secondary]) { posted.append($0) }
    let recognizer = makeRecognizer(tap)
    defer { recognizer.stop() }
    let clock = ReplayClock()
    recognizer.nowOverride = { clock.now }

    let down = try deadlineMouseEvent(.leftMouseDown, count: 2)
    XCTAssertNil(tap.handleTapEvent(type: .leftMouseDown, event: down))
    clock.now = try XCTUnwrap(recognizer.nextDeadline)
    let up = try deadlineMouseEvent(.rightMouseUp, count: 1)
    XCTAssertTrue(tap.handleTapEvent(type: .rightMouseUp, event: up)?.takeUnretainedValue() === up)
    XCTAssertEqual(posted.map(\.type), [.leftMouseDown, .leftMouseUp])
  }

  func testReentrantPressCannotAcknowledgeOuterRelease() throws {
    var posted: [CGEvent] = []
    weak var activeTap: GestureEventTap?
    var nestedDownConsumed = false
    let nestedDown = try deadlineMouseEvent(.leftMouseDown, count: 3, flags: .maskAlternate)
    let tap = GestureEventTap(capturedButtons: [.primary]) { event in
      posted.append(event)
      if posted.count == 1, let activeTap {
        nestedDownConsumed =
          activeTap.handleTapEvent(type: .leftMouseDown, event: nestedDown) == nil
      }
    }
    activeTap = tap
    tap.installHandler { [weak tap] input in
      if case .buttonDown = input.kind { return .consume }
      tap?.replay(.click(button: .primary, at: input.location))
      return .passThrough
    }
    defer { tap.stop() }
    let down = try deadlineMouseEvent(.leftMouseDown, count: 2, flags: .maskShift)
    let up = try deadlineMouseEvent(.leftMouseUp, count: 2, flags: .maskControl)

    XCTAssertNil(tap.handleTapEvent(type: .leftMouseDown, event: down))
    XCTAssertTrue(
      tap.handleTapEvent(type: .leftMouseUp, event: up)?.takeUnretainedValue() === up)
    XCTAssertTrue(nestedDownConsumed)
    tap.replay(.click(button: .primary, at: .zero))
    XCTAssertEqual(posted.map { $0.getIntegerValueField(.mouseEventClickState) }, [2, 2, 3, 3])
    XCTAssertEqual(posted.map(\.flags), [.maskShift, .maskControl, .maskAlternate, .maskAlternate])
  }

  func testRecoveredPrimaryClickPreservesAppKitMultipleClickCount() throws {
    var posted: [CGEvent] = []
    let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
    let recognizer = makeRecognizer(tap)
    defer { recognizer.stop() }

    for count: Int64 in [2, 3] {
      posted.removeAll()
      _ = tap.handleTapEvent(
        type: .leftMouseDown, event: try mouseEvent(.leftMouseDown, count: count))
      _ = tap.handleTapEvent(type: .leftMouseUp, event: try mouseEvent(.leftMouseUp, count: count))

      XCTAssertEqual(posted.map { $0.getIntegerValueField(.mouseEventClickState) }, [count, count])
      let down = try XCTUnwrap(posted.first.flatMap(NSEvent.init(cgEvent:)))
      XCTAssertEqual(down.clickCount, Int(count))
      XCTAssertTrue(posted.allSatisfy(ReplayedEventMarker.isPresent(on:)))
    }
  }

  func testRecoveryPreservesPressAndReleaseModifierFlags() throws {
    var posted: [CGEvent] = []
    let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
    let recognizer = makeRecognizer(tap)
    defer { recognizer.stop() }
    let pressed: CGEventFlags = [.maskShift, .maskAlternate]
    _ = tap.handleTapEvent(
      type: .leftMouseDown, event: try mouseEvent(.leftMouseDown, count: 2, flags: pressed))
    _ = tap.handleTapEvent(
      type: .leftMouseUp, event: try mouseEvent(.leftMouseUp, count: 2, flags: .maskControl))

    XCTAssertEqual(posted.map(\.flags), [pressed, .maskControl])
    let down = try XCTUnwrap(posted.first.flatMap(NSEvent.init(cgEvent:)))
    XCTAssertTrue(down.modifierFlags.contains([.shift, .option]))
    XCTAssertFalse(down.modifierFlags.contains(.control))
  }

  func testRecoveredDragUsesPressFlagsForDownAndLatestFlagsForMovementAndUp() throws {
    var posted: [CGEvent] = []
    let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
    let recognizer = makeRecognizer(tap)
    defer { recognizer.stop() }
    _ = tap.handleTapEvent(
      type: .leftMouseDown, event: try mouseEvent(.leftMouseDown, count: 2, flags: .maskShift))
    _ = tap.handleTapEvent(
      type: .leftMouseUp,
      event: try mouseEvent(
        .leftMouseUp, count: 2, location: CGPoint(x: 70, y: 30), flags: .maskCommand))

    XCTAssertEqual(posted.map(\.type), [.leftMouseDown, .leftMouseDragged, .leftMouseUp])
    XCTAssertEqual(posted.map(\.flags), [.maskShift, .maskCommand, .maskCommand])
  }

  func testDeferredRecoveryKeepsEachPressAfterInputAndTapStop() throws {
    var posted: [CGEvent] = []
    var pending: [@MainActor @Sendable () -> Void] = []
    let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
    let recognizer = makeRecognizer(tap) { pending.append(tap.prepareReplay($0)) }
    let presses: [(Int64, CGEventFlags, CGEventFlags)] = [
      (2, .maskShift, .maskControl), (3, .maskAlternate, .maskCommand),
    ]
    for (count, downFlags, upFlags) in presses {
      _ = tap.handleTapEvent(
        type: .leftMouseDown, event: try mouseEvent(.leftMouseDown, count: count, flags: downFlags))
      _ = tap.handleTapEvent(
        type: .leftMouseUp, event: try mouseEvent(.leftMouseUp, count: count, flags: upFlags))
    }
    recognizer.stop()
    XCTAssertTrue(posted.isEmpty)
    for deliver in pending { deliver() }
    XCTAssertEqual(posted.map { $0.getIntegerValueField(.mouseEventClickState) }, [2, 2, 3, 3])
    XCTAssertEqual(posted.map(\.flags), [.maskShift, .maskControl, .maskAlternate, .maskCommand])
  }

  func testPreparedReplayDoesNotRetainTap() throws {
    var posted: [CGEvent] = []
    var delivery: (@MainActor @Sendable () -> Void)?
    weak var releasedTap: GestureEventTap?
    do {
      let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
      releasedTap = tap
      tap.installHandler { _ in .consume }
      _ = tap.handleTapEvent(type: .leftMouseDown, event: try mouseEvent(.leftMouseDown, count: 2))
      delivery = tap.prepareReplay(.click(button: .primary, at: .zero))
    }
    XCTAssertNil(releasedTap)
    XCTAssertTrue(posted.isEmpty)
    try XCTUnwrap(delivery)()
    XCTAssertEqual(posted.map { $0.getIntegerValueField(.mouseEventClickState) }, [2, 2])
  }

  func testNativeTextSelectionAfterRecoveredPrimaryClick() throws {
    guard ProcessInfo.processInfo.environment["POINTER_GESTURE_NATIVE_UI_TESTS"] == "1" else {
      throw XCTSkip("Run only in an isolated macOS GUI session with native UI tests enabled.")
    }
    let app = NSApplication.shared
    app.setActivationPolicy(.regular)
    app.finishLaunching()
    let window = NSWindow(
      contentRect: NSRect(x: 100, y: 100, width: 480, height: 120),
      styleMask: [.titled, .closable], backing: .buffered, defer: false)
    window.isReleasedWhenClosed = false
    defer { window.close() }
    let textView = NSTextView(frame: NSRect(x: 0, y: 0, width: 480, height: 120))
    textView.string = "double click recovery"
    textView.font = .monospacedSystemFont(ofSize: 18, weight: .regular)
    textView.textContainerInset = NSSize(width: 10, height: 10)
    window.contentView = textView
    window.makeKeyAndOrderFront(nil)
    window.makeFirstResponder(textView)
    let readyDeadline = Date(timeIntervalSinceNow: 1)
    while let event = app.nextEvent(
      matching: .any, until: readyDeadline, inMode: .default, dequeue: true)
    {
      app.sendEvent(event)
      if Date() >= readyDeadline { break }
    }
    textView.layoutManager?.ensureLayout(for: try XCTUnwrap(textView.textContainer))
    let glyph = try XCTUnwrap(textView.layoutManager).boundingRect(
      forGlyphRange: NSRange(location: 1, length: 1), in: try XCTUnwrap(textView.textContainer))
    let point = textView.convert(
      NSPoint(
        x: glyph.midX + textView.textContainerOrigin.x,
        y: glyph.midY + textView.textContainerOrigin.y), to: nil)
    let screenPoint = window.convertPoint(toScreen: point)
    let capturePoint = CGPoint(
      x: screenPoint.x, y: try XCTUnwrap(NSScreen.screens.first).frame.maxY - screenPoint.y)

    let originalCursor = try XCTUnwrap(CGEvent(source: nil)).location
    XCTAssertEqual(CGWarpMouseCursorPosition(capturePoint), .success)
    defer { _ = CGWarpMouseCursorPosition(originalCursor) }

    let tap = GestureEventTap(capturedButtons: [.primary]) { cgEvent in
      let nativeModifiers = NSEvent(cgEvent: cgEvent)!.modifierFlags
      let event = NSEvent.mouseEvent(
        with: cgEvent.type == .leftMouseDown ? .leftMouseDown : .leftMouseUp,
        location: point, modifierFlags: nativeModifiers,
        timestamp: ProcessInfo.processInfo.systemUptime,
        windowNumber: window.windowNumber, context: nil, eventNumber: 0,
        clickCount: Int(cgEvent.getIntegerValueField(.mouseEventClickState)), pressure: 1)!
      app.postEvent(event, atStart: false)
    }
    let recognizer = makeRecognizer(tap)
    defer { recognizer.stop() }
    let cases: [(Int64, CGEventFlags)] = [(1, .maskShift), (2, []), (3, [])]
    for (count, downFlags) in cases {
      textView.setSelectedRange(NSRange(location: 0, length: 0))
      _ = tap.handleTapEvent(
        type: .leftMouseDown,
        event: try mouseEvent(
          .leftMouseDown, count: count, location: capturePoint, flags: downFlags))
      _ = tap.handleTapEvent(
        type: .leftMouseUp,
        event: try mouseEvent(.leftMouseUp, count: count, location: capturePoint))
      let down = try XCTUnwrap(
        app.nextEvent(
          matching: .leftMouseDown, until: Date(timeIntervalSinceNow: 2), inMode: .default,
          dequeue: true))
      XCTAssertEqual(down.windowNumber, window.windowNumber)
      // Exercise the native widget with recovered click metadata. A bare SwiftPM
      // XCTest process cannot assume cooperative foreground activation; neither
      // this injected replay fixture nor its assertions claim OS focus transfer.
      textView.mouseDown(with: down)
      let selected = (textView.string as NSString).substring(with: textView.selectedRange())
      if count == 1 {
        XCTAssertEqual(textView.selectedRange().location, 0)
        XCTAssertGreaterThan(textView.selectedRange().length, 0)
        XCTAssertLessThan(textView.selectedRange().length, "double".count)
      } else {
        XCTAssertEqual(selected, count == 2 ? "double" : textView.string)
      }
      if let directory = ProcessInfo.processInfo.environment["POINTER_GESTURE_UI_RESULTS"] {
        window.displayIfNeeded()
        let bitmap = try XCTUnwrap(textView.bitmapImageRepForCachingDisplay(in: textView.bounds))
        textView.cacheDisplay(in: textView.bounds, to: bitmap)
        let data = try XCTUnwrap(bitmap.representation(using: .png, properties: [:]))
        try data.write(
          to: URL(fileURLWithPath: directory).appendingPathComponent("selection-\(count).png"))
      }
    }
  }

  func testInterruptedMultipleClickDragPreservesCountAndReleasesCapture() throws {
    var posted: [CGEvent] = []
    let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
    let recognizer = makeRecognizer(tap)
    defer { recognizer.stop() }
    _ = tap.handleTapEvent(
      type: .leftMouseDown, event: try mouseEvent(.leftMouseDown, count: 2, flags: .maskShift))
    _ = tap.handleTapEvent(
      type: .leftMouseDragged,
      event: try mouseEvent(
        .leftMouseDragged, count: 2, location: CGPoint(x: 20, y: 70), flags: .maskAlternate))

    XCTAssertEqual(posted.map(\.type), [.leftMouseDown, .leftMouseDragged])
    XCTAssertEqual(posted.map { $0.getIntegerValueField(.mouseEventClickState) }, [2, 2])
    XCTAssertEqual(posted.map(\.flags), [.maskShift, .maskAlternate])
    XCTAssertTrue(
      tap.handleTapEvent(type: .leftMouseUp, event: try mouseEvent(.leftMouseUp, count: 2)) != nil)
    posted.removeAll()
    tap.replay(.click(button: .primary, at: .zero))
    XCTAssertEqual(posted.map { $0.getIntegerValueField(.mouseEventClickState) }, [1, 1])
  }

  func testStopClearsClickCountBeforeLaterManualReplay() throws {
    var posted: [CGEvent] = []
    let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
    let recognizer = makeRecognizer(tap)
    _ = tap.handleTapEvent(
      type: .leftMouseDown, event: try mouseEvent(.leftMouseDown, count: 3, flags: .maskShift))
    _ = tap.handleTapEvent(
      type: .leftMouseDragged,
      event: try mouseEvent(.leftMouseDragged, count: 3, flags: .maskControl))

    recognizer.stop()

    XCTAssertEqual(posted.first?.getIntegerValueField(.mouseEventClickState), 3)
    XCTAssertEqual(posted.map(\.flags), [.maskShift, .maskControl])
    posted.removeAll()
    tap.replay(.click(button: .primary, at: .zero))
    XCTAssertEqual(posted.map { $0.getIntegerValueField(.mouseEventClickState) }, [1, 1])
  }

  func testRecoveredDragKeepsCountOnDownMovementAndRelease() throws {
    var posted: [CGEvent] = []
    let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
    let recognizer = makeRecognizer(tap)
    defer { recognizer.stop() }
    _ = tap.handleTapEvent(type: .leftMouseDown, event: try mouseEvent(.leftMouseDown, count: 2))
    _ = tap.handleTapEvent(
      type: .leftMouseUp,
      event: try mouseEvent(.leftMouseUp, count: 2, location: CGPoint(x: 70, y: 30)))

    XCTAssertEqual(posted.map(\.type), [.leftMouseDown, .leftMouseDragged, .leftMouseUp])
    XCTAssertEqual(posted.map { $0.getIntegerValueField(.mouseEventClickState) }, [2, 2, 2])
  }

  func testReplacingPressRecoversOldCountBeforeUsingNewCount() throws {
    var posted: [CGEvent] = []
    let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
    let recognizer = makeRecognizer(tap)
    defer { recognizer.stop() }
    _ = tap.handleTapEvent(type: .leftMouseDown, event: try mouseEvent(.leftMouseDown, count: 2))
    _ = tap.handleTapEvent(type: .leftMouseDown, event: try mouseEvent(.leftMouseDown, count: 3))
    _ = tap.handleTapEvent(type: .leftMouseUp, event: try mouseEvent(.leftMouseUp, count: 3))

    XCTAssertEqual(posted.map { $0.getIntegerValueField(.mouseEventClickState) }, [2, 2, 3, 3])
  }

  func testSyntheticAndPassedThroughInputDoNotReplaceCapturedClick() throws {
    var posted: [CGEvent] = []
    let tap = GestureEventTap(capturedButtons: [.primary, .secondary]) { posted.append($0) }
    let recognizer = makeRecognizer(tap)
    defer { recognizer.stop() }
    _ = tap.handleTapEvent(type: .leftMouseDown, event: try mouseEvent(.leftMouseDown, count: 2))
    let replayed = try mouseEvent(.leftMouseDown, count: 99)
    ReplayedEventMarker.mark(replayed)
    XCTAssertTrue(tap.handleTapEvent(type: .leftMouseDown, event: replayed) != nil)
    XCTAssertTrue(
      tap.handleTapEvent(type: .rightMouseDown, event: try mouseEvent(.rightMouseDown, count: 7))
        != nil)
    _ = tap.handleTapEvent(type: .leftMouseUp, event: try mouseEvent(.leftMouseUp, count: 2))

    XCTAssertEqual(posted.map { $0.getIntegerValueField(.mouseEventClickState) }, [2, 2])
    posted.removeAll()
    tap.replay(.click(button: .secondary, at: .zero))
    XCTAssertEqual(posted.map { $0.getIntegerValueField(.mouseEventClickState) }, [1, 1])
  }

  func testNewPressDuringRecoveryKeepsItsOwnClickCount() throws {
    var posted: [CGEvent] = []
    var activeTap: GestureEventTap?
    let nextDown = try mouseEvent(.leftMouseDown, count: 3, flags: .maskAlternate)
    let tap = GestureEventTap(capturedButtons: [.primary]) { event in
      posted.append(event)
      if posted.count == 1 { _ = activeTap?.handleTapEvent(type: .leftMouseDown, event: nextDown) }
    }
    activeTap = tap
    let recognizer = makeRecognizer(tap)
    defer {
      recognizer.stop()
      activeTap = nil
    }
    _ = tap.handleTapEvent(
      type: .leftMouseDown, event: try mouseEvent(.leftMouseDown, count: 2, flags: .maskShift))
    _ = tap.handleTapEvent(
      type: .leftMouseUp, event: try mouseEvent(.leftMouseUp, count: 2, flags: .maskControl))
    _ = tap.handleTapEvent(
      type: .leftMouseUp, event: try mouseEvent(.leftMouseUp, count: 3, flags: .maskCommand))

    XCTAssertEqual(posted.map { $0.getIntegerValueField(.mouseEventClickState) }, [2, 2, 3, 3])
    XCTAssertEqual(posted.map(\.flags), [.maskShift, .maskControl, .maskAlternate, .maskCommand])
  }

  func testStoppedPressRecoveryDoesNotTakeMetadataFromAReentrantNewPress() throws {
    var posted: [CGEvent] = []
    var pending: [@MainActor @Sendable () -> Void] = []
    let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
    let nextDown = try mouseEvent(.leftMouseDown, count: 3, flags: .maskAlternate)
    tap.installHandler { _ in .consume }
    _ = tap.handleTapEvent(
      type: .leftMouseDown, event: try mouseEvent(.leftMouseDown, count: 2, flags: .maskShift))
    _ = tap.handleTapEvent(
      type: .leftMouseDragged,
      event: try mouseEvent(.leftMouseDragged, count: 2, flags: .maskControl))

    tap.stop {
      tap.installHandler { _ in .consume }
      _ = tap.handleTapEvent(type: .leftMouseDown, event: nextDown)
      pending.append(tap.prepareReplay(.click(button: .primary, at: .zero)))
    }
    _ = tap.handleTapEvent(
      type: .leftMouseDragged,
      event: try mouseEvent(.leftMouseDragged, count: 3, flags: .maskCommand))
    pending.append(tap.prepareReplay(.click(button: .primary, at: .zero)))

    XCTAssertTrue(posted.isEmpty)
    for deliver in pending { deliver() }
    XCTAssertEqual(posted.map { $0.getIntegerValueField(.mouseEventClickState) }, [2, 2, 3, 3])
    XCTAssertEqual(posted.map(\.flags), [.maskShift, .maskControl, .maskAlternate, .maskCommand])
  }

  func testNewInputReplayInsideStopRecoveryDoesNotTakeStoppedPressMetadata() throws {
    var posted: [CGEvent] = []
    var oldDelivery: (@MainActor @Sendable () -> Void)?
    var newDelivery: (@MainActor @Sendable () -> Void)?
    let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
    defer { tap.stop() }
    let nextDown = try mouseEvent(.leftMouseDown, count: 3, flags: .maskAlternate)
    let nextUp = try mouseEvent(.leftMouseUp, count: 3, flags: .maskCommand)
    tap.installHandler { _ in .consume }
    _ = tap.handleTapEvent(
      type: .leftMouseDown, event: try mouseEvent(.leftMouseDown, count: 2, flags: .maskShift))
    _ = tap.handleTapEvent(
      type: .leftMouseDragged,
      event: try mouseEvent(.leftMouseDragged, count: 2, flags: .maskControl))

    tap.stop {
      tap.installHandler { input in
        if case .buttonUp = input.kind {
          newDelivery = tap.prepareReplay(.click(button: .primary, at: .zero))
        }
        return .consume
      }
      _ = tap.handleTapEvent(type: .leftMouseDown, event: nextDown)
      _ = tap.handleTapEvent(type: .leftMouseUp, event: nextUp)
      oldDelivery = tap.prepareReplay(.click(button: .primary, at: .zero))
    }

    XCTAssertTrue(posted.isEmpty)
    try XCTUnwrap(oldDelivery)()
    try XCTUnwrap(newDelivery)()
    XCTAssertEqual(posted.map { $0.getIntegerValueField(.mouseEventClickState) }, [2, 2, 3, 3])
    XCTAssertEqual(posted.map(\.flags), [.maskShift, .maskControl, .maskAlternate, .maskCommand])
  }

  func testTapDisableReplayInsideStopRecoveryUsesNewPressMetadata() throws {
    var posted: [CGEvent] = []
    var oldDelivery: (@MainActor @Sendable () -> Void)?
    var newDelivery: (@MainActor @Sendable () -> Void)?
    let tap = GestureEventTap(capturedButtons: [.primary]) { posted.append($0) }
    defer { tap.stop() }
    let nextDown = try mouseEvent(.leftMouseDown, count: 3, flags: .maskAlternate)
    let nextMove = try mouseEvent(.leftMouseDragged, count: 3, flags: .maskCommand)
    tap.installHandler { _ in .consume }
    _ = tap.handleTapEvent(
      type: .leftMouseDown, event: try mouseEvent(.leftMouseDown, count: 2, flags: .maskShift))
    _ = tap.handleTapEvent(
      type: .leftMouseDragged,
      event: try mouseEvent(.leftMouseDragged, count: 2, flags: .maskControl))

    tap.stop {
      tap.installHandler { input in
        if case .cancel = input.kind {
          newDelivery = tap.prepareReplay(.click(button: .primary, at: .zero))
        }
        return .consume
      }
      _ = tap.handleTapEvent(type: .leftMouseDown, event: nextDown)
      _ = tap.handleTapEvent(type: .leftMouseDragged, event: nextMove)
      tap.recoverFromTapDisabled(
        input: GestureInputEvent(kind: .cancel, location: .zero, modifiers: []))
      oldDelivery = tap.prepareReplay(.click(button: .primary, at: .zero))
    }

    XCTAssertTrue(posted.isEmpty)
    try XCTUnwrap(oldDelivery)()
    try XCTUnwrap(newDelivery)()
    XCTAssertEqual(posted.map { $0.getIntegerValueField(.mouseEventClickState) }, [2, 2, 3, 3])
    XCTAssertEqual(posted.map(\.flags), [.maskShift, .maskControl, .maskAlternate, .maskCommand])
  }
}

@MainActor
private final class ReplayClock {
  var now: UInt64 = 0
}

@MainActor
private func makeRecognizer(
  _ tap: GestureEventTap,
  onReplay: (@MainActor @Sendable (GestureReplayRequest) -> Void)? = nil
) -> GestureRecognizer<String> {
  var matcher = GesturePatternMatcher<String>()
  matcher.register(pattern: [.right], match: "right")
  let recognizer = GestureRecognizer(
    eventSource: VirtualTapSource(tap),
    configuration: GestureRecognizerConfiguration(
      makeMatcher: { [matcher] _ in matcher },
      onReplayRequested: { request in
        if let onReplay { onReplay(request) } else { tap.replay(request) }
      }, onMatch: { _ in }, recognitionButton: .primary))
  recognizer.start()
  return recognizer
}

@MainActor
private final class VirtualTapSource: GestureEventSource {
  private let tap: GestureEventTap

  init(_ tap: GestureEventTap) { self.tap = tap }

  func start(
    handler: @escaping @MainActor @Sendable (GestureInputEvent) -> GestureEventDisposition
  ) -> Bool {
    tap.installHandler(handler)
    return true
  }

  func stop(recoveringInput: @MainActor () -> Void) {
    tap.stop(recoveringInput: recoveringInput)
  }
}

private func deadlineMouseEvent(
  _ type: CGEventType, count: Int64, location: CGPoint = CGPoint(x: 20, y: 30),
  flags: CGEventFlags = []
) throws -> CGEvent {
  let source = try XCTUnwrap(CGEventSource(stateID: .privateState))
  return try mouseEvent(type, count: count, location: location, flags: flags, source: source)
}

private func mouseEvent(
  _ type: CGEventType, count: Int64, location: CGPoint = CGPoint(x: 20, y: 30),
  flags: CGEventFlags = [], source: CGEventSource? = nil
) throws -> CGEvent {
  let event = try XCTUnwrap(
    CGEvent(
      mouseEventSource: source, mouseType: type, mouseCursorPosition: location,
      mouseButton: type == .rightMouseDown || type == .rightMouseUp ? .right : .left))
  event.setIntegerValueField(.mouseEventClickState, value: count)
  event.flags = flags
  return event
}
