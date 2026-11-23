import CPointerGestureRecognition
import Foundation

/// Swift owns the handle; Rust alone owns recognition. No borrowed pointer escapes a call.
@MainActor
final class RustGestureCore<Match: Sendable> {
  private var handle: OpaquePointer?
  private var now: UInt64 = 0
  private var matcher = GesturePatternMatcher<Match>()
  private var inputMatcher: GesturePatternMatcher<Match>?
  private var inputGeneration: UInt64 = 0
  private let onAbandonedInput: @MainActor @Sendable ([GestureReplayRequest]) -> Void

  init?(
    config: PointerGestureRecognitionConfig,
    onAbandonedInput: @escaping @MainActor @Sendable ([GestureReplayRequest]) -> Void
  ) {
    guard pointer_gesture_recognition_abi_version() == POINTER_GESTURE_RECOGNITION_ABI_VERSION
    else { return nil }
    guard let handle = pointer_gesture_recognition_engine_create(config) else { return nil }
    self.handle = handle
    self.onAbandonedInput = onAbandonedInput
  }

  isolated deinit {
    if let frame = close() {
      onAbandonedInput(
        frame.deliveries.compactMap {
          if case .replay(let request) = $0 { request } else { nil }
        })
    }
  }

  func useMatcher(_ matcher: GesturePatternMatcher<Match>) {
    guard let handle else { return }
    matcher.withForeignHandle { pointer_gesture_recognition_engine_use_matcher(handle, $0) }
    self.matcher = matcher
  }

  func updateTuning(
    _ config: PointerGestureRecognitionConfig, now: UInt64, resetUnchanged: Bool = false
  ) -> Bool {
    guard let handle else { return false }
    self.now = now
    return pointer_gesture_recognition_engine_update_tuning(
      handle, config, now, resetUnchanged ? 1 : 0) == 1
  }

  func step(_ event: PointerGestureRecognitionEvent) -> GestureCoreFrame<Match>? {
    guard let handle else { return nil }
    now = event.now_ns
    let lookup = inputMatcher ?? matcher
    let frame = GestureCoreFrame(
      taking: pointer_gesture_recognition_engine_step(handle, event)!, resolve: lookup.value)
    if frame.inputGeneration == 0 {
      inputMatcher = nil
    } else if frame.inputGeneration != inputGeneration {
      inputMatcher = matcher
    }
    inputGeneration = frame.inputGeneration
    return frame
  }

  /// Consume once, before any host callbacks; repeated close and later input are harmless.
  func close() -> GestureCoreFrame<Match>? {
    guard let handle else { return nil }
    self.handle = nil
    let lookup = inputMatcher ?? matcher
    inputMatcher = nil
    let frame = pointer_gesture_recognition_engine_step(
      handle, .init(kind: 5, button: 0, x: 0, y: 0, now_ns: now, value: 0))!
    pointer_gesture_recognition_engine_destroy(handle)
    return GestureCoreFrame(taking: frame, resolve: lookup.value)
  }
}

enum GestureCoreDelivery<Match: Sendable>: Sendable {
  case replay(GestureReplayRequest)
  case match(Match)
  case trace(GestureRecognizerState)
}

/// A complete value copy releases the foreign frame before the host can reenter the adapter.
struct GestureCoreFrame<Match: Sendable>: Sendable {
  let disposition: UInt32
  let inputGeneration: UInt64
  let discardVisible: Bool
  let state: GestureRecognizerState
  let deadline: UInt64?
  let deliveries: [GestureCoreDelivery<Match>]

  fileprivate init(taking frame: OpaquePointer, resolve: (UInt64) -> Match?) {
    defer { pointer_gesture_recognition_frame_destroy(frame) }
    let value = pointer_gesture_recognition_frame_view(frame)
    disposition = value.disposition
    inputGeneration = value.input_generation
    discardVisible = value.discard_visible != 0
    deadline = value.deadline_present == 0 ? nil : value.deadline_ns
    // Copies can share Swift value storage when native views alias within this frame.
    // Every array is owned before the foreign frame is released or callbacks run.
    let finalState = GestureRecognizerState(
      status: .init(
        lifecycle: value.ready == 0 ? .idle : .ready,
        isCapturingGesture: value.capturing != 0,
        isRecordingModeEnabled: value.recording != 0,
        lastRecordedDirections: Self.directions(value.recorded),
        lastFailure: Self.failure(value.failure)),
      trace: .init(
        isVisible: value.visible != 0,
        discardsPendingPresentation: value.discard_visible != 0, rawPoints: Self.points(value.raw),
        directions: Self.directions(value.directions),
        directionEndpoints: Self.points(value.endpoints),
        tailPoint: value.tail_present == 0 ? nil : Self.point(value.tail)),
      lastSessionReport: GestureSessionReport(
        pointer_gesture_recognition_frame_session_report(frame)))
    state = finalState
    deliveries = (0..<value.effect_count).map { index in
      let effect = pointer_gesture_recognition_frame_effect(frame, index)
      switch effect.kind {
      case 5:
        guard let match = resolve(effect.match_value) else {
          preconditionFailure("Core match has no retained Swift command")
        }
        return .match(match)
      case 6:
        let trace = effect.trace
        return .trace(
          .init(
            status: .init(
              lifecycle: effect.ready == 0 ? .idle : .ready,
              isCapturingGesture: effect.capturing != 0,
              isRecordingModeEnabled: effect.recording != 0,
              lastRecordedDirections:
                effect.recorded.data == value.recorded.data
                && effect.recorded.count == value.recorded.count
                ? finalState.status.lastRecordedDirections : Self.directions(effect.recorded),
              lastFailure: Self.failure(effect.failure)),
            trace: .init(
              isVisible: trace.visible != 0,
              discardsPendingPresentation: value.discard_visible != 0,
              rawPoints: trace.raw.data == value.raw.data && trace.raw.count == value.raw.count
                ? finalState.trace.rawPoints : Self.points(trace.raw),
              directions:
                trace.directions.data == value.directions.data
                && trace.directions.count == value.directions.count
                ? finalState.trace.directions : Self.directions(trace.directions),
              directionEndpoints:
                trace.endpoints.data == value.endpoints.data
                && trace.endpoints.count == value.endpoints.count
                ? finalState.trace.directionEndpoints : Self.points(trace.endpoints),
              tailPoint: trace.tail_present == 0 ? nil : Self.point(trace.tail)),
            lastSessionReport: finalState.lastSessionReport))
      case 1...4:
        let button: PointerButton
        switch effect.button {
        case 0: button = .primary
        case 1: button = .secondary
        case 2: button = .middle
        default: button = PointerButton(auxiliaryButtonID: effect.button)!
        }
        let points = Self.points(effect.points)
        switch effect.kind {
        case 1: return .replay(.click(button: button, at: points[0]))
        case 2: return .replay(.drag(button: button, points: points))
        case 3: return .replay(.dragStart(button: button, points: points))
        default: return .replay(.release(button: button, at: points[0]))
        }
      default: preconditionFailure("Unknown core delivery")
      }
    }
  }

  private static func failure(_ code: UInt32) -> GestureRecognizerFailure? {
    switch code {
    case 0: nil
    case 1: .recognitionDisabled
    case 2: .modifiersNotSatisfied
    case 3: .gestureSessionExpired
    default: preconditionFailure("Unknown core failure")
    }
  }

  private static func directions(_ view: PointerGestureRecognitionDirectionsView)
    -> [GestureDirection]
  {
    UnsafeBufferPointer(start: view.data, count: view.count).map {
      GestureDirection(coreCode: $0)
    }
  }

  private static func point(_ value: PointerGestureRecognitionPoint) -> GesturePoint {
    .init(x: value.x, y: value.y)
  }

  private static func points(_ view: PointerGestureRecognitionPointsView) -> [GesturePoint] {
    UnsafeBufferPointer(start: view.data, count: view.count).map(point)
  }
}
