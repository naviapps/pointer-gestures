import AppKit
import PointerGestureRecognition
import PointerGestureRendering
import PointerGestureRenderingSwiftUI
import XCTest

@testable import PointerGestureRenderingMacOS

@MainActor
final class GestureOverlayCoordinatorTests: XCTestCase {

  func testConfigureCreatesHiddenWindowPerScreenAndRemoveClosesWindows() throws {
    let screens = overlayTestDisplays

    var created: [FakeOverlayWindow] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { screens },
      makeWindow: { _, screen, _ in
        let window = FakeOverlayWindow(screen: screen)
        created.append(window)
        return window
      }
    )

    coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    XCTAssertEqual(created.count, screens.count)

    coordinator.removeOverlays()
    XCTAssertTrue(created.allSatisfy { $0.closeCount == 1 })
  }

  func testConfigurePassesScreenSpecificConfigurationToEachWindow() throws {
    let screens = overlayTestDisplays

    var receivedScreens: [GestureOverlayDisplay] = []
    var receivedIntervals: [TimeInterval] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { screen in
        receivedScreens.append(screen)
        return GestureOverlayConfiguration(
          minimumAnimationInterval: 0.01 + Double(receivedScreens.count) * 0.001
        )
      },
      screensProvider: { screens },
      makeWindow: { _, screen, configuration in
        receivedIntervals.append(configuration.minimumAnimationInterval)
        return FakeOverlayWindow(screen: screen)
      }
    )

    coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())

    XCTAssertEqual(receivedScreens, screens)
    XCTAssertEqual(receivedIntervals.count, screens.count)
    for (index, interval) in receivedIntervals.enumerated() {
      XCTAssertEqual(interval, 0.01 + Double(index + 1) * 0.001, accuracy: 0.0001)
    }
  }

  func testConfigureUsesSingleRecognizerTraceObservationAcrossOverlayWindows() throws {
    let screens = overlayTestDisplays

    let recognizer = TraceObservationCountingGestureRecognizer()
    var created: [ObservingFakeOverlayWindow] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { screens },
      makeWindow: { traceSource, screen, _ in
        let window = ObservingFakeOverlayWindow(traceSource: traceSource, screen: screen)
        created.append(window)
        return window
      }
    )

    coordinator.configureOverlays(gestureRecognizer: recognizer)

    XCTAssertEqual(created.count, screens.count)
    XCTAssertEqual(recognizer.observerCount, 1)
    XCTAssertTrue(created.allSatisfy { $0.observedTraceCount == 0 })

    coordinator.removeOverlays()

    XCTAssertEqual(recognizer.observerCount, 0)
    XCTAssertTrue(created.allSatisfy { $0.closeCount == 1 })
  }

  func testConfigureTwiceRemovesPreviousWindows() throws {
    let screens = overlayTestDisplays

    var created: [FakeOverlayWindow] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { screens },
      makeWindow: { _, screen, _ in
        let window = FakeOverlayWindow(screen: screen)
        created.append(window)
        return window
      }
    )

    coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    let firstBatch = created

    coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    XCTAssertTrue(firstBatch.allSatisfy { $0.closeCount == 1 })
  }

  func testConfigureTwiceReleasesPreviousRecognizerTraceObservation() throws {
    let screens = overlayTestDisplays

    let firstRecognizer = TraceObservationCountingGestureRecognizer()
    let secondRecognizer = TraceObservationCountingGestureRecognizer()
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { screens },
      makeWindow: { _, screen, _ in
        FakeOverlayWindow(screen: screen)
      }
    )

    coordinator.configureOverlays(gestureRecognizer: firstRecognizer)

    XCTAssertEqual(firstRecognizer.observerCount, 1)
    XCTAssertEqual(secondRecognizer.observerCount, 0)

    coordinator.configureOverlays(gestureRecognizer: secondRecognizer)

    XCTAssertEqual(firstRecognizer.observerCount, 0)
    XCTAssertEqual(secondRecognizer.observerCount, 1)

    coordinator.removeOverlays()

    XCTAssertEqual(secondRecognizer.observerCount, 0)
  }

  func testReleaseRemovesActiveOverlayWindows() throws {
    let screens = overlayTestDisplays

    var created: [FakeOverlayWindow] = []
    var coordinator: GestureOverlayCoordinator? = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { screens },
      makeWindow: { _, screen, _ in
        let window = FakeOverlayWindow(screen: screen)
        created.append(window)
        return window
      }
    )

    coordinator?.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    XCTAssertEqual(created.count, screens.count)

    coordinator = nil

    XCTAssertNil(coordinator)
    XCTAssertTrue(created.allSatisfy { $0.closeCount == 1 })
  }

  func testReleaseAfterScreenParameterChangeDoesNotRebuildOverlays() async throws {
    let screens = overlayTestDisplays

    let notificationCenter = NotificationCenter()
    var created: [FakeOverlayWindow] = []
    var coordinator: GestureOverlayCoordinator? = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { screens },
      notificationCenter: notificationCenter,
      makeWindow: { _, screen, _ in
        let window = FakeOverlayWindow(screen: screen)
        created.append(window)
        return window
      }
    )

    coordinator?.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    let createdCountAfterConfigure = created.count

    notificationCenter.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)
    coordinator = nil

    await assertNotEventually(timeout: 0.1) {
      created.count > createdCountAfterConfigure
    }

    XCTAssertNil(coordinator)
    XCTAssertEqual(created.count, createdCountAfterConfigure)
    XCTAssertTrue(created.allSatisfy { $0.closeCount == 1 })
  }

  func testScreenParameterChangeUpdatesResourcesInPlace() async throws {
    let screens = overlayTestDisplays

    let notificationCenter = NotificationCenter()
    var created: [FakeOverlayWindow] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { screens },
      notificationCenter: notificationCenter,
      makeWindow: { _, screen, _ in
        let window = FakeOverlayWindow(screen: screen)
        created.append(window)
        return window
      }
    )

    coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    let firstBatch = created

    notificationCenter.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)

    await waitUntil {
      created.allSatisfy { $0.configureCount == 1 }
    }
    XCTAssertTrue(firstBatch.allSatisfy { $0.closeCount == 0 })
    XCTAssertEqual(created.count, screens.count)
  }

  func testRapidScreenParameterChangesAreCoalescedIntoSingleUpdate() async throws {
    let screens = overlayTestDisplays

    let notificationCenter = NotificationCenter()
    var created: [FakeOverlayWindow] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { screens },
      notificationCenter: notificationCenter,
      makeWindow: { _, screen, _ in
        let window = FakeOverlayWindow(screen: screen)
        created.append(window)
        return window
      }
    )

    coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    let initialCount = created.count

    notificationCenter.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)
    notificationCenter.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)
    notificationCenter.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)

    await waitUntil {
      created.allSatisfy { $0.configureCount == 1 }
    }
    await assertNotEventually(timeout: 0.1) {
      created.contains { $0.configureCount > 1 }
    }

    XCTAssertEqual(created.count, initialCount)
  }

  func testConfigurationChangeUpdatesResourcesInPlace() async throws {
    let screens = overlayTestDisplays

    let configurationChanges = OverlayConfigurationChangeEmitter()
    var created: [FakeOverlayWindow] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { screens },
      configurationChangeEvents: { configurationChanges.events },
      makeWindow: { _, screen, _ in
        let window = FakeOverlayWindow(screen: screen)
        created.append(window)
        return window
      }
    )

    coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    let firstBatch = created

    configurationChanges.send()

    await waitUntil {
      created.allSatisfy { $0.configureCount == 1 }
    }
    XCTAssertTrue(firstBatch.allSatisfy { $0.closeCount == 0 })
    XCTAssertEqual(created.count, screens.count)
  }

  func testRapidConfigurationChangesAreCoalescedIntoSingleUpdate() async throws {
    let screens = overlayTestDisplays

    let configurationChanges = OverlayConfigurationChangeEmitter()
    var created: [FakeOverlayWindow] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { screens },
      configurationChangeEvents: { configurationChanges.events },
      makeWindow: { _, screen, _ in
        let window = FakeOverlayWindow(screen: screen)
        created.append(window)
        return window
      }
    )

    coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    let initialCount = created.count

    configurationChanges.send()
    configurationChanges.send()
    configurationChanges.send()

    await waitUntil {
      created.allSatisfy { $0.configureCount == 1 }
    }
    await assertNotEventually(timeout: 0.1) {
      created.contains { $0.configureCount > 1 }
    }

    XCTAssertEqual(created.count, initialCount)
  }

  func testScreenParameterChangeIsIgnoredAfterRemovingOverlays() async throws {
    let screens = overlayTestDisplays

    let notificationCenter = NotificationCenter()
    var created: [FakeOverlayWindow] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { screens },
      notificationCenter: notificationCenter,
      makeWindow: { _, screen, _ in
        let window = FakeOverlayWindow(screen: screen)
        created.append(window)
        return window
      }
    )

    coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    coordinator.removeOverlays()
    let createdCountAfterRemoval = created.count

    notificationCenter.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)
    await assertNotEventually(timeout: 0.1) {
      created.count > createdCountAfterRemoval
    }

    XCTAssertEqual(created.count, createdCountAfterRemoval)
  }

  func testConfigurationChangeIsIgnoredAfterRemovingOverlays() async throws {
    let screens = overlayTestDisplays

    let configurationChanges = OverlayConfigurationChangeEmitter()
    var created: [FakeOverlayWindow] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { screens },
      configurationChangeEvents: { configurationChanges.events },
      makeWindow: { _, screen, _ in
        let window = FakeOverlayWindow(screen: screen)
        created.append(window)
        return window
      }
    )

    coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    coordinator.removeOverlays()
    let createdCountAfterRemoval = created.count

    configurationChanges.send()
    await assertNotEventually(timeout: 0.1) {
      created.count > createdCountAfterRemoval
    }

    XCTAssertEqual(created.count, createdCountAfterRemoval)
  }

  func testStaleScreenParameterChangeDoesNotRebuildAfterRemoveAndReconfigure() async throws {
    let screens = overlayTestDisplays

    let notificationCenter = NotificationCenter()
    var created: [FakeOverlayWindow] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { screens },
      notificationCenter: notificationCenter,
      makeWindow: { _, screen, _ in
        let window = FakeOverlayWindow(screen: screen)
        created.append(window)
        return window
      }
    )

    coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    let firstBatch = created

    notificationCenter.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)
    coordinator.removeOverlays()
    coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())

    let createdCountAfterReconfigure = created.count
    let secondBatch = Array(created.suffix(screens.count))
    await assertNotEventually(timeout: 0.1) {
      created.count > createdCountAfterReconfigure
    }

    XCTAssertEqual(created.count, createdCountAfterReconfigure)
    XCTAssertTrue(firstBatch.allSatisfy { $0.closeCount == 1 })
    XCTAssertTrue(secondBatch.allSatisfy { $0.closeCount == 0 })
  }

  func testScreenAndAppearanceChangesShareOneCoalescedUpdate() async throws {
    let screens = overlayTestDisplays
    let notificationCenter = NotificationCenter()
    let changes = OverlayConfigurationChangeEmitter()
    var created: [FakeOverlayWindow] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay }, screensProvider: { screens },
      notificationCenter: notificationCenter,
      configurationChangeEvents: { changes.events },
      makeWindow: { _, screen, _ in
        let window = FakeOverlayWindow(screen: screen)
        created.append(window)
        return window
      }
    )
    coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    changes.send()
    notificationCenter.post(name: NSApplication.didChangeScreenParametersNotification, object: nil)
    changes.send()
    await waitUntil(timeout: 1) { created.allSatisfy { $0.configureCount == 1 } }
    await assertNotEventually(timeout: 0.08) { created.contains { $0.configureCount > 1 } }
    XCTAssertEqual(created.count, screens.count)
    coordinator.removeOverlays()
  }

  func testRestartIgnoresOldAppearanceStreamAndObservesFreshRecognizer() async throws {
    let screens = overlayTestDisplays
    let oldChanges = OverlayConfigurationChangeEmitter()
    let freshChanges = OverlayConfigurationChangeEmitter()
    var events = oldChanges.events
    let oldRecognizer = TraceObservationCountingGestureRecognizer()
    let freshRecognizer = TraceObservationCountingGestureRecognizer()
    var created: [FakeOverlayWindow] = []
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay }, screensProvider: { screens },
      notificationCenter: NotificationCenter(),
      configurationChangeEvents: { events },
      makeWindow: { _, screen, _ in
        let window = FakeOverlayWindow(screen: screen)
        created.append(window)
        return window
      }
    )
    coordinator.configureOverlays(gestureRecognizer: oldRecognizer)
    oldChanges.send()
    coordinator.removeOverlays()
    events = freshChanges.events
    coordinator.configureOverlays(gestureRecognizer: freshRecognizer)
    oldChanges.send()
    await assertNotEventually(timeout: 0.08) { created.count > screens.count * 2 }
    XCTAssertEqual(oldRecognizer.observerCount, 0)
    XCTAssertEqual(freshRecognizer.observerCount, 1)
    freshChanges.send()
    await waitUntil(timeout: 1) {
      created.suffix(screens.count).allSatisfy { $0.configureCount == 1 }
    }
    XCTAssertEqual(created.count, screens.count * 2)
    coordinator.removeOverlays()
    XCTAssertEqual(freshRecognizer.observerCount, 0)
    XCTAssertTrue(created.allSatisfy { $0.closeCount == 1 })
  }

  func testImmediateStopTerminatesConfigurationProducerBeforeConsumerFirstRuns() async {
    let (events, continuation) = AsyncStream.makeStream(of: Void.self)
    let termination = OverlayStreamTermination()
    continuation.onTermination = { _ in termination.markTerminated() }
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay }, screensProvider: { [] },
      notificationCenter: NotificationCenter(), configurationChangeEvents: { events },
      makeWindow: { _, screen, _ in FakeOverlayWindow(screen: screen) }
    )
    coordinator.configureOverlays(gestureRecognizer: GestureTestRecognizer())
    coordinator.removeOverlays()
    await waitUntil(timeout: 1) { termination.isTerminated }
    XCTAssertTrue(termination.isTerminated)
    _ = continuation
  }

  func testRemoveIsSafeWhenNoOverlaysConfigured() {
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { [] },
      makeWindow: { _, _, _ in FakeOverlayWindow(screen: overlayTestDisplays.first) }
    )
    coordinator.removeOverlays()
  }

  func testConfigureWithNoScreensDoesNotCreateWindowsOrObserveRecognizerTrace() {
    let recognizer = TraceObservationCountingGestureRecognizer()
    var createdCount = 0
    let coordinator = GestureOverlayCoordinator(
      configurationProvider: { _ in .liveOverlay },
      screensProvider: { [] },
      makeWindow: { _, _, _ in
        createdCount += 1
        return FakeOverlayWindow(screen: nil)
      }
    )

    coordinator.configureOverlays(gestureRecognizer: recognizer)

    XCTAssertEqual(createdCount, 0)
    XCTAssertEqual(recognizer.observerCount, 0)
  }

  func testTraceFanoutSkipsDuplicateFilteredTracePerScreen() {
    let screenFrame = CGRect(x: 0, y: 0, width: 100, height: 100)
    let recognizer = GestureTestRecognizer()
    let traceFanout = GestureOverlayTraceFanout(
      gestureRecognizer: recognizer,
      screenFrames: [screenFrame]
    )
    let traceSource = traceFanout.source(
      visibleIn: screenFrame,
      lineWidth: 4
    )
    var observedTraces: [GestureOverlayTrace] = []
    let observation = traceSource.observe {
      _ = $1
      if let trace = $0 { observedTraces.append(trace) }
    }

    XCTAssertNil(traceSource.initialTrace)
    XCTAssertTrue(observedTraces.isEmpty)

    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = [
      GesturePoint(x: 200, y: 10),
      GesturePoint(x: 240, y: 10),
    ]
    recognizer.notifyObservers()

    recognizer.normalizedDirectionPoints = [
      GesturePoint(x: 260, y: 10),
      GesturePoint(x: 300, y: 10),
    ]
    recognizer.notifyObservers()

    XCTAssertTrue(observedTraces.isEmpty)

    recognizer.normalizedDirectionPoints = [
      GesturePoint(x: 96, y: 10),
      GesturePoint(x: 140, y: 10),
    ]
    recognizer.notifyObservers()

    XCTAssertEqual(observedTraces.count, 1)
    XCTAssertEqual(
      observedTraces.last?.points,
      [
        CGPoint(x: 96, y: 10),
        CGPoint(x: 140, y: 10),
      ]
    )

    _ = observation
  }

  func testTraceFanoutDeliversLatestTraceWhenChangedBeforeObservation() {
    let screenFrame = CGRect(x: 0, y: 0, width: 100, height: 100)
    let recognizer = GestureTestRecognizer()
    let traceFanout = GestureOverlayTraceFanout(
      gestureRecognizer: recognizer,
      screenFrames: [screenFrame]
    )
    let traceSource = traceFanout.source(
      visibleIn: screenFrame,
      lineWidth: 4
    )

    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = [
      GesturePoint(x: 0, y: 10),
      GesturePoint(x: 40, y: 10),
    ]
    recognizer.notifyObservers()

    var observedTraces: [GestureOverlayTrace] = []
    let observation = traceSource.observe {
      _ = $1
      if let trace = $0 { observedTraces.append(trace) }
    }

    XCTAssertNil(traceSource.initialTrace)
    XCTAssertEqual(observedTraces.count, 1)
    XCTAssertEqual(
      observedTraces.first?.points,
      [
        CGPoint(x: 0, y: 10),
        CGPoint(x: 40, y: 10),
      ]
    )

    _ = observation
  }

  func testTraceFanoutDeliversHiddenWhenVisibleTraceLeavesScreen() {
    let screenFrame = CGRect(x: 0, y: 0, width: 100, height: 100)
    let recognizer = GestureTestRecognizer()
    let traceFanout = GestureOverlayTraceFanout(
      gestureRecognizer: recognizer,
      screenFrames: [screenFrame]
    )
    let traceSource = traceFanout.source(
      visibleIn: screenFrame,
      lineWidth: 4
    )
    var observedTraces: [GestureOverlayTrace?] = []
    let observation = traceSource.observe {
      _ = $1
      observedTraces.append($0)
    }

    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = [
      GesturePoint(x: 10, y: 10),
      GesturePoint(x: 40, y: 10),
    ]
    recognizer.notifyObservers()

    recognizer.normalizedDirectionPoints = [
      GesturePoint(x: 200, y: 10),
      GesturePoint(x: 240, y: 10),
    ]
    recognizer.notifyObservers()

    XCTAssertEqual(observedTraces.count, 2)
    XCTAssertNotNil(observedTraces[0])
    XCTAssertNil(observedTraces[1])

    _ = observation
  }

  func testTraceFanoutDeliversFastTailUpdatesForSameDirectionGesture() {
    let screenFrame = CGRect(x: 0, y: 0, width: 100, height: 100)
    let recognizer = GestureTestRecognizer()
    let traceFanout = GestureOverlayTraceFanout(
      gestureRecognizer: recognizer,
      screenFrames: [screenFrame]
    )
    let traceSource = traceFanout.source(
      visibleIn: screenFrame,
      lineWidth: 4
    )
    var observedTraces: [GestureOverlayTrace] = []
    let observation = traceSource.observe {
      _ = $1
      if let trace = $0 { observedTraces.append(trace) }
    }

    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = [
      GesturePoint(x: 10, y: 10),
      GesturePoint(x: 40, y: 10),
    ]

    for x in [40.0, 55.0, 70.0, 90.0] {
      recognizer.tailPoint = GesturePoint(x: x, y: 10)
      recognizer.notifyObservers()
    }

    XCTAssertEqual(
      observedTraces.map(\.tailPoint),
      [
        CGPoint(x: 40, y: 10),
        CGPoint(x: 55, y: 10),
        CGPoint(x: 70, y: 10),
        CGPoint(x: 90, y: 10),
      ]
    )
    XCTAssertEqual(observedTraces.map(\.directions), Array(repeating: [.right], count: 4))

    _ = observation
  }

  func testTraceFanoutAllowsSubscriberCancellationDuringPublish() {
    let screenFrame = CGRect(x: 0, y: 0, width: 100, height: 100)
    let recognizer = GestureTestRecognizer()
    let traceFanout = GestureOverlayTraceFanout(
      gestureRecognizer: recognizer,
      screenFrames: [screenFrame]
    )
    let traceSource = traceFanout.source(
      visibleIn: screenFrame,
      lineWidth: 4
    )
    var observation: GestureObservationToken?
    var observedTraces: [GestureOverlayTrace] = []
    observation = traceSource.observe {
      _ = $1
      if let trace = $0 { observedTraces.append(trace) }
      observation?.cancel()
      observation = nil
    }

    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = [
      GesturePoint(x: 0, y: 10),
      GesturePoint(x: 40, y: 10),
    ]
    recognizer.notifyObservers()

    recognizer.normalizedDirectionPoints = [
      GesturePoint(x: 0, y: 20),
      GesturePoint(x: 40, y: 20),
    ]
    recognizer.notifyObservers()

    XCTAssertEqual(observedTraces.count, 1)
    XCTAssertEqual(
      observedTraces.first?.points,
      [
        CGPoint(x: 0, y: 10),
        CGPoint(x: 40, y: 10),
      ]
    )
  }

  func testTraceFanoutContinuesDeliveringWhenSubscriberCancelsDuringPublish() {
    let screenFrame = CGRect(x: 0, y: 0, width: 100, height: 100)
    let recognizer = GestureTestRecognizer()
    let traceFanout = GestureOverlayTraceFanout(
      gestureRecognizer: recognizer,
      screenFrames: [screenFrame]
    )
    let traceSource = traceFanout.source(
      visibleIn: screenFrame,
      lineWidth: 4
    )
    var cancellingObservation: GestureObservationToken?
    var cancellingTraces: [GestureOverlayTrace] = []
    var stableTraces: [GestureOverlayTrace] = []
    cancellingObservation = traceSource.observe {
      _ = $1
      if let trace = $0 { cancellingTraces.append(trace) }
      cancellingObservation?.cancel()
      cancellingObservation = nil
    }
    let stableObservation = traceSource.observe {
      _ = $1
      if let trace = $0 { stableTraces.append(trace) }
    }

    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = [
      GesturePoint(x: 0, y: 10),
      GesturePoint(x: 40, y: 10),
    ]
    recognizer.notifyObservers()

    XCTAssertEqual(cancellingTraces.count, 1)
    XCTAssertEqual(stableTraces.count, 1)
    XCTAssertEqual(stableTraces.first?.directions, [.right])

    _ = stableObservation
  }

  func testTraceFanoutDoesNotRestoreCancelledTraceForLaterScreens() {
    let screenFrame = CGRect(x: 0, y: 0, width: 100, height: 100)
    let recognizer = GestureTestRecognizer()
    let fanout = GestureOverlayTraceFanout(
      gestureRecognizer: recognizer, screenFrames: [screenFrame])
    let source = fanout.source(visibleIn: screenFrame, lineWidth: 4)
    let cancelling = source.observe { trace, _ in
      if trace != nil { recognizer.cancelActiveGesture() }
    }
    var laterTraces: [GestureOverlayTrace?] = []
    let later = source.observe {
      _ = $1
      laterTraces.append($0)
    }

    recognizer.isTraceVisible = true
    recognizer.gestureDirections = [.right]
    recognizer.normalizedDirectionPoints = [.zero, GesturePoint(x: 40, y: 0)]
    recognizer.notifyObservers()

    XCTAssertFalse(recognizer.snapshot.trace.isVisible)
    XCTAssertTrue(laterTraces.isEmpty, "Cancelled geometry must not reappear on a later screen")
    withExtendedLifetime((cancelling, later)) {}
  }

  func testLiveRecognizerCancellationLeavesEveryOverlayHiddenAndRecoversInput() {
    let events = FixedResultGestureEventSource(startResult: true)
    var recovery: [GestureReplayRequest] = []
    let recognizer = GestureRecognizer<Int>(
      eventSource: events,
      configuration: GestureRecognizerConfiguration(
        makeMatcher: { _ in GesturePatternMatcher<Int>() },
        onReplayRequested: { recovery.append($0) },
        onMatch: { _ in XCTFail("Cancelled input matched") }))
    recognizer.isRecordingModeEnabled = true
    recognizer.start()
    defer { recognizer.stop() }
    let frame = CGRect(x: 0, y: 0, width: 200, height: 200)
    let fanout = GestureOverlayTraceFanout(gestureRecognizer: recognizer, screenFrames: [frame])
    let source = fanout.source(visibleIn: frame, lineWidth: 4)
    let cancelling = source.observe { trace, _ in
      if trace != nil { recognizer.cancelActiveGesture() }
    }
    var laterVisible = false
    let later = source.observe {
      _ = $1
      laterVisible = $0 != nil
    }

    XCTAssertEqual(events.send(.init(kind: .buttonDown(.secondary), location: .zero)), .consume)
    XCTAssertEqual(
      events.send(.init(kind: .buttonMoved(.secondary), location: .init(x: 100, y: 0))), .consume)

    XCTAssertFalse(laterVisible)
    XCTAssertFalse(recognizer.snapshot.trace.isVisible)
    XCTAssertEqual(recovery, [.release(button: .secondary, at: .init(x: 100, y: 0))])
    withExtendedLifetime((cancelling, later)) {}
  }

}

@MainActor
private final class FakeOverlayWindow: GestureOverlayWindowResource {
  let screen: GestureOverlayDisplay?
  private(set) var closeCount = 0
  private(set) var configureCount = 0

  init(screen: GestureOverlayDisplay?) {
    self.screen = screen
  }

  func configure(
    traceSource _: GestureOverlayWindowTraceSource, configuration _: GestureOverlayConfiguration
  ) {
    configureCount += 1
  }

  func close() {
    closeCount += 1
  }
}

@MainActor
private final class OverlayConfigurationChangeEmitter {
  let events: AsyncStream<Void>
  private let continuation: AsyncStream<Void>.Continuation

  init() {
    var capturedContinuation: AsyncStream<Void>.Continuation?
    events = AsyncStream { continuation in
      capturedContinuation = continuation
    }
    continuation = capturedContinuation!
  }

  func send() {
    continuation.yield()
  }
}

@MainActor
private final class ObservingFakeOverlayWindow: GestureOverlayWindowResource {
  let screen: GestureOverlayDisplay
  private(set) var closeCount = 0
  private(set) var observedTraceCount = 0
  private var observation: GestureObservationToken?

  init(traceSource: GestureOverlayWindowTraceSource, screen: GestureOverlayDisplay) {
    self.screen = screen
    observation = traceSource.observe { [weak self] _, _ in
      self?.observedTraceCount += 1
    }
  }

  func configure(
    traceSource: GestureOverlayWindowTraceSource, configuration _: GestureOverlayConfiguration
  ) {
    observation?.cancel()
    observation = traceSource.observe { [weak self] _, _ in self?.observedTraceCount += 1 }
  }

  func close() {
    closeCount += 1
    observation?.cancel()
    observation = nil
  }
}

@MainActor
private final class TraceObservationCountingGestureRecognizer: GestureRecognizerObserving {
  private var observers: [UUID: @MainActor @Sendable (GestureRecognizerState.Trace) -> Void] = [:]

  var observerCount: Int {
    observers.count
  }

  var snapshot: GestureRecognizerState {
    GestureRecognizerState(
      status: .init(),
      trace: .init()
    )
  }

  func observe(
    _: @escaping @MainActor @Sendable (GestureRecognizerState) -> Void
  ) -> GestureObservationToken {
    GestureObservationToken {}
  }

  func observeTrace(
    _ handler: @escaping @MainActor @Sendable (GestureRecognizerState.Trace) -> Void
  ) -> GestureObservationToken {
    let id = UUID()
    observers[id] = handler
    handler(snapshot.trace)
    return GestureObservationToken { [weak self] in
      self?.observers.removeValue(forKey: id)
    }
  }
}

private final class OverlayStreamTermination: @unchecked Sendable {
  private let lock = NSLock()
  private var terminated = false
  var isTerminated: Bool { lock.withLock { terminated } }
  func markTerminated() { lock.withLock { terminated = true } }
}
