import PointerGestureRendering

func requireSendable<Value: Sendable>(_: Value.Type) {}
requireSendable(GestureOverlayTrace.self)
requireSendable(GestureGeometryError.self)
func requireEquatable<Value: Equatable>(_: Value.Type) {}
requireEquatable(GestureOverlayTrace.self)
requireEquatable(GesturePatternGeometry.self)
precondition(GesturePath.planPattern([], canvasSize: .zero) == .failure(.empty))
let path = try GesturePath.planLiveTrace(
  directions: [.right], points: [.zero, .init(x: 40, y: 0)]
).get()
precondition(path.commands.last == .line(to: .init(x: 40, y: 0)))
let converter = GestureOverlayTraceConverter()
let trace = converter.convert(
  .init(
    isVisible: true, directions: [.right],
    directionEndpoints: [.zero, .init(x: 40, y: 0)]))
precondition(converter.convert(.init()) == nil)
precondition(trace?.points.count == 2)
let first = GestureOverlayState(nowNanoseconds: 0)
let second = GestureOverlayState(nowNanoseconds: 0)
precondition(
  first.update(trace, nowNanoseconds: 0)
    == second.update(trace, nowNanoseconds: 0))
print("PointerGestureRendering imported")
