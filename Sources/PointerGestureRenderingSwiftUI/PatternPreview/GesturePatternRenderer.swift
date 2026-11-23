import PointerGestureRecognition
import PointerGestureRendering
import SwiftUI

@MainActor
struct GesturePatternRenderer {
  struct RenderPlan {
    let path: Path
    let arrowheadPath: Path?
    let color: Color
    let strokeStyle: StrokeStyle
  }

  private let configuration: GesturePatternConfiguration

  init(configuration: GesturePatternConfiguration) {
    self.configuration = configuration
  }

  func drawPattern(
    in context: GraphicsContext,
    canvasSize: CGSize,
    pattern: [GestureDirection]
  ) {
    guard let plan = makeRenderPlan(canvasSize: canvasSize, pattern: pattern) else {
      return
    }

    context.stroke(plan.path, with: .color(plan.color), style: plan.strokeStyle)
    if let arrowheadPath = plan.arrowheadPath {
      context.fill(arrowheadPath, with: .color(plan.color))
    }
  }
}

extension GesturePatternRenderer {
  func makeRenderPlan(canvasSize: CGSize, pattern: [GestureDirection]) -> RenderPlan? {
    let strokeStyle = configuration.strokeStyle
    guard
      let geometry = GesturePath.pattern(
        pattern, canvasSize: canvasSize,
        padding: configuration.padding, lineWidth: strokeStyle.lineWidth,
        showsArrowhead: configuration.showsArrowhead,
        cornerRadiusScale: configuration.turnStyle.cornerRadiusScale)
    else { return nil }

    return RenderPlan(
      path: geometry.path.swiftUIPath,
      arrowheadPath: geometry.arrowhead?.swiftUIPath,
      color: strokeStyle.color,
      strokeStyle: strokeStyle.makeStrokeStyle(lineJoin: configuration.turnStyle.lineJoin)
    )
  }

}
