import PointerGestureRecognition

/// Converts recognition snapshots to Rust-validated rendering traces.
/// Use one converter serially; unchanged direction runs reuse their Swift coordinate storage.
public final class GestureOverlayTraceConverter {
  private var previous: GestureOverlayTrace?

  /// Creates a converter without retaining a recognizer, observation or window.
  public init() {}

  /// Returns visible geometry. Hidden, empty or invalid input clears the cache and returns `nil`.
  /// Tail-only updates share geometry, and previously returned traces remain valid.
  /// Rust validates the geometry and determines when to reuse it.
  public func convert(_ trace: GestureRecognizerState.Trace) -> GestureOverlayTrace? {
    previous = GestureOverlayTrace(snapshot: trace, previous: previous)
    return previous
  }
}
