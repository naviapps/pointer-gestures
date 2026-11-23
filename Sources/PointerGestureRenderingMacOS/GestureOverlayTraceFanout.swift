import CoreGraphics
import Foundation
import PointerGestureRecognition
import PointerGestureRendering

/// A display-filtered trace snapshot and its cancellable, main-actor updates.
@MainActor
public struct GestureOverlayWindowTraceSource {
  /// The filtered trace captured when the source was created.
  public let initialTrace: GestureOverlayTrace?
  /// Whether a newly bound renderer must immediately discard earlier presentation.
  public let initiallyDiscardsPendingPresentation: Bool
  /// Primary-first AppKit frames defining the CGEvent display coordinate space.
  public let screenFrames: [CGRect]
  /// Registers distinct updates; a change since the initial snapshot is delivered synchronously.
  /// The Boolean requests immediate discard of visible/pending presentation,
  /// including when the filtered trace is already `nil`.
  /// Retain the returned token. Cancellation is synchronous and reentrant-safe.
  public let observe:
    @MainActor (@escaping @MainActor @Sendable (GestureOverlayTrace?, Bool) -> Void)
      -> GestureObservationToken
}

@MainActor
final class GestureOverlayTraceFanout {
  private let screenFrames: [CGRect]
  private var latestTrace: GestureOverlayTrace?
  private var discardsPendingPresentation: Bool
  private var subscribers: [UUID: GestureOverlayTraceFanoutSubscriber] = [:]
  private var subscriberIDs: [UUID] = []
  private var publishDepth = 0
  private var publicationRevision: UInt64 = 0
  private var removedSubscriberIDs: Set<UUID> = []
  private var traceObservation: GestureObservationToken?
  private var isStopped = false
  private let traceConverter = GestureOverlayTraceConverter()

  init(
    gestureRecognizer: any GestureRecognizerObserving,
    screenFrames: [CGRect]
  ) {
    self.screenFrames = screenFrames
    latestTrace = traceConverter.convert(gestureRecognizer.snapshot.trace)
    discardsPendingPresentation = gestureRecognizer.snapshot.trace.discardsPendingPresentation
    traceObservation = gestureRecognizer.observeTrace { [weak self] trace in
      guard let self else { return }
      publish(traceConverter.convert(trace), discard: trace.discardsPendingPresentation)
    }
  }

  isolated deinit { stop() }

  func stop() {
    guard !isStopped else { return }
    isStopped = true
    let observation = traceObservation
    traceObservation = nil
    publicationRevision &+= 1
    subscribers.removeAll()
    subscriberIDs.removeAll()
    removedSubscriberIDs.removeAll()
    observation?.cancel()
  }

  func source(
    visibleIn screenFrame: CGRect,
    lineWidth: CGFloat
  ) -> GestureOverlayWindowTraceSource {
    let displayCoordinateFrame = GestureOverlayWindow.displayCoordinateFrame(
      for: screenFrame,
      screenFrames: screenFrames
    )
    let traceFilter = GestureOverlayTraceFilter(
      displayCoordinateFrame: displayCoordinateFrame,
      lineWidth: lineWidth
    )
    let initialTrace = traceFilter.filteredTrace(latestTrace)
    let initialDiscard = discardsPendingPresentation
    return GestureOverlayWindowTraceSource(
      initialTrace: initialTrace,
      initiallyDiscardsPendingPresentation: initialDiscard,
      screenFrames: screenFrames,
      observe: { [weak self] handler in
        guard let self, !isStopped else { return GestureObservationToken {} }
        let id = UUID()
        let subscriber = GestureOverlayTraceFanoutSubscriber(
          traceFilter: traceFilter,
          lastDeliveredTrace: initialTrace,
          lastDiscard: initialDiscard,
          handler: handler
        )
        subscribers[id] = subscriber
        subscriberIDs.append(id)
        subscriber.deliver(latestTrace, discard: discardsPendingPresentation)
        return GestureObservationToken { [weak self] in
          self?.removeSubscriber(id)
        }
      }
    )
  }

  private func publish(_ trace: GestureOverlayTrace?, discard: Bool) {
    guard !isStopped else { return }
    guard latestTrace != trace || discardsPendingPresentation != discard else { return }
    latestTrace = trace
    discardsPendingPresentation = discard
    publicationRevision &+= 1
    let revision = publicationRevision

    publishDepth += 1
    defer {
      publishDepth -= 1
      if publishDepth == 0 {
        removeMarkedSubscribers()
      }
    }
    let subscriberCount = subscriberIDs.count
    var deliveryIndex = 0
    while deliveryIndex < subscriberCount {
      guard publicationRevision == revision else { return }
      let id = subscriberIDs[deliveryIndex]
      deliveryIndex += 1
      guard let subscriber = subscribers[id] else { continue }
      subscriber.deliver(trace, discard: discard)
    }
  }

  private func removeSubscriber(_ id: UUID) {
    guard subscribers.removeValue(forKey: id) != nil else { return }
    if publishDepth > 0 {
      removedSubscriberIDs.insert(id)
    } else {
      subscriberIDs.removeAll { $0 == id }
    }
  }

  private func removeMarkedSubscribers() {
    guard !removedSubscriberIDs.isEmpty else { return }
    subscriberIDs.removeAll { removedSubscriberIDs.contains($0) }
    removedSubscriberIDs.removeAll(keepingCapacity: true)
  }

}

@MainActor
private final class GestureOverlayTraceFanoutSubscriber {
  private let traceFilter: GestureOverlayTraceFilter
  private let handler: @MainActor @Sendable (GestureOverlayTrace?, Bool) -> Void
  private var lastDeliveredTrace: GestureOverlayTrace?
  private var lastDiscard: Bool

  init(
    traceFilter: GestureOverlayTraceFilter,
    lastDeliveredTrace: GestureOverlayTrace?,
    lastDiscard: Bool,
    handler: @escaping @MainActor @Sendable (GestureOverlayTrace?, Bool) -> Void
  ) {
    self.traceFilter = traceFilter
    self.lastDeliveredTrace = lastDeliveredTrace
    self.lastDiscard = lastDiscard
    self.handler = handler
  }

  func deliver(_ trace: GestureOverlayTrace?, discard: Bool) {
    let filteredTrace = traceFilter.filteredTrace(trace)
    guard lastDeliveredTrace != filteredTrace || lastDiscard != discard else { return }
    lastDeliveredTrace = filteredTrace
    lastDiscard = discard
    handler(filteredTrace, discard)
  }
}
