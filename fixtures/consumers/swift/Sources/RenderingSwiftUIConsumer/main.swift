import PointerGestureRendering
import PointerGestureRenderingSwiftUI

func requireSendable<Value: Sendable>(_: Value.Type) {}
requireSendable(GestureOverlayCoordinateMapping.self)
requireSendable(GestureOverlayConfiguration.self)
requireSendable(GesturePatternConfiguration.self)
requireSendable(GestureOverlayModel.self)
requireSendable(GestureOverlayView.self)
requireSendable(GesturePatternView.self)
func requireEquatable<Value: Equatable>(_: Value.Type) {}
requireEquatable(GestureOverlayCoordinateMapping.self)
requireEquatable(GestureOverlayConfiguration.self)
requireEquatable(GesturePatternConfiguration.self)

let model = GestureOverlayModel(
  trace: GestureOverlayTrace(
    directions: [.right], points: [.zero, .init(x: 48, y: 0)]),
  configuration: .init(strokeStyle: .init(lineWidth: 4), turnStyle: .rightAngle)
)
_ =
  GestureOverlayView(
    model: model,
    coordinateMapping: .init(windowOrigin: .zero, displayCoordinateMaxY: 120)
  ).body
_ =
  GesturePatternView(
    pattern: [.right, .down], configuration: .init(showsArrowhead: false, padding: 4)
  ).body

print("PointerGestureRenderingSwiftUI imported")
