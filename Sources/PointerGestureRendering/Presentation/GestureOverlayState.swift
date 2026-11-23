import CPointerGestureRendering

/// Serial Rust presentation state. Hosts own clocks, scheduling and observation.
/// Call `tick` at the returned deadline; cancelling a host timer changes no Rust state.
public final class GestureOverlayState {
  private let handle: OpaquePointer
  private var trace: GestureOverlayTrace?
  /// Creates a serial reducer with normalized timing and optional initially visible geometry.
  ///
  /// Use one monotonic nanosecond time origin for construction, updates and ticks.
  /// Later clock regressions are clamped; the reducer creates no timers or callbacks.
  public init(
    timing: GestureOverlayTiming = .init(), trace: GestureOverlayTrace? = nil,
    nowNanoseconds: UInt64
  ) {
    self.trace = trace
    handle = pointer_gesture_rendering_overlay_create(
      timing.native, trace?.nativeHandle, nowNanoseconds)!
  }
  deinit { pointer_gesture_rendering_overlay_destroy(handle) }

  /// Submits visible geometry or `nil` to hide, at an absolute monotonic nanosecond time.
  ///
  /// Ordinary movement coalesces. Direction changes and rate-limited large movement
  /// publish immediately. Hiding first presents any pending final trace.
  /// Schedule a tick at the returned deadline even when `changed` is false.
  public func update(_ trace: GestureOverlayTrace?, nowNanoseconds: UInt64) -> GestureOverlayUpdate
  {
    decode(pointer_gesture_rendering_overlay_update(handle, trace?.nativeHandle, nowNanoseconds)!)
  }
  /// Immediately discards visible and queued geometry and cancels every deadline.
  /// Configuration is retained; this does not present a pending final frame.
  public func clear(nowNanoseconds: UInt64) -> GestureOverlayUpdate {
    decode(pointer_gesture_rendering_overlay_clear(handle, nowNanoseconds)!)
  }
  /// Publishes an elapsed pending update or hide at an absolute monotonic nanosecond time.
  ///
  /// Clock regressions are clamped and absolute deadlines saturate at `UInt64.max`.
  /// Returned snapshots remain valid after later calls or reducer destruction.
  public func tick(nowNanoseconds: UInt64) -> GestureOverlayUpdate {
    decode(pointer_gesture_rendering_overlay_tick(handle, nowNanoseconds)!)
  }
  /// Replaces cadence while preserving current/pending geometry and final hiding.
  /// Deadlines remain anchored to the last presentation. Schedule the returned deadline.
  public func configure(timing: GestureOverlayTiming, nowNanoseconds: UInt64)
    -> GestureOverlayUpdate
  {
    decode(pointer_gesture_rendering_overlay_configure(handle, timing.native, nowNanoseconds)!)
  }
  private func decode(_ frame: OpaquePointer) -> GestureOverlayUpdate {
    defer { pointer_gesture_rendering_overlay_frame_destroy(frame) }
    let view = pointer_gesture_rendering_overlay_frame_view(frame)
    if view.changed != 0 { trace = view.trace.map(GestureOverlayTrace.init(cloning:)) }
    return GestureOverlayUpdate(
      changed: view.changed != 0,
      trace: trace,
      deadlineNanoseconds: view.deadline_present == 0 ? nil : view.deadline_ns)
  }
}
