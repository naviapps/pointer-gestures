import CPointerGestureRendering
import CoreGraphics
import Foundation
import PointerGestureRecognition

/// A portable command stream planned by Rust. Coordinates use the caller's drawing space.
public struct GesturePath: Equatable, Sendable {
  /// A native-renderer-independent drawing operation in the path's coordinate space.
  public enum Command: Equatable, Sendable {
    /// Starts a subpath at the supplied point without drawing a segment.
    case move(to: CGPoint)
    /// Draws a line from the current location to the supplied point.
    case line(to: CGPoint)
    /// Draws a cubic Bézier segment using two control points and an endpoint.
    case cubic(control1: CGPoint, control2: CGPoint, to: CGPoint)
    /// Closes the current subpath to its initial point.
    case close
  }

  /// Copied command values in drawing order, independent of native handle lifetimes.
  public let commands: [Command]

  /// Plans finite live geometry with cardinal runs and an optional tail projected onto them.
  ///
  /// Points must contain the stroke origin and one finite endpoint per direction.
  /// The default corner radius is bounded by 24 coordinate units and half of each
  /// adjacent run. Segment lengths are checked after projection, including with a tail.
  /// Invalid input or unrepresentable computed geometry returns `nil`.
  public static func liveTrace(
    directions: [GestureDirection], points: [CGPoint],
    cornerRadiusScale: CGFloat = 1,
    tailPoint: CGPoint? = nil
  ) -> GesturePath? {
    try? planLiveTrace(
      directions: directions, points: points, cornerRadiusScale: cornerRadiusScale,
      tailPoint: tailPoint
    ).get()
  }

  /// Reports the reason when live path planning rejects the input or its projected geometry.
  /// Inputs and drawing units are the same as for ``liveTrace(directions:points:cornerRadiusScale:tailPoint:)``.
  public static func planLiveTrace(
    directions: [GestureDirection], points: [CGPoint],
    cornerRadiusScale: CGFloat = 1,
    tailPoint: CGPoint? = nil
  ) -> Result<GesturePath, GestureGeometryError> {
    requireNativeABI()
    let codes = directions.map(\.coreCode)
    let points = points.map { PointerGestureRenderingPoint(x: $0.x, y: $0.y) }
    let result = codes.withUnsafeBufferPointer { codes in
      nativeLiveTrace(
        codes: codes, points: points, cornerRadiusScale: cornerRadiusScale,
        tailPoint: tailPoint.map { PointerGestureRenderingPoint(x: $0.x, y: $0.y) })
    }
    return decode(result).map(\.path)
  }

  package static func liveTrace(
    trace: GestureOverlayTrace, cornerRadiusScale: CGFloat,
    transform: (CGPoint) -> CGPoint
  ) -> GesturePath? {
    requireNativeABI()
    func nativePoint(_ point: CGPoint) -> PointerGestureRenderingPoint {
      let mapped = transform(point)
      return PointerGestureRenderingPoint(x: mapped.x, y: mapped.y)
    }
    let points = trace.points.map(nativePoint)
    let tail = trace.tailPoint.map(nativePoint)
    let result = trace.withDirectionCodes { codes in
      nativeLiveTrace(
        codes: codes, points: points, cornerRadiusScale: cornerRadiusScale, tailPoint: tail)
    }
    return try? decode(result).get().path
  }

  private static func requireNativeABI() {
    precondition(
      pointer_gesture_rendering_abi_version() == POINTER_GESTURE_RENDERING_ABI_VERSION,
      "Unsupported native rendering ABI")
  }

  private static func nativeLiveTrace(
    codes: UnsafeBufferPointer<UInt32>, points: [PointerGestureRenderingPoint],
    cornerRadiusScale: CGFloat,
    tailPoint: PointerGestureRenderingPoint?
  ) -> PointerGestureRenderingResult {
    points.withUnsafeBufferPointer { points in
      pointer_gesture_rendering_live_trace(
        codes.baseAddress, codes.count, points.baseAddress, points.count,
        cornerRadiusScale, tailPoint ?? PointerGestureRenderingPoint(x: 0, y: 0),
        tailPoint == nil ? 0 : 1)
    }
  }

  /// Fits preview geometry and its separate arrowhead without constructing a UI path.
  ///
  /// Canvas dimensions, padding and stroke width share one coordinate unit; negative
  /// or non-finite scalar inputs become zero. Empty directions or unrepresentable
  /// geometry return `nil`. Disabling the head preserves the shaft's layout.
  /// Corner scale multiplies one tenth of the smaller canvas dimension; zero
  /// keeps sharp turns, and negative or non-finite scales become zero.
  public static func pattern(
    _ directions: [GestureDirection], canvasSize: CGSize, padding: CGFloat = 8,
    lineWidth: CGFloat = 3,
    showsArrowhead: Bool = true,
    cornerRadiusScale: CGFloat = 1
  ) -> GesturePatternGeometry? {
    try? planPattern(
      directions, canvasSize: canvasSize, padding: padding, lineWidth: lineWidth,
      showsArrowhead: showsArrowhead, cornerRadiusScale: cornerRadiusScale
    ).get()
  }

  /// Reports the reason when preview planning rejects the directions or fitted geometry.
  /// Inputs and drawing units are the same as for ``pattern(_:canvasSize:padding:lineWidth:showsArrowhead:cornerRadiusScale:)``.
  public static func planPattern(
    _ directions: [GestureDirection], canvasSize: CGSize, padding: CGFloat = 8,
    lineWidth: CGFloat = 3,
    showsArrowhead: Bool = true,
    cornerRadiusScale: CGFloat = 1
  ) -> Result<GesturePatternGeometry, GestureGeometryError> {
    requireNativeABI()
    let result = directions.map(\.coreCode).withUnsafeBufferPointer {
      pointer_gesture_rendering_pattern(
        $0.baseAddress, $0.count, canvasSize.width, canvasSize.height,
        padding, lineWidth, showsArrowhead ? 1 : 0, cornerRadiusScale)
    }
    return decode(result)
  }

  static func decode(_ result: PointerGestureRenderingResult)
    -> Result<GesturePatternGeometry, GestureGeometryError>
  {
    guard result.code == 0 else { return .failure(GestureGeometryError(nativeCode: result.code)) }
    guard let geometry = result.geometry else {
      return .failure(.invalidNativeResult)
    }
    defer { pointer_gesture_rendering_geometry_destroy(geometry) }
    let view = pointer_gesture_rendering_geometry_view(geometry)
    return .success(
      GesturePatternGeometry(
        path: decode(view.path, count: view.path_count),
        arrowhead: view.arrowhead_count == 0
          ? nil : decode(view.arrowhead, count: view.arrowhead_count)))
  }

  /// Copy every native value before releasing its owner or invoking any UI code.
  private static func decode(_ data: UnsafePointer<PointerGestureRenderingCommand>?, count: Int)
    -> Self
  {
    Self(
      commands: UnsafeBufferPointer(start: data, count: count).map { command in
        switch command.kind {
        case 1: .move(to: point(command.point))
        case 2: .line(to: point(command.point))
        case 3:
          .cubic(
            control1: point(command.control1), control2: point(command.control2),
            to: point(command.point))
        case 4: .close
        default: preconditionFailure("Unsupported rendering command")
        }
      })
  }

  private static func point(_ value: PointerGestureRenderingPoint) -> CGPoint {
    CGPoint(x: value.x, y: value.y)
  }
}
