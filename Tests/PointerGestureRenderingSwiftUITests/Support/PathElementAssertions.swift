import SwiftUI

enum PathElementKind {
  case move
  case line
  case quadCurve
  case curve
  case closeSubpath
}

func pathElementKinds(in path: Path) -> [PathElementKind] {
  var kinds: [PathElementKind] = []
  // Path exposes callback traversal rather than Sequence iteration.
  // swift-format-ignore: ReplaceForEachWithForLoop
  path.forEach { element in
    switch element {
    case .move:
      kinds.append(.move)
    case .line:
      kinds.append(.line)
    case .quadCurve:
      kinds.append(.quadCurve)
    case .curve:
      kinds.append(.curve)
    case .closeSubpath:
      kinds.append(.closeSubpath)
    }
  }
  return kinds
}
