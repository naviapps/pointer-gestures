import CoreGraphics
import PointerGestureRecognition
import XCTest

@testable import PointerGestureRendering
@testable import PointerGestureRenderingSwiftUI

@MainActor
final class GestureOverlayRendererTests: XCTestCase {
  private let renderer = GestureOverlayRenderer(
    configuration: .liveOverlay
  )

  func testMakeRenderPlanReturnsNilForHiddenTrace() {
    let plan = renderer.makeRenderPlan(
      canvasSize: CGSize(width: 200, height: 120),
      coordinateMapping: makeCoordinateMapping(
        windowOrigin: .zero,
        displayCoordinateMaxY: 120
      ),
      trace: nil
    )

    XCTAssertNil(plan)
  }

  func testMakeRenderPlanReturnsNilWhenLayoutCannotBeBuilt() {
    let plan = makeRenderPlan(
      canvasSize: CGSize(width: -1, height: 120)
    )

    XCTAssertNil(plan)
  }

  func testMakeRenderPlanConvertsTracePointsIntoDrawablePath() throws {
    let plan = try XCTUnwrap(
      makeRenderPlan(
        canvasSize: CGSize(width: 200, height: 120),
        coordinateMapping: makeCoordinateMapping(
          windowOrigin: CGPoint(x: 20, y: 30),
          displayCoordinateMaxY: 180
        ),
        trace: makeTrace(
          directions: [.right, .down],
          points: [
            CGPoint(x: 25, y: 90),
            CGPoint(x: 75, y: 90),
            CGPoint(x: 75, y: 140),
          ]
        )
      ))

    assertEqualRect(
      plan.path.boundingRect,
      CGRect(x: 5, y: 60, width: 50, height: 50),
      accuracy: 0.0001
    )
  }

  func testMakeRenderPlanProjectsTailOntoTheCapturedAxis() throws {
    let plan = try XCTUnwrap(
      makeRenderPlan(
        trace: makeTrace(
          directions: [.right],
          points: [
            CGPoint(x: 10, y: 10),
            CGPoint(x: 40, y: 10),
          ],
          tailPoint: CGPoint(x: 65, y: 40)
        )
      ))

    assertEqualRect(
      plan.path.boundingRect,
      CGRect(x: 10, y: 10, width: 55, height: 0),
      accuracy: 0.0001
    )
  }

  func testRenderPlansOwnMappedPathsAfterTailSnapshotsAreReleased() throws {
    let plans = try autoreleasepool {
      let trace = makeTrace(
        directions: [.right, .down],
        points: [CGPoint(x: 25, y: 90), CGPoint(x: 75, y: 90), CGPoint(x: 75, y: 110)],
        tailPoint: CGPoint(x: 80, y: 120))
      let replacement = try XCTUnwrap(trace.replacingTailPoint(with: CGPoint(x: 90, y: 140)))
      let mapping = makeCoordinateMapping(
        windowOrigin: CGPoint(x: 20, y: 30), displayCoordinateMaxY: 180)
      return (
        try XCTUnwrap(makeRenderPlan(coordinateMapping: mapping, trace: trace)),
        try XCTUnwrap(makeRenderPlan(coordinateMapping: mapping, trace: replacement))
      )
    }

    assertEqualRect(
      plans.0.path.boundingRect, CGRect(x: 5, y: 60, width: 55, height: 30),
      accuracy: 0.0001)
    assertEqualRect(
      plans.1.path.boundingRect, CGRect(x: 5, y: 60, width: 65, height: 50),
      accuracy: 0.0001)
  }

  func testMakeRenderPlanReturnsNilWhenTracePathIsOutsideCanvas() {
    let plan = makeRenderPlan(
      canvasSize: CGSize(width: 200, height: 120),
      coordinateMapping: makeCoordinateMapping(
        windowOrigin: .zero,
        displayCoordinateMaxY: 500
      ),
      trace: makeTrace(
        directions: [.right],
        points: [CGPoint(x: 10, y: 10), CGPoint(x: 40, y: 10)]
      )
    )

    XCTAssertNil(plan)
  }

  func testMakeRenderPlanUsesConfiguredStrokeStyle() throws {
    let renderer = GestureOverlayRenderer(
      configuration: GestureOverlayConfiguration(
        strokeStyle: GestureStrokeStyle(
          color: .red,
          lineWidth: 7.0
        )
      )
    )

    let plan = try XCTUnwrap(
      makeRenderPlan(with: renderer)
    )

    XCTAssertEqual(plan.color, .red)
    XCTAssertEqual(plan.strokeStyle.lineWidth, 7.0, accuracy: 0.0001)
  }

  func testMakeRenderPlanUsesMiterJoinsForRightAngleTurns() throws {
    let renderer = GestureOverlayRenderer(
      configuration: GestureOverlayConfiguration(turnStyle: .rightAngle)
    )

    let plan = try XCTUnwrap(
      makeRenderPlan(
        with: renderer,
        trace: makeTrace(
          directions: [.right, .down],
          points: [
            CGPoint(x: 10, y: 10),
            CGPoint(x: 40, y: 10),
            CGPoint(x: 40, y: 40),
          ]
        )
      ))

    XCTAssertEqual(plan.strokeStyle.lineJoin, .miter)
  }

  private func makeRenderPlan(
    with renderer: GestureOverlayRenderer? = nil,
    canvasSize: CGSize = CGSize(width: 200, height: 120),
    coordinateMapping: GestureOverlayCoordinateMapping = GestureOverlayCoordinateMapping(
      windowOrigin: .zero,
      displayCoordinateMaxY: 120
    ),
    trace: GestureOverlayTrace? = nil
  ) -> GestureOverlayRenderer.RenderPlan? {
    (renderer ?? self.renderer).makeRenderPlan(
      canvasSize: canvasSize,
      coordinateMapping: coordinateMapping,
      trace: trace ?? makeTrace()
    )
  }

  private func makeTrace(
    directions: [GestureDirection] = [.right],
    points: [CGPoint] = [CGPoint(x: 10, y: 10), CGPoint(x: 40, y: 10)],
    tailPoint: CGPoint? = nil
  ) -> GestureOverlayTrace {
    makeGestureOverlayTrace(
      directions: directions,
      points: points,
      tailPoint: tailPoint
    )
  }

  private func makeCoordinateMapping(
    windowOrigin: CGPoint,
    displayCoordinateMaxY: CGFloat
  ) -> GestureOverlayCoordinateMapping {
    GestureOverlayCoordinateMapping(
      windowOrigin: windowOrigin,
      displayCoordinateMaxY: displayCoordinateMaxY
    )
  }
}
