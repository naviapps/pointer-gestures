import CoreGraphics
import PointerGestureRecognition
import SwiftUI
import XCTest

@testable import PointerGestureRendering
@testable import PointerGestureRenderingSwiftUI

@MainActor
final class GesturePatternRendererTests: XCTestCase {
  private let renderer = GesturePatternRenderer(configuration: .patternPreview)

  func testPreviewTranslatesCornerScalesAndStrokeJoins() throws {
    func plan(_ configuration: GesturePatternConfiguration) throws
      -> GesturePatternRenderer.RenderPlan
    {
      try XCTUnwrap(
        GesturePatternRenderer(configuration: configuration)
          .makeRenderPlan(canvasSize: CGSize(width: 96, height: 64), pattern: [.right, .down]))
    }
    let sharp = try plan(.init(turnStyle: .rightAngle))
    let rounded = try plan(.patternPreview)
    let subtle = try plan(.init(turnStyle: .init(cornerRadiusScale: 0.5)))
    XCTAssertEqual(pathElementKinds(in: sharp.path), [.move, .line, .line])
    XCTAssertEqual(sharp.strokeStyle.lineJoin, .miter)
    XCTAssertNotNil(sharp.arrowheadPath)
    XCTAssertTrue(pathElementKinds(in: rounded.path).contains(.curve))
    XCTAssertTrue(pathElementKinds(in: subtle.path).contains(.curve))
    XCTAssertNotEqual(subtle.path, rounded.path)
    XCTAssertNotNil(rounded.arrowheadPath)
    XCTAssertGreaterThan(rounded.strokeStyle.lineWidth, 0)
    XCTAssertEqual(rounded.strokeStyle.lineCap, .round)
    XCTAssertEqual(rounded.strokeStyle.lineJoin, .round)
  }

  func testMakeRenderPlanReturnsNilForEmptyPattern() {
    let plan = renderer.makeRenderPlan(
      canvasSize: CGSize(width: 96, height: 64),
      pattern: []
    )

    XCTAssertNil(plan)
  }

  func testMakeRenderPlanUsesCurvedOrthogonalPreviewPath() throws {
    let renderer = GesturePatternRenderer(
      configuration: .init(showsArrowhead: false, padding: 6)
    )

    let plan = try XCTUnwrap(
      renderer.makeRenderPlan(
        canvasSize: CGSize(width: 96, height: 64),
        pattern: [.right, .down]
      ))

    XCTAssertTrue(pathElementKinds(in: plan.path).contains(.curve))
    XCTAssertGreaterThan(plan.path.boundingRect.width, 0)
    XCTAssertGreaterThan(plan.path.boundingRect.height, 0)
  }

  func testMakeRenderPlanBuildsClosedArrowhead() throws {
    let plan = try XCTUnwrap(
      renderer.makeRenderPlan(
        canvasSize: CGSize(width: 96, height: 64),
        pattern: [.right]
      ))
    let arrowheadPath = try XCTUnwrap(plan.arrowheadPath)

    XCTAssertEqual(pathElementKinds(in: arrowheadPath), [.move, .line, .line, .closeSubpath])
  }

  func testRoundedStrokeStaysInsideThePreviewCanvas() throws {
    for width: CGFloat in [3, 12] {
      let renderer = GesturePatternRenderer(
        configuration: .init(strokeStyle: .init(lineWidth: width), padding: 0))
      for pattern: [GestureDirection] in [
        [.right], [.down], [.right, .down], [.right, .down, .left, .up, .right],
      ] {
        let plan = try XCTUnwrap(
          renderer.makeRenderPlan(canvasSize: CGSize(width: 96, height: 64), pattern: pattern))
        let painted = plan.path.cgPath.copy(
          strokingWithWidth: width, lineCap: .round, lineJoin: .round, miterLimit: 10
        ).boundingBoxOfPath
        XCTAssertGreaterThanOrEqual(painted.minX, -0.0001)
        XCTAssertGreaterThanOrEqual(painted.minY, -0.0001)
        XCTAssertLessThanOrEqual(painted.maxX, 96.0001)
        XCTAssertLessThanOrEqual(painted.maxY, 64.0001)
      }
    }
  }

  func testMakeRenderPlanOmitsArrowheadWhenDisabled() throws {
    let renderer = GesturePatternRenderer(
      configuration: .init(showsArrowhead: false)
    )

    let plan = try XCTUnwrap(
      renderer.makeRenderPlan(
        canvasSize: CGSize(width: 96, height: 64),
        pattern: [.right]
      ))

    XCTAssertNil(plan.arrowheadPath)
  }

  func testMakeRenderPlanKeepsPathWhenArrowheadCannotBeBuilt() throws {
    let renderer = GesturePatternRenderer(
      configuration: .init(padding: 100)
    )

    let plan = try XCTUnwrap(
      renderer.makeRenderPlan(
        canvasSize: CGSize(width: 20, height: 20),
        pattern: [.right]
      ))

    XCTAssertNil(plan.arrowheadPath)
    XCTAssertEqual(pathElementKinds(in: plan.path), [.move, .line])
  }

  func testMakeRenderPlanUsesConfiguredStrokeStyle() throws {
    let renderer = GesturePatternRenderer(
      configuration: .init(
        strokeStyle: GestureStrokeStyle(color: .red, lineWidth: 7),
        showsArrowhead: false
      )
    )

    let plan = try XCTUnwrap(
      renderer.makeRenderPlan(
        canvasSize: CGSize(width: 96, height: 64),
        pattern: [.right]
      ))

    XCTAssertEqual(plan.color, .red)
    XCTAssertEqual(plan.strokeStyle.lineWidth, 7, accuracy: 0.0001)
  }

  func testMakeRenderPlanUsesConfiguredPadding() throws {
    let renderer = GesturePatternRenderer(
      configuration: .init(showsArrowhead: false, padding: 20)
    )

    let plan = try XCTUnwrap(
      renderer.makeRenderPlan(
        canvasSize: CGSize(width: 96, height: 64),
        pattern: [.right]
      ))

    XCTAssertGreaterThanOrEqual(plan.path.boundingRect.minX, 20)
    XCTAssertLessThanOrEqual(plan.path.boundingRect.maxX, 76)
    XCTAssertEqual(plan.path.boundingRect.midY, 32, accuracy: 0.0001)
  }
}
