import CoreGraphics
import PointerGestureRecognition
import PointerGestureRenderingSwiftUI
import XCTest

@testable import PointerGestureRendering

final class PointerGestureRenderingSwiftUIPublicAPITests: XCTestCase {
  @MainActor
  func testLiveOverlayPublicContractIsUsableWithoutTestableImport() throws {
    let trace = try XCTUnwrap(
      GestureOverlayTrace(
        directions: [.right],
        points: [.zero, CGPoint(x: 48, y: 0)],
        tailPoint: CGPoint(x: 52, y: 6)
      )
    )
    let configuration = GestureOverlayConfiguration(
      minimumAnimationInterval: 0.25,
      maximumPointLagDistance: 32,
      strokeStyle: GestureStrokeStyle(color: .red, lineWidth: 3),
      turnStyle: .rightAngle
    )
    let model = GestureOverlayModel(trace: trace, configuration: configuration)
    let coordinateMapping = makeCoordinateMapping()
    let view = GestureOverlayView(model: model, coordinateMapping: coordinateMapping)

    XCTAssertEqual(trace.directions, [.right])
    XCTAssertEqual(trace.points, [.zero, CGPoint(x: 48, y: 0)])
    XCTAssertEqual(trace.tailPoint, CGPoint(x: 52, y: 6))
    XCTAssertEqual(model.trace, trace)
    XCTAssertEqual(model.configuration, configuration)
    XCTAssertEqual(coordinateMapping, makeCoordinateMapping())
    _ = view.body

    model.update(nil)
  }

  @MainActor
  func testPatternPreviewPublicContractIsUsableWithoutTestableImport() {
    let configuration = GesturePatternConfiguration(
      frameSize: CGSize(width: 48, height: 32),
      strokeStyle: .patternPreview,
      showsArrowhead: false,
      padding: 4
    )
    let pattern: [GestureDirection] = [.right, .down, .left]
    let view = GesturePatternView(pattern: pattern, configuration: configuration)

    XCTAssertEqual(configuration.frameSize, CGSize(width: 48, height: 32))
    XCTAssertEqual(configuration.strokeStyle, .patternPreview)
    XCTAssertFalse(configuration.showsArrowhead)
    XCTAssertEqual(configuration.padding, 4, accuracy: 0.0001)
    _ = view.body
  }

  @MainActor
  private func makeCoordinateMapping() -> GestureOverlayCoordinateMapping {
    GestureOverlayCoordinateMapping(
      windowOrigin: .zero,
      displayCoordinateMaxY: 120
    )
  }
}
