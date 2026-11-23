import CPointerGestureRendering
import CoreGraphics
import PointerGestureRecognition

/// An immutable trace validated by Rust, with shared native coordinates and cached bounds.
public struct GestureOverlayTrace: Equatable, Sendable {
  private final class Storage: @unchecked Sendable {
    let handle: OpaquePointer
    init(taking handle: OpaquePointer) { self.handle = handle }
    deinit { pointer_gesture_rendering_trace_destroy(handle) }
  }
  private let storage: Storage
  /// Captured cardinal runs in recognition order.
  public let directions: [GestureDirection]
  /// Stroke origin followed by one endpoint per direction, in display coordinate space.
  public let points: [CGPoint]
  /// Latest pointer location when supplied separately from captured endpoints.
  public let tailPoint: CGPoint?

  /// Fails for empty directions, mismatched point counts or non-finite coordinates.
  public init?(directions: [GestureDirection], points: [CGPoint], tailPoint: CGPoint? = nil) {
    Self.requireNativeABI()
    let codes = directions.map(\.coreCode)
    let nativePoints = points.map { PointerGestureRenderingPoint(x: $0.x, y: $0.y) }
    let handle = codes.withUnsafeBufferPointer { codes in
      nativePoints.withUnsafeBufferPointer { points in
        pointer_gesture_rendering_trace_create(
          codes.baseAddress, codes.count, points.baseAddress, points.count,
          PointerGestureRenderingPoint(x: Double(tailPoint?.x ?? 0), y: Double(tailPoint?.y ?? 0)),
          tailPoint == nil ? 0 : 1)
      }
    }
    guard let handle else { return nil }
    storage = Storage(taking: handle)
    self.directions = directions
    self.points = points
    self.tailPoint = tailPoint
  }

  init?(snapshot: GestureRecognizerState.Trace, previous: Self?) {
    Self.requireNativeABI()
    let directions = snapshot.isVisible ? snapshot.directions : []
    let endpoints = snapshot.isVisible ? snapshot.directionEndpoints : []
    let tail = snapshot.tailPoint
    let converted = withExtendedLifetime(previous) {
      withUnsafeTemporaryAllocation(of: UInt32.self, capacity: directions.count) { codes in
        _ = codes.initialize(from: directions.lazy.map(\.coreCode))
        return withUnsafeTemporaryAllocation(
          of: PointerGestureRenderingPoint.self, capacity: endpoints.count
        ) { points in
          _ = points.initialize(
            from: endpoints.lazy.map { PointerGestureRenderingPoint(x: $0.x, y: $0.y) })
          return pointer_gesture_rendering_trace_from_snapshot(
            previous?.nativeHandle, snapshot.isVisible ? 1 : 0, codes.baseAddress, codes.count,
            points.baseAddress, points.count, .init(x: tail?.x ?? 0, y: tail?.y ?? 0),
            tail == nil ? 0 : 1)
        }
      }
    }
    guard let handle = converted.trace else { return nil }
    let reused = converted.reused_geometry == 0 ? nil : previous
    self.init(
      taking: handle,
      directions: reused?.directions ?? directions,
      points: reused?.points ?? endpoints.map { CGPoint(x: $0.x, y: $0.y) },
      tailPoint: tail.map { CGPoint(x: $0.x, y: $0.y) })
  }

  private static func requireNativeABI() {
    precondition(
      pointer_gesture_rendering_abi_version() == POINTER_GESTURE_RENDERING_ABI_VERSION,
      "Unsupported native rendering ABI")
  }

  init(cloning handle: OpaquePointer) {
    storage = Storage(taking: pointer_gesture_rendering_trace_clone(handle)!)
    let view = pointer_gesture_rendering_trace_view(storage.handle)
    directions = UnsafeBufferPointer(start: view.directions, count: view.count).map {
      GestureDirection(coreCode: $0)
    }
    points = UnsafeBufferPointer(start: view.points, count: view.point_count).map {
      CGPoint(x: $0.x, y: $0.y)
    }
    tailPoint = view.tail_present == 0 ? nil : CGPoint(x: view.tail.x, y: view.tail.y)
  }

  /// Returns an immutable tail replacement sharing native geometry and cached bounds.
  /// Non-finite points return `nil`; prior traces remain independently valid.
  public func replacingTailPoint(with tailPoint: CGPoint?) -> Self? {
    guard
      let handle = pointer_gesture_rendering_trace_with_tail(
        storage.handle,
        .init(x: Double(tailPoint?.x ?? 0), y: Double(tailPoint?.y ?? 0)), tailPoint == nil ? 0 : 1)
    else { return nil }
    return Self(taking: handle, directions: directions, points: points, tailPoint: tailPoint)
  }

  private init(
    taking handle: OpaquePointer, directions: [GestureDirection], points: [CGPoint],
    tailPoint: CGPoint?
  ) {
    storage = Storage(taking: handle)
    self.directions = directions
    self.points = points
    self.tailPoint = tailPoint
  }

  var nativeHandle: OpaquePointer { storage.handle }

  // The immutable trace owns this buffer for the synchronous native call only.
  func withDirectionCodes<Result>(_ body: (UnsafeBufferPointer<UInt32>) -> Result) -> Result {
    withExtendedLifetime(storage) {
      let view = pointer_gesture_rendering_trace_view(storage.handle)
      return body(UnsafeBufferPointer(start: view.directions, count: view.count))
    }
  }

  /// Conservative stroke/tail visibility in the trace's coordinate space.
  ///
  /// Bounds include the tail and half the nonnegative line width; edge contact counts.
  /// Non-finite coordinates reject visibility. Invalid widths use zero padding.
  public func intersects(viewport: CGRect, lineWidth: CGFloat = 0) -> Bool {
    pointer_gesture_rendering_trace_intersects(
      storage.handle,
      PointerGestureRenderingBounds(
        min_x: viewport.minX, min_y: viewport.minY, max_x: viewport.maxX, max_y: viewport.maxY),
      lineWidth) != 0
  }

  public static func == (lhs: Self, rhs: Self) -> Bool {
    lhs.storage === rhs.storage
      || (lhs.directions == rhs.directions && lhs.points == rhs.points
        && lhs.tailPoint == rhs.tailPoint)
  }
}
