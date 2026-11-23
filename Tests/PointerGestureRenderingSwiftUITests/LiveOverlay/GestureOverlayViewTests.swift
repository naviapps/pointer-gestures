import CoreGraphics
import PointerGestureRenderingSwiftUI
import SwiftUI
import XCTest

@testable import PointerGestureRendering

@MainActor
final class GestureOverlayViewTests: XCTestCase {
  func testBodyRendersVisibleModelTrace() {
    let model = GestureOverlayModel(
      trace: makeGestureOverlayTrace(
        points: [CGPoint(x: 12, y: 24), CGPoint(x: 96, y: 24)]
      )
    )

    let image = renderedImage(makeView(model: model))

    assertRenderedImageContainsVisiblePixel(image)
  }

  func testBodyRendersConfiguredColor() {
    let model = GestureOverlayModel(
      trace: makeGestureOverlayTrace(
        points: [CGPoint(x: 12, y: 24), CGPoint(x: 96, y: 24)]
      ),
      configuration: GestureOverlayConfiguration(
        strokeStyle: GestureStrokeStyle(color: .red, lineWidth: 8)
      )
    )

    let image = renderedImage(makeView(model: model))

    assertRenderedImageContainsRedPixel(image)
  }

  func testBodyDoesNotRenderHiddenModel() {
    let image = renderedImage(makeView(model: GestureOverlayModel()))

    assertRenderedImageContainsNoVisiblePixels(image)
  }

  func testBodyTracksModelUpdates() {
    let model = GestureOverlayModel()
    let view = makeView(model: model)

    model.update(
      makeGestureOverlayTrace(
        points: [CGPoint(x: 12, y: 24), CGPoint(x: 96, y: 24)]
      )
    )

    assertRenderedImageContainsVisiblePixel(renderedImage(view))
  }

  private func makeView(model: GestureOverlayModel) -> some View {
    GestureOverlayView(
      model: model,
      coordinateMapping: GestureOverlayCoordinateMapping(
        windowOrigin: .zero,
        displayCoordinateMaxY: 80
      )
    )
    .frame(width: 120, height: 80)
  }
}
