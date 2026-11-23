import CPointerGestureRendering
import CoreGraphics

package enum RenderingInputNormalizer {
  package static func finiteOrZero(_ value: CGFloat) -> CGFloat {
    precondition(
      pointer_gesture_rendering_abi_version() == POINTER_GESTURE_RENDERING_ABI_VERSION,
      "Unsupported native rendering ABI")
    return pointer_gesture_rendering_finite_or_zero(value)
  }
  package static func nonNegativeFiniteOrZero(_ value: CGFloat) -> CGFloat {
    precondition(
      pointer_gesture_rendering_abi_version() == POINTER_GESTURE_RENDERING_ABI_VERSION,
      "Unsupported native rendering ABI")
    return pointer_gesture_rendering_non_negative(value)
  }
}
