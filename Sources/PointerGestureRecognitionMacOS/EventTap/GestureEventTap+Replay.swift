import PointerGestureRecognition

import class CoreGraphics.CGEvent
import enum CoreGraphics.CGEventField
import class CoreGraphics.CGEventSource
import enum CoreGraphics.CGEventSourceStateID
import enum CoreGraphics.CGEventTapLocation
import enum CoreGraphics.CGEventType

private let maximumReplayDragPointCount = 64

extension GestureEventTap {
  struct ReplayBatch {
    let events: [CGEvent]
    let button: PointerButton
    let endsCapture: Bool
  }

  /// Replays the concrete Core Graphics events for a recognizer replay request.
  ///
  /// Click requests post down and up. Drags post down, movement and up; empty drags do nothing
  /// and single-point drags become clicks. Drag starts omit up, and releases post only up.
  /// Events are marked to bypass capture and preserve the captured press's click count.
  /// Down uses the original press's native modifier flags; movement and up use the latest captured
  /// pointer event's flags. Without captured input, replay uses a single click and source flags.
  /// A terminal batch posted during the original button-up callback may replace that event when
  /// the batch belongs to the same consumed press and capture has not reentered or stopped.
  public func replay(_ request: GestureReplayRequest) {
    enqueueReplay(request) { deliver in
      deliver()
      return true
    }
  }

  /// Offers a complete replay operation for guaranteed delivery during the recognizer callback.
  ///
  /// The synchronous `accept` callback must return `true` only after taking ownership for delivery
  /// exactly once on the main actor and in request order. Accepted operations must survive tap
  /// shutdown, owner teardown and cancellation of unrelated host work. On `false`, do not retain
  /// or execute the operation. Refusal leaves capture metadata available for fallback during
  /// the same callback. An invalid request does not call `accept`.
  ///
  /// Returns whether delivery was accepted. An accepted terminal batch may replace the original
  /// button-up only within the same input callback for a previously consumed press.
  @discardableResult
  public func enqueueReplay(
    _ request: GestureReplayRequest,
    accepting accept: (@escaping @MainActor @Sendable () -> Void) -> Bool
  ) -> Bool {
    guard let batch = replayBatch(for: request) else { return false }
    return acceptReplayBatch(batch, accepting: accept)
  }

  /// Enqueues a replay in a retained FIFO. An exact live drag-start replay also owns its
  /// original button's subsequent movement and release, so they cannot pass ahead of its Down.
  ///
  /// The queue must take each operation synchronously and run it later exactly once, in order on
  /// the main actor, even after the tap or host shuts down. It must not run the operation inside
  /// `inOrder`. Only one continued movement is pending at a time. The physical release is retained.
  /// If capture is lost before an open start is delivered, that start is discarded; otherwise a
  /// fallback release is queued. A pair whose physical release is already owned remains deliverable.
  @discardableResult
  public func enqueueReplayFollowingThrough(
    _ request: GestureReplayRequest,
    inOrder enqueue:
      @escaping @MainActor @Sendable (@escaping @MainActor @Sendable () -> Void) -> Void
  ) -> Bool {
    guard let batch = replayBatch(for: request) else { return false }
    if case .dragStart = request, let accepted = acceptOrderedDragStart(batch, enqueue: enqueue) {
      return accepted
    }
    return acceptReplayBatch(batch) { delivery in
      enqueue(delivery)
      return true
    }
  }

  func makeContinuationRelease(button: PointerButton, at location: GesturePoint) -> CGEvent? {
    replayBatch(for: .release(button: button, at: location))?.events.first
  }

  /// Captures a replay request's native events and returns their deferred delivery operation.
  ///
  /// Call this inside the recognizer's replay callback while the original input is still captured.
  /// Deliver the returned operation once, on the main actor and in request order. It retains its
  /// events, click count and modifier flags independently of later input, tap shutdown or deallocation.
  /// Preparing does not post input. Do not discard pending delivery when canceling unrelated work.
  /// Preparing or invoking this operation does not replace the original up. For a queue that
  /// guarantees delivery across teardown, use `enqueueReplay(_:accepting:)`.
  /// Empty or invalid requests return an operation that does nothing.
  public func prepareReplay(_ request: GestureReplayRequest) -> @MainActor @Sendable () -> Void {
    guard let batch = replayBatch(for: request) else { return {} }
    return prepareReplayBatch(batch)
  }

  private func replayBatch(for request: GestureReplayRequest) -> ReplayBatch? {
    let button: PointerButton
    let points: [GesturePoint]
    let postsDown: Bool
    let postsUp: Bool
    switch request {
    case .click(let capturedButton, let location):
      (button, points, postsDown, postsUp) = (capturedButton, [location], true, true)
    case .release(let capturedButton, let location):
      (button, points, postsDown, postsUp) = (capturedButton, [location], false, true)
    case .drag(let capturedButton, let capturedPoints):
      (button, points, postsDown, postsUp) = (
        capturedButton,
        coalescedReplayDragPoints(capturedPoints, maximumPointCount: maximumReplayDragPointCount),
        true, true
      )
    case .dragStart(let capturedButton, let capturedPoints):
      (button, points, postsDown, postsUp) = (
        capturedButton,
        coalescedReplayDragPoints(capturedPoints, maximumPointCount: maximumReplayDragPointCount),
        true, false
      )
    }
    guard let first = points.first, let last = points.last else { return nil }
    let source = replayEventSource()
    let types = button.cgEventTypes
    var events: [CGEvent] = []
    if postsDown {
      guard
        let down = replayedButtonEvent(type: types.down, button: button, at: first, source: source)
      else { return nil }
      events.append(down)
    }
    guard
      let movement = replayedButtonEvents(
        type: types.moved, button: button, at: points.dropFirst(), source: source)
    else { return nil }
    events.append(contentsOf: movement)
    if postsUp {
      guard let up = replayedButtonEvent(type: types.up, button: button, at: last, source: source)
      else { return nil }
      events.append(up)
    }
    return ReplayBatch(events: events, button: button, endsCapture: postsUp)
  }
}

private func replayEventSource() -> CGEventSource? {
  let source = CGEventSource(stateID: .hidSystemState)
  source?.localEventsSuppressionInterval = 0
  return source
}

private func replayedButtonEvent(
  type: CGEventType,
  button: PointerButton,
  at location: GesturePoint,
  source: CGEventSource?
) -> CGEvent? {
  guard location.isFinite else { return nil }
  guard let cgMouseButton = button.cgMouseButton else { return nil }
  guard
    let event = CGEvent(
      mouseEventSource: source,
      mouseType: type,
      mouseCursorPosition: location.cgPoint,
      mouseButton: cgMouseButton
    )
  else {
    return nil
  }
  ReplayedEventMarker.mark(event)
  return event
}

private func replayedButtonEvents(
  type: CGEventType,
  button: PointerButton,
  at locations: ArraySlice<GesturePoint>,
  source: CGEventSource?
) -> [CGEvent]? {
  var events: [CGEvent] = []
  events.reserveCapacity(locations.count)
  for location in locations {
    guard let event = replayedButtonEvent(type: type, button: button, at: location, source: source)
    else {
      return nil
    }
    events.append(event)
  }
  return events
}
