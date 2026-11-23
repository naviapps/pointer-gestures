import PointerGestureRecognition

func requireSendable<Value: Sendable>(_: Value.Type) {}
requireSendable(GesturePoint.self)
requireSendable(GestureDirection.self)
requireSendable(PointerButton.self)
requireSendable(GestureModifierFlags.self)
requireSendable(GestureEventDisposition.self)
requireSendable(GestureInputEvent.self)
requireSendable(GestureInputEvent.Kind.self)
requireSendable(GestureReplayRequest.self)
requireSendable(GestureRecognitionContext.self)
requireSendable(GestureRecognizerTuning.self)
requireSendable(GestureRecognizerTuning.ValidationError.self)
requireSendable(GestureRecognizerState.self)
requireSendable(GestureRecognizerState.Status.self)
requireSendable(GestureRecognizerState.Trace.self)
requireSendable(GestureSessionReport.self)
requireSendable(GestureRecognizerState.Status.Lifecycle.self)
requireSendable(GestureRecognizerFailure.self)
requireSendable(GesturePatternMatcher<Int>.self)
requireSendable(GestureRecognizerConfiguration<Int>.self)
requireSendable(GestureObservationToken.self)
requireSendable(GestureRecognizer<Int>.self)
func requireHashable<Value: Hashable>(_: Value.Type) {}
requireHashable(GestureDirection.self)
func requireEquatable<Value: Equatable>(_: Value.Type) {}
requireEquatable(GestureEventDisposition.self)
requireEquatable(GestureInputEvent.self)
requireEquatable(GestureInputEvent.Kind.self)
requireEquatable(GestureReplayRequest.self)
requireEquatable(GestureRecognizerTuning.self)
requireEquatable(GestureRecognizerState.self)
requireEquatable(GestureRecognizerState.Status.self)
requireEquatable(GestureRecognizerState.Status.Lifecycle.self)
requireEquatable(GestureRecognizerState.Trace.self)
requireEquatable(GestureRecognizerFailure.self)
requireEquatable(GestureSessionReport.self)

@MainActor
final class Input: GestureEventSource {
  var handler: (@MainActor @Sendable (GestureInputEvent) -> GestureEventDisposition)?

  func start(handler: @escaping @MainActor @Sendable (GestureInputEvent) -> GestureEventDisposition)
    -> Bool
  {
    self.handler = handler
    return true
  }

  func stop(recoveringInput: @MainActor () -> Void) {
    handler = nil
    recoveringInput()
  }
}

let source = Input()
var matcher = GesturePatternMatcher<String>()
precondition(matcher.register(pattern: [.right], match: "show-inspector") == nil)
var recovery: [GestureReplayRequest] = []
var matchedCommand: String?
let recognizer = GestureRecognizer(
  eventSource: source,
  configuration: GestureRecognizerConfiguration(
    makeMatcher: { [matcher] _ in matcher },
    onReplayRequested: { recovery.append($0) },
    onMatch: { matchedCommand = $0 },
    recognitionButton: .primary,
    areModifiersSatisfied: { modifiers, _ in modifiers.contains(.command) }
  )
)
var sawVisibleTrace = false
let observation = recognizer.observeTrace { trace in
  sawVisibleTrace = sawVisibleTrace || trace.isVisible
}
recognizer.start()
let end = GesturePoint(x: 40, y: 0)
precondition(
  source.handler?(.init(kind: .buttonDown(.primary), location: .zero, modifiers: .command))
    == .consume)
_ = source.handler?(.init(kind: .buttonMoved(.primary), location: end))
_ = source.handler?(.init(kind: .buttonUp(.primary), location: end))
precondition(matchedCommand == "show-inspector" && sawVisibleTrace)
precondition(recovery == [.release(button: .primary, at: end)])
observation.cancel()
recognizer.stop()

print("PointerGestureRecognition imported")
