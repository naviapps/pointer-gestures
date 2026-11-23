import CoreGraphics
import PointerGestureRecognition
import PointerGestureRendering
import PointerGestureRenderingSwiftUI

func makeGestureOverlayTrace(
  directions: [GestureDirection] = [.right],
  points: [CGPoint] = [.zero, CGPoint(x: 20, y: 0)],
  tailPoint: CGPoint? = nil
) -> GestureOverlayTrace {
  guard
    let trace = GestureOverlayTrace(
      directions: directions,
      points: points,
      tailPoint: tailPoint
    )
  else {
    preconditionFailure("Test fixture must describe a valid visible trace.")
  }
  return trace
}
