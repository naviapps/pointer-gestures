import Foundation
import Observation
import PointerGestureRendering

/// Main-actor observation and timer adapter for the shared Rust presentation reducer.
@MainActor
@Observable
public final class GestureOverlayModel {
  /// Currently presented geometry observed by SwiftUI, or `nil` while hidden.
  public private(set) var trace: GestureOverlayTrace?
  /// Current host-owned styling and cadence. Change through `configure(_:)`.
  public private(set) var configuration: GestureOverlayConfiguration
  @ObservationIgnored private let state: GestureOverlayState
  @ObservationIgnored private let clock = ContinuousClock()
  @ObservationIgnored private let origin: ContinuousClock.Instant
  @ObservationIgnored private var pendingUpdateTask: Task<Void, Never>?
  @ObservationIgnored private var scheduledDeadline: UInt64?
  @ObservationIgnored private var pendingInputs: [Input] = []
  @ObservationIgnored private var isDraining = false

  private enum Input {
    case trace(GestureOverlayTrace?)
    case clear
    case tick
    case configuration(GestureOverlayConfiguration)
  }

  isolated deinit { pendingUpdateTask?.cancel() }

  /// Creates an overlay adapter with an optional initially presented trace.
  ///
  /// The model owns observation and timer delivery; the host owns its window,
  /// recognition subscription and display coordinate mapping.
  public init(
    trace: GestureOverlayTrace? = nil, configuration: GestureOverlayConfiguration = .liveOverlay
  ) {
    self.trace = trace
    self.configuration = configuration
    origin = clock.now
    state = GestureOverlayState(timing: configuration.timing, trace: trace, nowNanoseconds: 0)
  }

  /// Submits visible geometry or `nil` to hide through the shared presentation reducer.
  ///
  /// Ordinary movement coalesces and hiding preserves a pending final trace briefly.
  /// Updates submitted from observation callbacks drain after the current publication.
  public func update(_ trace: GestureOverlayTrace?) {
    enqueue(.trace(trace))
  }

  /// Immediately discards visible/pending geometry and its scheduled presentation.
  public func clear() { enqueue(.clear) }

  /// Updates appearance/cadence without replacing the reducer or losing pending
  /// final geometry. Host windows and subscriptions keep their current lifetime.
  public func configure(_ configuration: GestureOverlayConfiguration) {
    enqueue(.configuration(configuration))
  }

  private func enqueue(_ input: Input) {
    pendingInputs.append(input)
    guard !isDraining else { return }
    isDraining = true
    defer {
      pendingInputs.removeAll(keepingCapacity: true)
      isDraining = false
    }
    // Observation callbacks run before a property write completes. Drain their
    // updates afterward so an older publication cannot overwrite newer state.
    var index = 0
    while index < pendingInputs.count {
      let input = pendingInputs[index]
      index += 1
      switch input {
      case .trace(let trace):
        apply(state.update(trace, nowNanoseconds: nowNanoseconds))
      case .clear:
        apply(state.clear(nowNanoseconds: nowNanoseconds))
      case .tick:
        apply(state.tick(nowNanoseconds: nowNanoseconds))
      case .configuration(let configuration):
        let update = state.configure(timing: configuration.timing, nowNanoseconds: nowNanoseconds)
        self.configuration = configuration
        apply(update)
      }
    }
  }

  private var nowNanoseconds: UInt64 {
    let value = origin.duration(to: clock.now).components
    return GestureOverlayTiming.elapsedNanoseconds(
      seconds: value.seconds, attoseconds: value.attoseconds)
  }

  private func apply(_ update: GestureOverlayUpdate) {
    if update.changed { trace = update.trace }
    guard scheduledDeadline != update.deadlineNanoseconds else { return }
    pendingUpdateTask?.cancel()
    pendingUpdateTask = nil
    scheduledDeadline = update.deadlineNanoseconds
    guard let deadline = scheduledDeadline else { return }
    let instant = origin.advanced(by: .nanoseconds(deadline))
    pendingUpdateTask = Task { @MainActor [weak self, clock] in
      do { try await clock.sleep(until: instant) } catch { return }
      guard !Task.isCancelled, let self else { return }
      pendingUpdateTask = nil
      scheduledDeadline = nil
      enqueue(.tick)
    }
  }
}
