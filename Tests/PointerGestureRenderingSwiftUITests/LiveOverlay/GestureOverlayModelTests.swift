import CoreGraphics
import Observation
import XCTest

@testable import PointerGestureRendering
@testable import PointerGestureRenderingSwiftUI

@MainActor
final class GestureOverlayModelTests: XCTestCase {
  private let updateInterval: TimeInterval = 0.08

  func testInitializerOwnsInitialTraceAndConfiguration() {
    let trace = makeGestureOverlayTrace()
    let configuration = GestureOverlayConfiguration(
      minimumAnimationInterval: updateInterval,
      maximumPointLagDistance: 30,
      strokeStyle: GestureStrokeStyle(lineWidth: 7),
      turnStyle: .rightAngle
    )

    let model = GestureOverlayModel(trace: trace, configuration: configuration)

    XCTAssertEqual(model.trace, trace)
    XCTAssertEqual(model.configuration, configuration)
  }

  func testReconfigurationPreservesPendingFinalTraceAndDeliversHide() async {
    let first = makeGestureOverlayTrace()
    let final = makeGestureOverlayTrace(points: [.zero, CGPoint(x: 21, y: 0)])
    let model = makeModel(trace: first)
    model.update(final)
    model.update(nil)
    let configuration = GestureOverlayConfiguration(
      minimumAnimationInterval: 0.02, strokeStyle: .init(lineWidth: 12), turnStyle: .rightAngle)
    model.configure(configuration)
    XCTAssertEqual(model.configuration, configuration)
    XCTAssertEqual(model.trace, final)
    await assertEventually { model.trace == nil }
  }

  func testDefaultInitializerStartsHidden() {
    XCTAssertNil(GestureOverlayModel().trace)
  }

  func testFirstVisibleTracePublishesImmediately() {
    let model = makeModel()
    let trace = makeGestureOverlayTrace()

    model.update(trace)

    XCTAssertEqual(model.trace, trace)
  }

  func testGeometryUpdatesCoalesceToLatestTrace() async {
    let first = makeGestureOverlayTrace()
    let second = makeGestureOverlayTrace(points: [.zero, CGPoint(x: 21, y: 0)])
    let latest = makeGestureOverlayTrace(points: [.zero, CGPoint(x: 22, y: 0)])
    let model = makeModel(trace: first)

    model.update(second)
    model.update(latest)

    XCTAssertEqual(model.trace, first)
    await assertEventually { model.trace == latest }
  }

  func testReturningToVisibleGeometryCancelsOlderQueuedMovement() async throws {
    let first = makeGestureOverlayTrace()
    let moved = makeGestureOverlayTrace(points: [.zero, CGPoint(x: 21, y: 0)])
    let model = makeModel(trace: first)

    model.update(moved)
    model.update(makeGestureOverlayTrace())
    try await Task.sleep(nanoseconds: 250_000_000)

    XCTAssertEqual(model.trace, first)
  }

  func testDirectionChangeBypassesCadence() {
    let first = makeGestureOverlayTrace()
    let turned = makeGestureOverlayTrace(
      directions: [.right, .down],
      points: [.zero, CGPoint(x: 20, y: 0), CGPoint(x: 20, y: 20)]
    )
    let model = makeModel(trace: first)

    model.update(turned)

    XCTAssertEqual(model.trace, turned)
  }

  func testLargePointerMovementBypassesCadence() {
    let first = makeGestureOverlayTrace(tailPoint: CGPoint(x: 20, y: 0))
    let moved = makeGestureOverlayTrace(tailPoint: CGPoint(x: 80, y: 0))
    let model = makeModel(trace: first, maximumPointLagDistance: 24)

    model.update(moved)

    XCTAssertEqual(model.trace, moved)
  }

  func testLargeMovementBypassIsRateLimited() async {
    let first = makeGestureOverlayTrace(tailPoint: CGPoint(x: 20, y: 0))
    let bypassed = makeGestureOverlayTrace(tailPoint: CGPoint(x: 80, y: 0))
    let latest = makeGestureOverlayTrace(tailPoint: CGPoint(x: 140, y: 0))
    let model = makeModel(trace: first, maximumPointLagDistance: 24)

    model.update(bypassed)
    model.update(latest)

    XCTAssertEqual(model.trace, bypassed)
    await assertEventually { model.trace == latest }
  }

  func testHideKeepsNewTraceVisibleForAtLeastOneRenderOpportunity() async {
    let trace = makeGestureOverlayTrace()
    let model = makeModel()

    model.update(trace)
    model.update(nil)

    XCTAssertEqual(model.trace, trace)
    await assertEventually { model.trace == nil }
  }

  func testHideFlushesPendingLatestGeometryBeforeClearing() async {
    let first = makeGestureOverlayTrace()
    let latest = makeGestureOverlayTrace(points: [.zero, CGPoint(x: 21, y: 0)])
    let model = makeModel()

    model.update(first)
    model.update(latest)
    model.update(nil)

    XCTAssertEqual(model.trace, latest)
    await assertEventually { model.trace == nil }
  }

  func testSameTraceRefreshCancelsPendingHide() async throws {
    let trace = makeGestureOverlayTrace()
    let model = makeModel()

    model.update(trace)
    model.update(nil)
    model.update(trace)
    try await Task.sleep(nanoseconds: 250_000_000)

    XCTAssertEqual(model.trace, trace)
  }

  func testExpiredHideDeadlineDoesNotWaitAgainAfterMainActorWasBusy() async {
    let model = GestureOverlayModel(
      trace: makeGestureOverlayTrace(),
      configuration: .init(minimumAnimationInterval: 0.6)
    )
    // Queue the timer but keep the main actor occupied beyond its deadline.
    let holdMainActor = { Thread.sleep(forTimeInterval: 1.3) }
    model.update(nil)
    XCTAssertNotNil(model.trace)
    holdMainActor()
    await assertEventually(timeout: 0.5) { model.trace == nil }
  }

  func testNewGestureDuringPendingHidePublishesImmediately() {
    let first = makeGestureOverlayTrace()
    let next = makeGestureOverlayTrace(directions: [.left])
    let model = makeModel()

    model.update(first)
    model.update(nil)
    model.update(next)

    XCTAssertEqual(model.trace, next)
  }

  func testHideFromTraceObservationKeepsItsScheduledDeadline() async {
    let model = makeModel(trace: makeGestureOverlayTrace())
    withObservationTracking {
      _ = model.trace
    } onChange: {
      MainActor.assumeIsolated { model.update(nil) }
    }
    model.update(makeGestureOverlayTrace(directions: [.down]))
    await assertEventually { model.trace == nil }
  }

  func testReplacementFromTraceObservationCannotBeOverwrittenByOlderPublication() {
    let model = makeModel(trace: makeGestureOverlayTrace())
    let replacement = makeGestureOverlayTrace(directions: [.left])
    withObservationTracking {
      _ = model.trace
    } onChange: {
      MainActor.assumeIsolated { model.update(replacement) }
    }
    model.update(makeGestureOverlayTrace(directions: [.down]))
    XCTAssertEqual(model.trace, replacement)
  }

  private func makeModel(
    trace: GestureOverlayTrace? = nil,
    maximumPointLagDistance: CGFloat = 24
  ) -> GestureOverlayModel {
    GestureOverlayModel(
      trace: trace,
      configuration: GestureOverlayConfiguration(
        minimumAnimationInterval: updateInterval,
        maximumPointLagDistance: maximumPointLagDistance
      )
    )
  }
}
