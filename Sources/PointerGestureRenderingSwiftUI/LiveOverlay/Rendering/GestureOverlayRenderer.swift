import PointerGestureRendering
import SwiftUI

@MainActor
struct GestureOverlayRenderer {
  struct RenderPlan {
    let path: Path
    let color: Color
    let strokeStyle: StrokeStyle
  }

  private let configuration: GestureOverlayConfiguration

  init(configuration: GestureOverlayConfiguration) {
    self.configuration = configuration
  }

  func drawTrace(
    in context: GraphicsContext,
    canvasSize: CGSize,
    coordinateMapping: GestureOverlayCoordinateMapping,
    trace: GestureOverlayTrace?
  ) {
    guard
      let plan = makeRenderPlan(
        canvasSize: canvasSize,
        coordinateMapping: coordinateMapping,
        trace: trace
      )
    else {
      return
    }

    context.stroke(plan.path, with: .color(plan.color), style: plan.strokeStyle)
  }
}

extension GestureOverlayRenderer {
  func makeRenderPlan(
    canvasSize: CGSize,
    coordinateMapping: GestureOverlayCoordinateMapping,
    trace: GestureOverlayTrace?
  ) -> RenderPlan? {
    guard let trace else { return nil }

    guard canvasSize.width.isFinite, canvasSize.height.isFinite,
      canvasSize.width >= 0, canvasSize.height >= 0
    else { return nil }
    guard
      trace.intersects(
        viewport: coordinateMapping.traceViewport(canvasSize: canvasSize),
        lineWidth: configuration.strokeStyle.lineWidth),
      let path = GesturePath.liveTrace(
        trace: trace, cornerRadiusScale: configuration.turnStyle.cornerRadiusScale,
        transform: { coordinateMapping.convert($0, canvasSize: canvasSize) })
    else { return nil }
    return RenderPlan(
      path: path.swiftUIPath,
      color: configuration.strokeStyle.color,
      strokeStyle: configuration.strokeStyle.makeStrokeStyle(
        lineJoin: configuration.turnStyle.lineJoin
      )
    )
  }

}
