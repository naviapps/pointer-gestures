import CoreGraphics

typealias OrderedReplayQueue =
  @MainActor @Sendable (@escaping @MainActor @Sendable () -> Void) -> Void

/// The retained queue owns a restored press and its physical continuation together.
@MainActor
final class OrderedDragReplay {
  @MainActor
  final class ReleaseBarrier {
    var isPending = true
    var didPost: (@MainActor () -> Void)?

    func finish() {
      isPending = false
      let notify = didPost
      didPost = nil
      notify?()
    }
  }

  private let enqueue: OrderedReplayQueue
  private let post: @MainActor @Sendable (CGEvent) -> Void
  private let fallbackRelease: CGEvent
  private let predecessor: ReleaseBarrier?
  private var motionQueued = false
  private var cancelled = false
  private var closed = false
  private var started = false

  init(
    enqueue: @escaping OrderedReplayQueue,
    post: @escaping @MainActor @Sendable (CGEvent) -> Void,
    fallbackRelease: CGEvent,
    predecessor: ReleaseBarrier? = nil
  ) {
    self.enqueue = enqueue
    self.post = post
    self.fallbackRelease = fallbackRelease
    self.predecessor = predecessor
  }

  func queueStart(_ deliver: @escaping @MainActor @Sendable () -> Void) {
    enqueue { [self] in
      guard !cancelled else { return }
      started = true
      deliver()
    }
  }

  func makeSuccessor(fallbackRelease: CGEvent, behind barrier: ReleaseBarrier) -> OrderedDragReplay
  {
    OrderedDragReplay(
      enqueue: enqueue, post: post, fallbackRelease: fallbackRelease, predecessor: barrier)
  }

  func queueMovement(_ original: CGEvent) {
    guard !closed else { return }
    let location = original.location
    fallbackRelease.flags = original.flags
    guard location.x.isFinite, location.y.isFinite else { return }
    fallbackRelease.location = location
    // Keep the first pending sample in its FIFO position. Later movement is optional;
    // it cannot overtake a different accepted batch or displace the terminal release.
    guard !motionQueued else { return }
    motionQueued = true
    let event = Self.copyForDelivery(original)
    enqueue { [self] in
      defer { motionQueued = false }
      if !cancelled { post(event) }
    }
  }

  func queueRelease(_ original: CGEvent) -> ReleaseBarrier? {
    guard !closed else { return nil }
    closed = true
    let location = original.location
    if location.x.isFinite, location.y.isFinite {
      return queueTerminal(Self.copyForDelivery(original))
    }
    fallbackRelease.flags = original.flags
    return queueTerminal(fallbackRelease)
  }

  func loseOwnership() -> ReleaseBarrier? {
    guard !closed else { return nil }
    closed = true
    guard started else {
      // The real release may pass the disconnected tap. Do not post a later Down.
      cancelled = true
      if let predecessor, predecessor.isPending { return predecessor }
      return nil
    }
    return queueTerminal(fallbackRelease)
  }

  private func queueTerminal(_ release: CGEvent) -> ReleaseBarrier {
    let barrier = ReleaseBarrier()
    let post = post
    enqueue {
      post(release)
      barrier.finish()
    }
    return barrier
  }

  static func copyForDelivery(_ original: CGEvent) -> CGEvent {
    // We suppress the original. If copying fails, retaining its CF event still gives
    // the guaranteed queue a terminal event instead of leaving an already posted Down held.
    let event = original.copy() ?? original
    ReplayedEventMarker.mark(event)
    return event
  }
}
