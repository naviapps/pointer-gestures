/// Read-only recognition state, independent of command values and input-source ownership.
/// Hosts can inject another implementation without wrapping the SDK's observation tokens.
/// Implementations must stop callbacks that have not started when a subscription's
/// token is cancelled on the main actor. A callback already running may finish.
@MainActor
public protocol GestureRecognizerObserving: AnyObject {
  /// Current state without advancing recognition or input deadlines.
  var snapshot: GestureRecognizerState { get }

  /// Observes full snapshots, synchronously delivering the initial state.
  /// Retain the token while observing. Later updates may coalesce.
  @discardableResult
  func observe(_ handler: @escaping @MainActor @Sendable (GestureRecognizerState) -> Void)
    -> GestureObservationToken

  /// Observes distinct status values, synchronously delivering the initial status.
  @discardableResult
  func observeStatus(
    _ handler: @escaping @MainActor @Sendable (GestureRecognizerState.Status) -> Void
  ) -> GestureObservationToken

  /// Observes distinct trace values, synchronously delivering the initial trace.
  /// A live input adapter must deliver final geometry and hiding in input order.
  @discardableResult
  func observeTrace(
    _ handler: @escaping @MainActor @Sendable (GestureRecognizerState.Trace) -> Void
  ) -> GestureObservationToken
}

extension GestureRecognizer: GestureRecognizerObserving {}

extension GestureRecognizerObserving {
  /// Filters status changes from full snapshots.
  @discardableResult
  public func observeStatus(
    _ handler: @escaping @MainActor @Sendable (GestureRecognizerState.Status) -> Void
  ) -> GestureObservationToken {
    let observer = DistinctGestureObserver<GestureRecognizerState.Status>()
    return observe { observer.deliver($0.status, to: handler) }
  }

  /// Filters trace changes from full snapshots for custom state providers.
  /// Providers needing immediate trace delivery should implement this requirement directly.
  @discardableResult
  public func observeTrace(
    _ handler: @escaping @MainActor @Sendable (GestureRecognizerState.Trace) -> Void
  ) -> GestureObservationToken {
    let observer = DistinctGestureObserver<GestureRecognizerState.Trace>()
    return observe { observer.deliver($0.trace, to: handler) }
  }
}

@MainActor
private final class DistinctGestureObserver<Value: Equatable> {
  private var last: Value?

  func deliver(_ value: Value, to handler: @MainActor (Value) -> Void) {
    guard last != value else { return }
    last = value
    handler(value)
  }
}
