import PointerGestureRecognition
import XCTest

@MainActor
final class GestureRecognizerObservingTests: XCTestCase {
  func testCustomSourceFiltersStatusAndTraceAndCancelsSynchronously() {
    let source = StateSource()
    let observer: any GestureRecognizerObserving = source
    var statuses: [GestureRecognizerState.Status] = []
    var traces: [GestureRecognizerState.Trace] = []
    let status = observer.observeStatus { statuses.append($0) }
    let trace = observer.observeTrace { traces.append($0) }
    XCTAssertEqual(statuses, [.init()])
    XCTAssertEqual(traces, [.init()])

    let visible = GestureRecognizerState.Trace(
      isVisible: true, directions: [.right],
      directionEndpoints: [.zero, GesturePoint(x: 40, y: 0)])
    source.publish(.init(status: .init(), trace: visible))
    source.publish(.init(status: .init(lifecycle: .ready), trace: visible))
    source.publish(source.snapshot)
    XCTAssertEqual(statuses.map(\.lifecycle), [.idle, .ready])
    XCTAssertEqual(traces, [.init(), visible])

    status.cancel()
    trace.cancel()
    XCTAssertEqual(source.observers.count, 0)
    source.publish(.init(status: .init(), trace: .init()))
    XCTAssertEqual(statuses.count, 2)
    XCTAssertEqual(traces.count, 2)
  }
}

@MainActor
private final class StateSource: GestureRecognizerObserving {
  var snapshot = GestureRecognizerState(status: .init(), trace: .init())
  var observers: [UUID: @MainActor @Sendable (GestureRecognizerState) -> Void] = [:]

  func observe(_ handler: @escaping @MainActor @Sendable (GestureRecognizerState) -> Void)
    -> GestureObservationToken
  {
    let id = UUID()
    observers[id] = handler
    handler(snapshot)
    return GestureObservationToken { [weak self] in self?.observers[id] = nil }
  }

  func publish(_ state: GestureRecognizerState) {
    snapshot = state
    for id in Array(observers.keys) { observers[id]?(state) }
  }
}
