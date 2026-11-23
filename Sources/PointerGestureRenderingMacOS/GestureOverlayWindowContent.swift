import PointerGestureRecognition
import PointerGestureRenderingSwiftUI

/// The same subscription lifetime is exercised with and without native resources.
@MainActor
final class GestureOverlayWindowContent {
  let model: GestureOverlayModel
  private var observation: GestureObservationToken?
  private var isClosed = false
  private var generation: UInt64 = 0

  init(traceSource: GestureOverlayWindowTraceSource, configuration: GestureOverlayConfiguration) {
    model = GestureOverlayModel(trace: traceSource.initialTrace, configuration: configuration)
    bind(traceSource)
  }

  isolated deinit { close() }

  func configure(
    traceSource: GestureOverlayWindowTraceSource, configuration: GestureOverlayConfiguration
  ) {
    guard !isClosed else { return }
    generation &+= 1
    let revision = generation
    let old = observation
    observation = nil
    old?.cancel()
    guard !isClosed, generation == revision else { return }
    model.configure(configuration)
    guard !isClosed, generation == revision else { return }
    // Binding supplies current input; do not clear the model's pending final frame.
    if traceSource.initiallyDiscardsPendingPresentation {
      model.clear()
    } else {
      model.update(traceSource.initialTrace)
    }
    guard !isClosed, generation == revision else { return }
    bind(traceSource)
  }

  private func bind(_ source: GestureOverlayWindowTraceSource) {
    let revision = generation
    let token = source.observe { [weak self] trace, discard in
      guard let self, !isClosed, generation == revision else { return }
      if discard { model.clear() } else { model.update(trace) }
    }
    guard !isClosed, generation == revision else {
      token.cancel()
      return
    }
    observation = token
  }

  func close() {
    guard !isClosed else { return }
    isClosed = true
    generation &+= 1
    let observation = observation
    self.observation = nil
    observation?.cancel()
  }
}
