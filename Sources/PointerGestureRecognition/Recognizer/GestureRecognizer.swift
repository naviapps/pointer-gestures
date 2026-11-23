import CPointerGestureRecognition
import Foundation

/// Main-actor host for the Rust recognizer, source lifecycle and observable UI state.
///
/// Cancelling an observation token synchronously skips callbacks that have not
/// started, including during the current notification. Running callbacks may finish.
@MainActor
public final class GestureRecognizer<Match: Sendable> {
  let eventSource: any GestureEventSource
  let configuration: GestureRecognizerConfiguration<Match>
  private let core: RustGestureCore<Match>
  private let clockOrigin = ContinuousClock.now
  var nowOverride: (@MainActor @Sendable () -> UInt64)?
  private var state = GestureRecognizerState(status: .init(), trace: .init())
  private let observationCenter = GestureRecognizerObservationCenter()
  private var sourceFailure: GestureRecognizerFailure?
  private var epoch: UInt64 = 0
  private var matchRevision: UInt64 = 0
  private var configuredPressRevision: UInt64 = 0
  private(set) var inputGeneration: UInt64 = 0
  private(set) var nextDeadline: UInt64?
  private var timerGeneration: UInt64 = 0
  private var inputExpirationTask: Task<Void, Never>?
  var startupRetryTask: Task<Void, Never>?
  let startup: RustStartup
  var lifecycleRevision: UInt64 { startup.view.revision }
  private(set) var tuning: GestureRecognizerTuning
  var lifecycleState: GestureRecognizerState.Status.Lifecycle { startup.lifecycle }
  var isReady: Bool { lifecycleState == .ready }
  var lastFailure: GestureRecognizerFailure? {
    get { sourceFailure ?? state.status.lastFailure }
    set {
      sourceFailure = newValue
      scheduleSnapshot()
    }
  }

  /// Recording mode for future input sessions; active input keeps its captured mode.
  ///
  /// Recording bypasses matcher and modifier approval while respecting enablement policy.
  public var isRecordingModeEnabled: Bool {
    get { state.status.isRecordingModeEnabled }
    set {
      if newValue != state.status.isRecordingModeEnabled {
        _ = deliver(kind: 7, value: newValue ? 1 : 0, preservingMatch: true)
      }
    }
  }

  /// Creates a stopped recognizer with host callbacks, policy and exclusive source ownership.
  ///
  /// Retain it while capture is needed. Call `start()` explicitly and `stop()` before
  /// releasing the source, delivering recovery through the configured replay callback.
  /// Deinitialization also attempts recovery but may be scheduled later on the main actor.
  public init(
    eventSource: any GestureEventSource, configuration: GestureRecognizerConfiguration<Match>
  ) {
    self.eventSource = eventSource
    self.configuration = configuration
    tuning = configuration.tuning
    startup = RustStartup(delays: configuration.tuning.eventSourceStartRetryDelays)
    core = RustGestureCore(
      config: Self.coreConfig(configuration.tuning, configuration),
      onAbandonedInput: { requests in requests.forEach(configuration.onReplayRequested) })!
  }

  isolated deinit {
    startupRetryTask?.cancel()
    inputExpirationTask?.cancel()
    eventSource.stop {
      if let frame = core.close() {
        for delivery in frame.deliveries {
          if case .replay(let request) = delivery { configuration.onReplayRequested(request) }
        }
      }
    }
  }

  /// Current copied lifecycle, recognition and trace state without expiring input.
  ///
  /// Reading a snapshot does not start the event source or replace deadline delivery.
  public var snapshot: GestureRecognizerState {
    return .init(
      status: .init(
        lifecycle: lifecycleState,
        isCapturingGesture: state.status.isCapturingGesture,
        isRecordingModeEnabled: state.status.isRecordingModeEnabled,
        lastRecordedDirections: state.status.lastRecordedDirections,
        lastFailure: lastFailure), trace: state.trace, lastSessionReport: state.lastSessionReport)
  }

  /// Observes full snapshots, synchronously delivering the initial value.
  ///
  /// Later snapshot notifications are scheduled and visible updates may coalesce.
  /// Retain the token while observing; cancellation or token release removes the subscription.
  @discardableResult
  public func observe(_ handler: @escaping @MainActor @Sendable (GestureRecognizerState) -> Void)
    -> GestureObservationToken
  {
    observationCenter.observeSnapshot(currentState: snapshot, handler: handler)
  }

  /// Observes distinct trace values, synchronously delivering the initial trace.
  ///
  /// Visible tail updates and hiding are delivered during input handling. Handlers
  /// may reenter recognition; retain the token while observation is needed.
  @discardableResult
  public func observeTrace(
    _ handler: @escaping @MainActor @Sendable (GestureRecognizerState.Trace) -> Void
  ) -> GestureObservationToken {
    observationCenter.observeTrace(currentTrace: state.trace, handler: handler)
  }

  /// Applies changed tuning and restarts current input deadlines from the current time.
  ///
  /// A smaller raw point limit trims the current trace immediately and bounds replay.
  /// Returns `false` for unchanged tuning. Pending startup retries use the new schedule;
  /// this does not change the configured button, matcher callback or host policy.
  @discardableResult
  public func updateTuning(_ tuning: GestureRecognizerTuning) -> Bool {
    guard tuning != self.tuning else { return false }
    self.tuning = tuning
    _ = core.updateTuning(Self.coreConfig(tuning, configuration), now: now, resetUnchanged: true)
    inputExpirationTask?.cancel()
    inputExpirationTask = nil
    timerGeneration = 0
    _ = deliver(kind: 9, preservingMatch: true)
    if startup.setDelays(tuning.eventSourceStartRetryDelays) == 2 {
      scheduleSnapshot()
      scheduleStartupRetry()
    }
    return true
  }

  func handleInputEvent(_ event: GestureInputEvent) -> GestureEventDisposition {
    guard isReady else { return .passThrough }
    // Configured motion, releases and cancellation let Rust check expiration before
    // recovery callbacks can start replacement input.
    switch event.kind {
    case .cancel: return deliver(kind: 4) == 1 ? .consume : .passThrough
    case .buttonMoved(let button) where button == configuration.recognitionButton:
      return deliver(kind: 2, point: event.location) == 1 ? .consume : .passThrough
    case .buttonUp(let button) where button == configuration.recognitionButton:
      return deliver(kind: 3, point: event.location) == 1 ? .consume : .passThrough
    case .buttonMoved, .buttonUp:
      if event.location.isFinite { _ = deliver(kind: 10) }
      return .passThrough
    case .buttonDown(let button):
      guard event.location.isFinite else { return .passThrough }
      guard button == configuration.recognitionButton else {
        _ = deliver(kind: 10)
        return .passThrough
      }
      configuredPressRevision &+= 1
      let pressRevision = configuredPressRevision
      let sourceRevision = lifecycleRevision
      _ = deliver(kind: UInt32(POINTER_GESTURE_RECOGNITION_PREPARE_REPLACEMENT_PRESS))
      // Recovery can update policy. Only a newer press or source lifetime invalidates this press.
      guard configuredPressRevision == pressRevision, lifecycleRevision == sourceRevision, isReady
      else { return .passThrough }
      let operation = epoch
      let context = configuration.recognitionContext(event.location)
      guard epoch == operation, isReady else { return .passThrough }
      let enabled = configuration.isRecognitionEnabled(context)
      guard epoch == operation, isReady else { return .passThrough }
      let modifiers =
        !enabled || isRecordingModeEnabled
        || configuration.areModifiersSatisfied(event.modifiers, context)
      guard epoch == operation, isReady else { return .passThrough }
      if enabled && modifiers && !isRecordingModeEnabled {
        let matcher = configuration.makeMatcher(context)
        guard epoch == operation, isReady else { return .passThrough }
        core.useMatcher(matcher)
      }
      _ = deliver(kind: 8, value: (enabled ? 1 : 0) | (modifiers ? 2 : 0))
      return deliver(kind: 1, point: event.location) == 1 ? .consume : .passThrough
    }
  }

  @discardableResult
  func deliver(
    kind: UInt32, point: GesturePoint = .zero, value: UInt32 = 0, preservingMatch: Bool = false
  ) -> UInt32 {
    epoch &+= 1
    let operation = epoch
    // Recording mode and tuning updates make state projections stale without cancelling a match
    // already completed by Rust. Input and lifecycle operations still supersede queued callbacks.
    if !preservingMatch { matchRevision = epoch }
    let matchOperation = matchRevision
    let frame = core.step(
      .init(
        kind: kind, button: Self.buttonID(configuration.recognitionButton),
        x: point.x, y: point.y, now_ns: now, value: value))!
    inputGeneration = frame.inputGeneration
    nextDeadline = frame.deadline
    if frame.discardVisible { observationCenter.discardPendingVisibleTraceSnapshots() }
    for delivery in frame.deliveries {
      switch delivery {
      case .replay(let request): configuration.onReplayRequested(request)
      case .match(let match): if matchRevision == matchOperation { configuration.onMatch(match) }
      case .trace(let publication):
        guard epoch == operation else { continue }
        publish(publication, immediate: true, operation: operation)
      }
    }
    if epoch == operation {
      publish(frame.state, immediate: false, operation: operation)
      scheduleExpiration()
    }
    return frame.disposition
  }

  private func publish(_ next: GestureRecognizerState, immediate: Bool, operation: UInt64) {
    let previous = snapshot
    state = next
    let current = snapshot
    if previous != current { observationCenter.scheduleCurrentSnapshot(current) }
    if previous.trace.isVisible != current.trace.isVisible {
      observationCenter.enqueueSnapshot(current)
    }
    // A nested stop/cancel can clear a completed Rust input whose earlier trace
    // publication is still being delivered by Swift. Hide that projection too.
    guard
      immediate || previous.trace.isVisible != current.trace.isVisible
        || current.trace.discardsPendingPresentation,
      previous.trace != current.trace
    else { return }
    observationCenter.deliverTrace(current.trace) { observer in
      self.epoch == operation
        && (current.trace.discardsPendingPresentation
          || current.trace.isVisible && !previous.trace.isVisible
          || observer.hasDeliveredVisibleTrace)
    }
  }

  func scheduleSnapshot() { observationCenter.scheduleCurrentSnapshot(snapshot) }

  private func scheduleExpiration() {
    guard inputGeneration != 0, nextDeadline != nil else {
      inputExpirationTask?.cancel()
      inputExpirationTask = nil
      timerGeneration = 0
      return
    }
    guard timerGeneration != inputGeneration || inputExpirationTask == nil else { return }
    inputExpirationTask?.cancel()
    timerGeneration = inputGeneration
    let generation = inputGeneration
    inputExpirationTask = Task { @MainActor [weak self] in
      while !Task.isCancelled {
        guard let deadline = self?.nextDeadline, self?.inputGeneration == generation,
          let now = self?.now
        else { return }
        if now >= deadline {
          _ = self?.deliver(kind: 10)
          return
        }
        do { try await Task.sleep(nanoseconds: deadline - now) } catch { return }
      }
    }
  }

  private var now: UInt64 {
    if let nowOverride { return nowOverride() }
    let elapsed = clockOrigin.duration(to: .now).components
    return pointer_gesture_recognition_elapsed_nanoseconds(elapsed.seconds, elapsed.attoseconds)
  }

  private static func coreConfig(
    _ tuning: GestureRecognizerTuning, _ configuration: GestureRecognizerConfiguration<Match>
  ) -> PointerGestureRecognitionConfig {
    .init(
      start_distance: tuning.minimumGestureStartAxisDistance,
      change_distance: tuning.minimumDirectionChangeAxisDistance,
      total_ns: tuning.maximumGestureSessionDuration.map(
        GestureRecognizerTiming.taskSleepNanoseconds) ?? 0,
      idle_ns: tuning.maximumGestureIdleDuration.map(GestureRecognizerTiming.taskSleepNanoseconds)
        ?? 0,
      maximum_points: tuning.maximumRawPointCount,
      button: buttonID(configuration.recognitionButton),
      pass_empty: configuration.passesThroughEmptyMatcher ? 1 : 0,
      maximum_directions: tuning.maximumDirectionCount)
  }

  private static func buttonID(_ button: PointerButton) -> UInt32 {
    if button == .primary { return 0 }
    if button == .secondary { return 1 }
    if button == .middle { return 2 }
    return button.auxiliaryButtonID!
  }
}
