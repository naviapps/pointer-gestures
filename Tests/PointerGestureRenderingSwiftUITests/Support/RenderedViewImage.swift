import CoreGraphics
import SwiftUI

@testable import PointerGestureRendering

@MainActor
func renderedImage(_ view: some View) -> CGImage? {
  let renderer = ImageRenderer(content: view)
  renderer.scale = 1
  renderer.proposedSize = .unspecified
  return renderer.cgImage
}
