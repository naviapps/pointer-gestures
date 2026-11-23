import AppKit
import PointerGestureRecognition
import PointerGestureRenderingMacOS
import XCTest

let overlayTestDisplays = [
  GestureOverlayDisplay(
    frame: CGRect(x: 0, y: 0, width: 200, height: 100), maximumFramesPerSecond: 60),
  GestureOverlayDisplay(
    frame: CGRect(x: -200, y: 100, width: 200, height: 100), maximumFramesPerSecond: 120),
]

@MainActor
func waitUntil(timeout: TimeInterval = 1, _ condition: @MainActor () -> Bool) async {
  let clock = ContinuousClock()
  let deadline = clock.now.advanced(by: .seconds(timeout))
  while !condition(), clock.now < deadline { try? await Task.sleep(for: .milliseconds(2)) }
  XCTAssertTrue(condition())
}

@MainActor
func assertNotEventually(timeout: TimeInterval, _ condition: @MainActor () -> Bool) async {
  let clock = ContinuousClock()
  let deadline = clock.now.advanced(by: .seconds(timeout))
  while clock.now < deadline {
    if condition() {
      XCTFail("Unexpected change during quiet interval")
      return
    }
    try? await Task.sleep(for: .milliseconds(2))
  }
}

@MainActor
final class FixedResultGestureEventSource: GestureEventSource {
  let startResult: Bool

  private(set) var startCount = 0
  private(set) var stopCount = 0
  private var handler: (@MainActor @Sendable (GestureInputEvent) -> GestureEventDisposition)?

  init(startResult: Bool) {
    self.startResult = startResult
  }

  func start(
    handler: @escaping @MainActor @Sendable (GestureInputEvent) -> GestureEventDisposition
  )
    -> Bool
  {
    startCount += 1
    self.handler = startResult ? handler : nil
    return startResult
  }

  func stop(recoveringInput: @MainActor () -> Void) {
    stopCount += 1
    handler = nil
    recoveringInput()
  }

  func send(_ event: GestureInputEvent) -> GestureEventDisposition {
    handler?(event) ?? .passThrough
  }
}

@MainActor
final class GestureTestRecognizer: GestureRecognizerObserving {
  private var observers: [UUID: @MainActor @Sendable (GestureRecognizerState) -> Void] = [:]

  var isEnabled: Bool = false
  var isStarting: Bool = false
  var isRecording: Bool = false
  var isRecordingModeEnabled: Bool = false
  var lastRecordedDirections: [GestureDirection] = []
  var recognitionUnavailable: Bool = false
  var lastFailure: GestureRecognizerFailure?

  var isTraceVisible: Bool = false
  var points: [GesturePoint] = []
  var gestureDirections: [GestureDirection] = []
  var normalizedDirectionPoints: [GesturePoint] = []
  var tailPoint: GesturePoint?

  private(set) var startCount = 0
  private(set) var stopCount = 0
  private(set) var retryCount = 0
  private(set) var cancelActiveGestureCount = 0
  var observerCount: Int {
    observers.count
  }

  init() {}

  var snapshot: GestureRecognizerState {
    GestureRecognizerState(
      status: GestureRecognizerState.Status(
        lifecycle: isEnabled ? .ready : (isStarting ? .starting : .idle),
        isCapturingGesture: isRecording,
        isRecordingModeEnabled: isRecordingModeEnabled,
        lastRecordedDirections: lastRecordedDirections,
        lastFailure: recognitionUnavailable ? .eventSourceStartFailed : lastFailure
      ),
      trace: GestureRecognizerState.Trace(
        isVisible: isTraceVisible,
        rawPoints: points,
        directions: gestureDirections,
        directionEndpoints: normalizedDirectionPoints,
        tailPoint: tailPoint
      )
    )
  }

  func start() {
    startCount += 1
  }

  func stop() {
    stopCount += 1
  }

  func retryStartNow() {
    retryCount += 1
  }

  func cancelActiveGesture() {
    cancelActiveGestureCount += 1
    isRecording = false
    isTraceVisible = false
    points = []
    gestureDirections = []
    normalizedDirectionPoints = []
    tailPoint = nil
    notifyObservers()
  }

  @discardableResult
  func observe(
    _ handler: @escaping @MainActor @Sendable (GestureRecognizerState) -> Void
  ) -> GestureObservationToken {
    let id = UUID()
    observers[id] = handler
    handler(snapshot)
    return GestureObservationToken(onCancel: { [weak self] in
      self?.observers.removeValue(forKey: id)
    })
  }

  func notifyObservers() {
    let current = snapshot
    for id in Array(observers.keys) {
      observers[id]?(current)
    }
  }
}
