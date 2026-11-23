/// A copied presentation snapshot and optional absolute monotonic deadline.
public struct GestureOverlayUpdate: Equatable, Sendable {
  /// Whether the currently presented trace changed during this call.
  public let changed: Bool
  /// Current presented geometry, or `nil` while hidden; valid independently of the reducer.
  public let trace: GestureOverlayTrace?
  /// Next absolute monotonic nanosecond time for a tick, or `nil` when none is needed.
  ///
  /// A pending deadline can change even when the presented trace does not.
  public let deadlineNanoseconds: UInt64?
}
