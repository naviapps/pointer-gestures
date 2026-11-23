/// Supplies normalized input events from a live platform source.
///
/// Event sources are main-actor reference types because capture lifecycle and delivery are tied to
/// the UI event loop.
@MainActor
public protocol GestureEventSource: AnyObject {
  /// Starts the event source with the supplied event handler.
  ///
  /// Implementations should pass normalized input events to the handler and apply the returned
  /// disposition to the original platform event: ``GestureEventDisposition/consume`` suppresses
  /// it, and ``GestureEventDisposition/passThrough`` leaves it in the platform event stream.
  /// A successful start replaces any previously active handler.
  /// Returns `false` when the source cannot start, for example because the host app lacks required
  /// platform permission. A source that returns `false` must not deliver events until a later
  /// successful start.
  func start(handler: @escaping @MainActor @Sendable (GestureInputEvent) -> GestureEventDisposition)
    -> Bool

  /// Stops capture, then synchronously delivers recovery for consumed input.
  ///
  /// Disconnect live input and clear the handler before invoking `recoveringInput` exactly once,
  /// including when already stopped. Retain capture metadata needed by recovery until it returns.
  /// A start requested from recovery begins a new capture that this stop must leave running.
  func stop(recoveringInput: @MainActor () -> Void)
}

extension GestureEventSource {
  /// Stops the event source without requesting additional input recovery.
  ///
  /// Calling this when already stopped is safe. Stop the owning recognizer instead when it may
  /// have consumed input that requires recovery.
  public func stop() { stop(recoveringInput: {}) }
}
