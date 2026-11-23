import PointerGestureRenderingMacOS

let coordinator = GestureOverlayCoordinator(
  configurationProvider: { _ in .liveOverlay }, screensProvider: { [] })
coordinator.removeOverlays()
print("PointerGestureRenderingMacOS imported")
