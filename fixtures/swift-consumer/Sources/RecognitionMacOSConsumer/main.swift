import CoreGraphics
import PointerGestureRecognition
import PointerGestureRecognitionMacOS

let point = CGPoint(x: 10, y: 20)
precondition(GesturePoint(point).cgPoint == point)
let tap = GestureEventTap(capturedButtons: [])
let source: any GestureEventSource = tap
precondition(!source.start { _ in .consume })
let prepare: (GestureReplayRequest) -> (@MainActor @Sendable () -> Void) = tap.prepareReplay
_ = prepare(.drag(button: .primary, points: []))
tap.stop()

print("PointerGestureRecognitionMacOS imported")
