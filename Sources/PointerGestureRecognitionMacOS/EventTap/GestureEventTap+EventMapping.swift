import PointerGestureRecognition

import class CoreGraphics.CGEvent
import enum CoreGraphics.CGEventField
import struct CoreGraphics.CGEventFlags
import enum CoreGraphics.CGEventType

extension GestureEventTap {
  nonisolated var eventMask: UInt64 {
    guard !capturedButtons.isEmpty else { return 0 }

    var eventTypes = Set<CGEventType>()
    eventTypes.insert(.keyDown)
    eventTypes.insert(.keyUp)
    for button in capturedButtons {
      let buttonEventTypes = button.cgEventTypes
      eventTypes.insert(buttonEventTypes.down)
      eventTypes.insert(buttonEventTypes.moved)
      eventTypes.insert(buttonEventTypes.up)
    }
    return eventTapMask(for: eventTypes)
  }

  nonisolated func mappedEscapeKeyPhase(for type: CGEventType, event: CGEvent) -> EscapeKeyPhase? {
    guard
      !capturedButtons.isEmpty, type == .keyDown || type == .keyUp,
      event.getIntegerValueField(.keyboardEventKeycode) == escapeVirtualKeyCode,
      !ReplayedEventMarker.isPresent(on: event)
    else { return nil }
    if type == .keyUp { return .up }
    if event.getIntegerValueField(.keyboardEventAutorepeat) != 0 { return .repeatedDown }
    return .down(makeInputEvent(kind: .cancel, event: event))
  }

  nonisolated func mappedInputEvent(for type: CGEventType, event: CGEvent) -> MappedInputEvent? {
    guard !capturedButtons.isEmpty else { return nil }

    guard let mapping = pointerButtonInput(for: type, event: event) else {
      return nil
    }
    guard capturedButtons.contains(mapping.button) else { return nil }
    return makeMappedInputEvent(
      kind: mapping.kind(mapping.button),
      event: event
    )
  }

  nonisolated func mappedTapDisabledInputEvent(
    for type: CGEventType,
    event: CGEvent
  ) -> MappedInputEvent? {
    guard type == .tapDisabledByTimeout || type == .tapDisabledByUserInput else { return nil }
    return makeMappedInputEvent(kind: .cancel, event: event)
  }

  private nonisolated func makeMappedInputEvent(
    kind: GestureInputEvent.Kind,
    event: CGEvent
  ) -> MappedInputEvent {
    MappedInputEvent(
      input: makeInputEvent(kind: kind, event: event),
      isReplayed: ReplayedEventMarker.isPresent(on: event)
    )
  }
}

enum EscapeKeyPhase: Sendable {
  case down(GestureInputEvent)
  case repeatedDown
  case up
}

struct MappedInputEvent: Sendable {
  let input: GestureInputEvent
  let isReplayed: Bool
}

private struct PointerButtonInputMapping {
  let kind: (PointerButton) -> GestureInputEvent.Kind
  let button: PointerButton
}

private let escapeVirtualKeyCode: Int64 = 53

private func makeInputEvent(kind: GestureInputEvent.Kind, event: CGEvent) -> GestureInputEvent {
  GestureInputEvent(
    kind: kind,
    location: GesturePoint(event.location),
    modifiers: gestureModifierFlags(from: event.flags)
  )
}

private func eventTapMask(for types: some Sequence<CGEventType>) -> UInt64 {
  types.reduce(0) { mask, type in
    mask | (1 << type.rawValue)
  }
}

private func pointerButtonInput(
  for type: CGEventType,
  event: CGEvent
) -> PointerButtonInputMapping? {
  switch type {
  case .leftMouseDown:
    return PointerButtonInputMapping(kind: GestureInputEvent.Kind.buttonDown, button: .primary)
  case .leftMouseDragged:
    return PointerButtonInputMapping(kind: GestureInputEvent.Kind.buttonMoved, button: .primary)
  case .leftMouseUp:
    return PointerButtonInputMapping(kind: GestureInputEvent.Kind.buttonUp, button: .primary)
  case .rightMouseDown:
    return PointerButtonInputMapping(kind: GestureInputEvent.Kind.buttonDown, button: .secondary)
  case .rightMouseDragged:
    return PointerButtonInputMapping(kind: GestureInputEvent.Kind.buttonMoved, button: .secondary)
  case .rightMouseUp:
    return PointerButtonInputMapping(kind: GestureInputEvent.Kind.buttonUp, button: .secondary)
  case .otherMouseDown:
    guard let button = otherPointerButton(for: event) else { return nil }
    return PointerButtonInputMapping(kind: GestureInputEvent.Kind.buttonDown, button: button)
  case .otherMouseDragged:
    guard let button = otherPointerButton(for: event) else { return nil }
    return PointerButtonInputMapping(kind: GestureInputEvent.Kind.buttonMoved, button: button)
  case .otherMouseUp:
    guard let button = otherPointerButton(for: event) else { return nil }
    return PointerButtonInputMapping(kind: GestureInputEvent.Kind.buttonUp, button: button)
  default:
    return nil
  }
}

private func otherPointerButton(for event: CGEvent) -> PointerButton? {
  guard let button = pointerButton(for: event) else { return nil }
  guard button == .middle || button.auxiliaryButtonID != nil else { return nil }
  return button
}

private func pointerButton(for event: CGEvent) -> PointerButton? {
  PointerButton(cgButtonNumber: event.getIntegerValueField(.mouseEventButtonNumber))
}

private func gestureModifierFlags(from flags: CGEventFlags) -> GestureModifierFlags {
  var result = GestureModifierFlags()
  if flags.contains(.maskCommand) { result.insert(.command) }
  if flags.contains(.maskAlternate) { result.insert(.option) }
  if flags.contains(.maskControl) { result.insert(.control) }
  if flags.contains(.maskShift) { result.insert(.shift) }
  return result
}
