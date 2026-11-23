import AppKit

/// Owns invalidation subscriptions and coalescing, independently of overlay windows.
@MainActor
final class GestureOverlayInvalidationObserver {
  private let notificationCenter: NotificationCenter
  private let configurationChangeEvents: (@MainActor () -> AsyncStream<Void>)?
  private let coalescingDelayNanoseconds: UInt64
  private var screenParametersObservation: NSObjectProtocol?
  private var configurationChangeObservationTask: Task<Void, Never>?
  private var pendingRebuildTask: Task<Void, Never>?
  private var observationGeneration = 0
  private var onInvalidate: (@MainActor () -> Void)?

  init(
    notificationCenter: NotificationCenter,
    configurationChangeEvents: (@MainActor () -> AsyncStream<Void>)?,
    coalescingDelayNanoseconds: UInt64
  ) {
    self.notificationCenter = notificationCenter
    self.configurationChangeEvents = configurationChangeEvents
    self.coalescingDelayNanoseconds = coalescingDelayNanoseconds
  }

  isolated deinit { stop() }

  func start(onInvalidate: @escaping @MainActor () -> Void) {
    self.onInvalidate = onInvalidate
    guard screenParametersObservation == nil else { return }
    observationGeneration += 1
    let generation = observationGeneration
    screenParametersObservation = notificationCenter.addObserver(
      forName: NSApplication.didChangeScreenParametersNotification,
      object: nil,
      queue: .main
    ) { [weak self] _ in
      Task { @MainActor [weak self] in
        self?.scheduleRebuild(generation: generation)
      }
    }
    if let configurationChangeEvents {
      let events = configurationChangeEvents()
      let task = Task { @MainActor [weak self] in
        // Enter the captured stream even when already cancelled so its producer
        // receives termination. The factory must never run in this delayed task.
        for await _ in events {
          guard !Task.isCancelled else { return }
          self?.scheduleRebuild(generation: generation)
        }
      }
      if generation == observationGeneration {
        configurationChangeObservationTask = task
      } else {
        task.cancel()
      }
    }
  }

  func cancelPendingRebuild() {
    pendingRebuildTask?.cancel()
    pendingRebuildTask = nil
  }

  func stop() {
    observationGeneration += 1
    let screenObservation = screenParametersObservation
    screenParametersObservation = nil
    let configurationTask = configurationChangeObservationTask
    configurationChangeObservationTask = nil
    let rebuildTask = pendingRebuildTask
    pendingRebuildTask = nil
    onInvalidate = nil
    if let screenObservation { notificationCenter.removeObserver(screenObservation) }
    rebuildTask?.cancel()
    configurationTask?.cancel()
  }

  private func scheduleRebuild(generation: Int) {
    guard generation == observationGeneration, pendingRebuildTask == nil else { return }
    let delay = coalescingDelayNanoseconds
    pendingRebuildTask = Task { @MainActor [weak self] in
      do {
        try await Task.sleep(nanoseconds: delay)
      } catch {
        return
      }
      guard !Task.isCancelled, let self, generation == observationGeneration else { return }
      pendingRebuildTask = nil
      onInvalidate?()
    }
  }
}
