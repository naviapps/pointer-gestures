/// A cancellable token that keeps a recognizer observation active.
@MainActor
public final class GestureObservationToken {
  private var onCancel: (@MainActor @Sendable () -> Void)?

  /// Creates a token for a custom observation. The handler runs on the main actor
  /// exactly once, on cancellation or release. Retain the token while observing.
  public init(onCancel: @escaping @MainActor @Sendable () -> Void) {
    self.onCancel = onCancel
  }

  isolated deinit { cancel() }

  /// Cancels synchronously on the main actor. Repeated cancellation has no effect.
  /// For recognizer subscriptions, no further callback starts after this returns,
  /// including during an active notification. A callback already running may finish.
  public func cancel() {
    let onCancel = onCancel
    self.onCancel = nil
    onCancel?()
  }
}
