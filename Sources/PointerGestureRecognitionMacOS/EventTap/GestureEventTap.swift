import CoreFoundation
import CoreGraphics
import Foundation
import PointerGestureRecognition

/// Captures macOS Core Graphics events for pointer gesture recognition.
///
/// The tap translates configured pointer-button mouse events and initial Escape presses into
/// platform-neutral `GestureInputEvent` values synchronously on the main run loop.
/// Teardown is isolated to the main actor;
/// call `stop()` to end capture before releasing the source.
@MainActor
public final class GestureEventTap: GestureEventSource {
  let capturedButtons: Set<PointerButton>
  private var tapPort: CFMachPort?
  private var runLoopSource: CFRunLoopSource?
  private var handler: (@MainActor @Sendable (GestureInputEvent) -> GestureEventDisposition)?
  private let postReplayedEvent: @MainActor @Sendable (CGEvent) -> Void
  private var capturedPresses: [PointerButton: [CapturedPress]] = [:]
  private var recoveryPresses: [[PointerButton: [CapturedPress]]] = []
  private var replayCaptureContext: ReplayCaptureContext = .live
  private var dispatchDepth = 0
  private var releaseDispatch: ReleaseDispatch?
  private var moveDispatch: (button: PointerButton, capture: CapturedPress)?
  private var orderedReleases:
    [PointerButton: (OrderedDragReplay.ReleaseBarrier, OrderedDragReplay)] = [:]
  private var escapePresses = EscapePresses()

  private struct EscapeKeySource: Hashable, Sendable {
    let processIdentifier: Int64
    let keyboardType: Int64

    init(_ event: CGEvent) {
      processIdentifier = event.getIntegerValueField(.eventSourceUnixProcessID)
      keyboardType = event.getIntegerValueField(.keyboardEventKeyboardType)
    }
  }

  private final class EscapePresses {
    var consumed: [EscapeKeySource: CGEventTimestamp] = [:]
  }

  private enum ReplayCaptureContext {
    case live
    case recovery(Int)
  }

  private struct ReleaseDispatch {
    let button: PointerButton
    let capture: CapturedPress
    var didAcceptTerminalReplay = false
  }

  @MainActor
  private final class CapturedPress {
    let clickCount: Int64
    let downFlags: CGEventFlags
    var latestFlags: CGEventFlags
    var consumedDown = false
    var isRetired = false
    var orderedDrag: OrderedDragReplay?

    init(clickCount: Int64, flags: CGEventFlags) {
      self.clickCount = max(1, clickCount)
      downFlags = flags
      latestFlags = flags
    }
  }

  /// Creates an event tap for the provided pointer buttons.
  ///
  /// Passing an empty set creates a tap configuration that captures no input.
  public convenience init(capturedButtons: Set<PointerButton> = [.secondary]) {
    self.init(capturedButtons: capturedButtons) { event in
      event.post(tap: .cghidEventTap)
    }
  }

  init(
    capturedButtons: Set<PointerButton>,
    postReplayedEvent: @escaping @MainActor @Sendable (CGEvent) -> Void
  ) {
    self.capturedButtons = capturedButtons
    self.postReplayedEvent = postReplayedEvent
  }

  isolated deinit { stop() }

  /// Starts the macOS event tap.
  ///
  /// A successful start replaces any previously active event tap and handler.
  /// Returns `false` when no pointer buttons are configured, or when Core Graphics cannot create or
  /// install the event tap, for example when the process is missing the required Accessibility
  /// permission.
  public func start(
    handler: @escaping @MainActor @Sendable (GestureInputEvent) -> GestureEventDisposition
  ) -> Bool {
    stop()
    guard !capturedButtons.isEmpty else { return false }
    guard let port = makeTapPort() else { return false }

    guard let source = makeRunLoopSource(for: port) else {
      CFMachPortInvalidate(port)
      return false
    }
    activateTapPort(port, source: source, handler: handler)
    return true
  }

  /// Disconnects the macOS event tap before delivering consumed-input recovery.
  ///
  /// The handler and native resources are released first. Capture metadata remains available to
  /// replay APIs only for the duration of `recoveringInput`.
  /// The closure runs exactly once, even when already stopped. A new capture started by the closure
  /// remains running after this method returns.
  public func stop(recoveringInput: @MainActor () -> Void) {
    releaseDispatch = nil
    moveDispatch = nil
    let interruptedDrags = takeOrderedDrags()
    if let port = tapPort {
      CGEvent.tapEnable(tap: port, enable: false)
      CFMachPortInvalidate(port)
      tapPort = nil
    }
    if let source = runLoopSource {
      CFRunLoopRemoveSource(CFRunLoopGetMain(), source, .commonModes)
      runLoopSource = nil
    }
    handler = nil
    escapePresses = EscapePresses()
    recoveryPresses.append(capturedPresses)
    capturedPresses.removeAll()
    defer { recoveryPresses.removeLast() }
    for (button, drag) in interruptedDrags {
      retainOrderedRelease(drag.loseOwnership(), drag, button)
    }
    withReplayCaptureContext(.recovery(recoveryPresses.count - 1), recoveringInput)
  }

  private func reenableTapPort() {
    guard let port = tapPort else { return }
    CGEvent.tapEnable(tap: port, enable: true)
  }

  private func makeTapPort() -> CFMachPort? {
    CGEvent.tapCreate(
      tap: .cgSessionEventTap,
      place: .headInsertEventTap,
      options: .defaultTap,
      eventsOfInterest: eventMask,
      callback: makeTapCallback(),
      userInfo: Unmanaged.passUnretained(self).toOpaque()
    )
  }

  private func makeRunLoopSource(for port: CFMachPort) -> CFRunLoopSource? {
    CFMachPortCreateRunLoopSource(kCFAllocatorDefault, port, 0)
  }

  private func activateTapPort(
    _ port: CFMachPort,
    source: CFRunLoopSource,
    handler: @escaping @MainActor @Sendable (GestureInputEvent) -> GestureEventDisposition
  ) {
    installHandler(handler)
    CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
    tapPort = port
    runLoopSource = source
    CGEvent.tapEnable(tap: port, enable: true)
  }

  private func makeTapCallback() -> CGEventTapCallBack {
    { _, type, event, refcon -> Unmanaged<CGEvent>? in
      guard let refcon else { return Unmanaged.passUnretained(event) }
      let gestureEventTap = Unmanaged<GestureEventTap>
        .fromOpaque(refcon)
        .takeUnretainedValue()
      return gestureEventTap.handleTapEvent(type: type, event: event)
    }
  }

  nonisolated func handleTapEvent(
    type: CGEventType,
    event: CGEvent
  ) -> Unmanaged<CGEvent>? {
    // The source is registered only with the main run loop. Never defer an event
    // from another thread into a later capture session.
    guard Thread.isMainThread else { return Unmanaged.passUnretained(event) }

    if let mappedTapDisabledInput = mappedTapDisabledInputEvent(for: type, event: event) {
      MainActor.assumeIsolated {
        recoverFromTapDisabled(
          input: mappedTapDisabledInput.input
        )
      }
      return Unmanaged.passUnretained(event)
    }

    if type == .keyDown || type == .keyUp {
      guard let phase = mappedEscapeKeyPhase(for: type, event: event) else {
        return Unmanaged.passUnretained(event)
      }
      let source = EscapeKeySource(event)
      let timestamp = event.timestamp
      let consumed = MainActor.assumeIsolated {
        handleEscapeKey(phase, source: source, timestamp: timestamp)
      }
      return consumed ? nil : Unmanaged.passUnretained(event)
    }

    guard let mappedInputEvent = mappedInputEvent(for: type, event: event) else {
      return Unmanaged.passUnretained(event)
    }

    let clickCount: Int64 =
      switch mappedInputEvent.input.kind {
      case .buttonDown: event.getIntegerValueField(.mouseEventClickState)
      default: 1
      }
    let flags = event.flags
    // The main-thread guard and synchronous actor check keep this borrowed reference on
    // the callback's thread. The event outlives the block; only actor-owned replay work is queued.
    nonisolated(unsafe) let rawEvent = Unmanaged.passUnretained(event)
    let disposition = withExtendedLifetime(event) {
      MainActor.assumeIsolated {
        if !mappedInputEvent.isReplayed,
          continueOrderedDrag(mappedInputEvent.input, rawEvent: rawEvent.takeUnretainedValue())
        {
          return GestureEventDisposition.consume
        }
        return dispatch(mappedInputEvent, clickCount: clickCount, flags: flags)
      }
    }
    if disposition.consumesOriginalEvent {
      return nil
    }
    return Unmanaged.passUnretained(event)
  }

  private func handleEscapeKey(
    _ phase: EscapeKeyPhase, source: EscapeKeySource, timestamp: CGEventTimestamp
  ) -> Bool {
    let presses = escapePresses
    if let previous = presses.consumed[source], timestamp < previous { return false }
    switch phase {
    case .down(let input):
      // A fresh down supersedes a press whose up was never seen.
      presses.consumed.removeValue(forKey: source)
      let disposition = dispatch(
        MappedInputEvent(input: input, isReplayed: false), clickCount: 1, flags: [])
      if disposition.consumesOriginalEvent {
        // A handler that stops or replaces capture cannot arm the new tap's state.
        presses.consumed[source] = timestamp
      }
      return disposition.consumesOriginalEvent
    case .repeatedDown:
      return presses.consumed[source] != nil
    case .up:
      return presses.consumed.removeValue(forKey: source) != nil
    }
  }

  private func dispatch(
    _ event: MappedInputEvent, clickCount: Int64, flags: CGEventFlags
  ) -> GestureEventDisposition {
    guard !event.isReplayed else { return .passThrough }
    // Reentrant input cannot acknowledge the outer event's release.
    releaseDispatch = nil
    moveDispatch = nil
    dispatchDepth += 1
    defer {
      releaseDispatch = nil
      moveDispatch = nil
      dispatchDepth -= 1
      if dispatchDepth == 0 {
        // Older captures are needed only while reentrant recovery callbacks drain.
        for (button, captures) in capturedPresses where captures.count > 1 {
          capturedPresses[button] = Array(captures.suffix(1))
        }
      }
    }
    let button: PointerButton? =
      switch event.input.kind {
      case .buttonDown(let button), .buttonMoved(let button), .buttonUp(let button): button
      case .cancel: nil
      }
    var capture = button.flatMap { capturedPresses[$0]?.first }
    if let button, case .buttonDown = event.input.kind {
      let next = CapturedPress(clickCount: clickCount, flags: flags)
      capturedPresses[button, default: []].append(next)
      capture = next
    } else if button != nil {
      capture?.latestFlags = flags
    }

    if case .buttonUp(let button) = event.input.kind, event.input.location.isFinite,
      dispatchDepth == 1, let capture, capture.consumedDown
    {
      releaseDispatch = ReleaseDispatch(button: button, capture: capture)
    }
    if case .buttonMoved(let button) = event.input.kind, event.input.location.isFinite,
      dispatchDepth == 1, let capture, capture.consumedDown
    {
      moveDispatch = (button, capture)
    }
    let disposition = withReplayCaptureContext(.live) { handler?(event.input) ?? .passThrough }
    let endsCapture =
      switch event.input.kind {
      case .buttonUp: true
      case .buttonDown: !disposition.consumesOriginalEvent
      default: false
      }
    if case .buttonDown = event.input.kind {
      capture?.consumedDown = disposition.consumesOriginalEvent
    }
    if case .buttonMoved = event.input.kind, capture?.orderedDrag != nil {
      return .consume
    }
    if endsCapture, let button, let capture { remove(capture, for: button) }
    return releaseDispatch?.didAcceptTerminalReplay == true ? .consume : disposition
  }

  func acceptReplayBatch(
    _ batch: ReplayBatch, accepting accept: (@escaping @MainActor @Sendable () -> Void) -> Bool
  ) -> Bool {
    let (deliver, capture) = makeReplayDelivery(batch)
    guard batch.endsCapture, let capture else { return accept(deliver) }

    let retire: @MainActor @Sendable () -> Void = { [weak self, button = batch.button] in
      self?.remove(capture, for: button)
    }
    // A synchronous consumer retires before posting so reentrant input uses the new press.
    // A queued consumer retires only after acceptance. Refusal leaves the original press intact.
    guard
      accept({
        retire()
        deliver()
      })
    else { return false }
    retire()
    // Posting or handing off the batch may synchronously reenter or replace capture.
    if case .live = replayCaptureContext,
      releaseDispatch?.button == batch.button, releaseDispatch?.capture === capture
    {
      releaseDispatch?.didAcceptTerminalReplay = true
    }
    return true
  }

  func prepareReplayBatch(_ batch: ReplayBatch) -> @MainActor @Sendable () -> Void {
    let (deliver, capture) = makeReplayDelivery(batch)
    if batch.endsCapture, let capture { remove(capture, for: batch.button) }
    return deliver
  }

  func acceptOrderedDragStart(_ batch: ReplayBatch, enqueue: @escaping OrderedReplayQueue) -> Bool?
  {
    guard case .live = replayCaptureContext, dispatchDepth == 1,
      let moveDispatch, moveDispatch.button == batch.button,
      capturedPresses[batch.button]?.first === moveDispatch.capture,
      moveDispatch.capture.orderedDrag == nil
    else { return nil }
    let capture = moveDispatch.capture
    guard let last = batch.events.last,
      let release = makeContinuationRelease(button: batch.button, at: GesturePoint(last.location))
    else { return false }
    release.flags = capture.latestFlags
    release.setIntegerValueField(.mouseEventClickState, value: capture.clickCount)
    let (deliver, _) = makeReplayDelivery(batch)
    let drag = OrderedDragReplay(
      enqueue: enqueue, post: postReplayedEvent, fallbackRelease: release)
    capture.orderedDrag = drag
    drag.queueStart(deliver)
    return true
  }

  private func continueOrderedDrag(_ input: GestureInputEvent, rawEvent: CGEvent) -> Bool {
    let button: PointerButton
    switch input.kind {
    case .buttonDown(let value), .buttonMoved(let value), .buttonUp(let value): button = value
    case .cancel: return false
    }
    if orderedReleases[button]?.0.isPending == false { orderedReleases[button] = nil }
    guard let capture = capturedPresses[button]?.first, let drag = capture.orderedDrag else {
      if case .buttonDown = input.kind, let (barrier, previous) = orderedReleases[button] {
        return queueOrderedReplacement(
          input, rawEvent: rawEvent, button: button, after: previous, barrier: barrier)
      }
      return false
    }
    switch input.kind {
    case .buttonMoved:
      drag.queueMovement(rawEvent)
    case .buttonUp:
      capture.orderedDrag = nil
      remove(capture, for: button)
      retainOrderedRelease(drag.queueRelease(rawEvent), drag, button)
    case .buttonDown:
      capture.orderedDrag = nil
      remove(capture, for: button)
      let barrier = drag.loseOwnership()
      retainOrderedRelease(barrier, drag, button)
      // A fresh Down can pass normally if there was never an older synthetic Down.
      // Otherwise keep it (and its own tail) behind the older synthetic release.
      guard let barrier else { return false }
      return queueOrderedReplacement(
        input, rawEvent: rawEvent, button: button, after: drag, barrier: barrier)
    case .cancel: return false
    }
    return true
  }

  private func queueOrderedReplacement(
    _ input: GestureInputEvent, rawEvent: CGEvent, button: PointerButton,
    after drag: OrderedDragReplay,
    barrier: OrderedDragReplay.ReleaseBarrier
  ) -> Bool {
    guard let release = makeContinuationRelease(button: button, at: input.location) else {
      // Do not post a new Down without a release available if capture is interrupted.
      return true
    }
    let clickCount = rawEvent.getIntegerValueField(.mouseEventClickState)
    let next = CapturedPress(clickCount: clickCount, flags: rawEvent.flags)
    next.consumedDown = true
    release.flags = next.downFlags
    release.setIntegerValueField(.mouseEventClickState, value: next.clickCount)
    let following = drag.makeSuccessor(fallbackRelease: release, behind: barrier)
    next.orderedDrag = following
    capturedPresses[button, default: []].append(next)
    let event = OrderedDragReplay.copyForDelivery(rawEvent)
    let post = postReplayedEvent
    following.queueStart { post(event) }
    return true
  }

  private func retainOrderedRelease(
    _ barrier: OrderedDragReplay.ReleaseBarrier?, _ drag: OrderedDragReplay, _ button: PointerButton
  ) {
    guard let barrier, barrier.isPending else { return }
    orderedReleases[button] = (barrier, drag)
    barrier.didPost = { [weak self, weak barrier] in
      guard let self, let barrier, orderedReleases[button]?.0 === barrier else { return }
      orderedReleases[button] = nil
    }
  }

  private func takeOrderedDrags() -> [(PointerButton, OrderedDragReplay)] {
    var drags: [(PointerButton, OrderedDragReplay)] = []
    for (button, captures) in capturedPresses {
      for capture in captures {
        guard let drag = capture.orderedDrag else { continue }
        capture.orderedDrag = nil
        remove(capture, for: button)
        drags.append((button, drag))
      }
    }
    return drags
  }

  private func makeReplayDelivery(_ batch: ReplayBatch) -> (
    deliver: @MainActor @Sendable () -> Void, capture: CapturedPress?
  ) {
    let (events, button) = (batch.events, batch.button)
    let capture =
      switch replayCaptureContext {
      case .live: capturedPresses[button]?.first
      case .recovery(let index): recoveryPresses[index][button]?.first
      }
    for event in events {
      event.setIntegerValueField(.mouseEventClickState, value: capture?.clickCount ?? 1)
      if let capture {
        switch event.type {
        case .leftMouseDown, .rightMouseDown, .otherMouseDown: event.flags = capture.downFlags
        default: event.flags = capture.latestFlags
        }
      }
    }
    let post = postReplayedEvent
    return ({ for event in events { post(event) } }, capture)
  }

  private func remove(_ capture: CapturedPress, for button: PointerButton) {
    guard !capture.isRetired else { return }
    capture.isRetired = true
    capturedPresses[button]?.removeAll { $0 === capture }
    if capturedPresses[button]?.isEmpty == true { capturedPresses[button] = nil }
    for index in recoveryPresses.indices {
      recoveryPresses[index][button]?.removeAll { $0 === capture }
    }
  }

  private func withReplayCaptureContext<Result>(
    _ context: ReplayCaptureContext, _ body: @MainActor () -> Result
  ) -> Result {
    let previous = replayCaptureContext
    replayCaptureContext = context
    defer { replayCaptureContext = previous }
    return body()
  }

  func installHandler(
    _ handler: @escaping @MainActor @Sendable (GestureInputEvent) -> GestureEventDisposition
  ) {
    releaseDispatch = nil
    moveDispatch = nil
    escapePresses = EscapePresses()
    self.handler = handler
  }

  func recoverFromTapDisabled(
    input: GestureInputEvent
  ) {
    releaseDispatch = nil
    moveDispatch = nil
    escapePresses = EscapePresses()
    let interruptedDrags = takeOrderedDrags()
    for (button, drag) in interruptedDrags {
      retainOrderedRelease(drag.loseOwnership(), drag, button)
    }
    _ = withReplayCaptureContext(.live) { handler?(input) }
    reenableTapPort()
  }
}
