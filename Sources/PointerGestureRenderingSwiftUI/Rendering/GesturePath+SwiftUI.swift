import PointerGestureRendering
import SwiftUI

extension GesturePath {
  /// Constructs a native SwiftUI path from geometry owned by the portable planner.
  public var swiftUIPath: Path {
    var path = Path()
    for command in commands {
      switch command {
      case .move(let point): path.move(to: point)
      case .line(let point): path.addLine(to: point)
      case .cubic(let first, let second, let point):
        path.addCurve(to: point, control1: first, control2: second)
      case .close: path.closeSubpath()
      }
    }
    return path
  }
}
