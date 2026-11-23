import SwiftUI

/// SwiftUI view that renders the currently observed pointer gesture trace.
public struct GestureOverlayView: View {
  private let model: GestureOverlayModel
  private let coordinateMapping: GestureOverlayCoordinateMapping

  /// Creates a live gesture overlay.
  ///
  /// The host owns `model` and updates it as recognition state changes.
  public init(
    model: GestureOverlayModel,
    coordinateMapping: GestureOverlayCoordinateMapping
  ) {
    self.model = model
    self.coordinateMapping = coordinateMapping
  }

  /// The rendered overlay content.
  public var body: some View {
    Canvas { context, canvasSize in
      GestureOverlayRenderer(configuration: model.configuration).drawTrace(
        in: context,
        canvasSize: canvasSize,
        coordinateMapping: coordinateMapping,
        trace: model.trace
      )
    }
    .ignoresSafeArea()
  }
}
