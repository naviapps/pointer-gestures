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
print("PointerGestureRenderingSwiftUI imported")
