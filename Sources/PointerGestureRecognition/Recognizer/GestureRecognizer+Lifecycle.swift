extension GestureRecognizer {
  /// Starts gesture recognition.
  ///
  /// Calling this while the recognizer is starting or ready has no effect. Calling it again after a
  /// failed or retrying start request retries startup immediately. When the event source cannot
  /// start, the recognizer records ``GestureRecognizerFailure/eventSourceStartFailed``. It follows
  /// the configured retry schedule when retry delays are present, or moves to a failed lifecycle
  /// when no retry delay is configured.
  public func start() {
    guard requestStartup(.start) else { return }
    cancelStartupRetry()
    attemptStartupOrScheduleRetry()
  }

  /// Stops gesture recognition and clears active state.
  ///
  /// This cancels pending startup retries, clears pending gesture state, and requests the replay
  /// needed when consumed pointer-button input is active.
  public func stop() {
    guard requestStartup(.stop) else { return }
    let revision = lifecycleRevision
    stopRuntimeResources()
    if lifecycleRevision == revision { lastFailure = nil }
  }

  /// Immediately retries startup after a previous ``start()`` request.
  ///
  /// Calling this before ``start()`` or while the recognizer is ready has no effect.
  public func retryStartNow() {
    guard requestStartup(.retryNow) else { return }
    cancelStartupRetry()
    attemptStartupOrScheduleRetry()
  }

  /// Cancels the active gesture session and releases any held pointer-button state.
  ///
  /// When consumed pointer-button input is active, this requests the replay needed to leave host
  /// input state consistent.
  ///
  /// Calling this while no gesture session or pending button input exists has no effect and
  /// preserves the current failure state.
  public func cancelActiveGesture() {
    guard inputGeneration != 0 || snapshot.status.isCapturingGesture else { return }
    _ = deliver(kind: 4)
  }

  private func attemptEventSourceStart() -> Bool {
    eventSource.start { [weak self] input in
      guard let self else { return .passThrough }
      return handleInputEvent(input)
    }
  }

  func stopRuntimeResources() {
    cancelStartupRetry()
    eventSource.stop { _ = deliver(kind: 5) }
  }

  @discardableResult
  private func requestStartup(_ request: RustStartup.Request, revision: UInt64 = 0) -> Bool {
    let previous = lifecycleState
    let accepted = startup.request(request, revision: revision)
    if previous != lifecycleState { scheduleSnapshot() }
    return accepted
  }

  private func attemptStartupOrScheduleRetry() {
    let revision = lifecycleRevision
    let started = attemptEventSourceStart()
    guard requestStartup(started ? .succeeded : .failed, revision: revision) else { return }
    if started {
      _ = deliver(kind: 6)
      // Delivery may reenter stop/start; an older completion must not clear it.
      if lifecycleRevision == revision { lastFailure = nil }
    } else {
      lastFailure = .eventSourceStartFailed
      scheduleStartupRetry()
    }
  }

  func scheduleStartupRetry() {
    cancelStartupRetry()
    guard let retry = startup.nextRetry() else { return }
    startupRetryTask = Task { @MainActor [weak self] in
      if retry.delay == 0 {
        await Task.yield()
      } else {
        do { try await Task.sleep(nanoseconds: retry.delay) } catch { return }
      }
      guard !Task.isCancelled, let self,
        requestStartup(.scheduledRetry, revision: retry.revision)
      else { return }
      startupRetryTask = nil
      attemptStartupOrScheduleRetry()
    }
  }

  func cancelStartupRetry() {
    startupRetryTask?.cancel()
    startupRetryTask = nil
  }
}
