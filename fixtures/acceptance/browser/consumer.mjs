import init, { GestureSession, tracePath } from "@naviapps/pointer-gestures";

const canvas = document.querySelector("#gesture");
const status = document.querySelector("#status");
const dispose = document.querySelector("#dispose");
const context = canvas.getContext("2d", { willReadFrequently: true });
const listeners = new AbortController();
const state = {
  ready: false,
  disposed: false,
  eventCount: 0,
  trustedInput: true,
  gotCapture: 0,
  lostCapture: 0,
  capturedOnDown: false,
  releasedOutside: false,
  activePointer: null,
  lastPointer: null,
  matches: 0,
  update: null,
  commandCount: 0,
  traceFrozen: false,
  disposalRejected: false,
  retainedTraceAfterDisposal: false,
};
let finalTrace = null;
let finalCommands = null;
let session;

function pixels() {
  const bytes = context.getImageData(0, 0, canvas.width, canvas.height).data;
  let count = 0;
  for (let i = 3; i < bytes.length; i += 4) if (bytes[i] > 0) count++;
  return count;
}

Object.defineProperty(window, "pointerConsumer", {
  value: Object.freeze({
    snapshot: () => JSON.parse(JSON.stringify({
      ...state,
      status: status.textContent,
      paintedPixels: pixels(),
      hasCapture: state.lastPointer !== null && canvas.hasPointerCapture(state.lastPointer),
    }, (_, value) => typeof value === "bigint" ? String(value) : value)),
  }),
});

function draw(trace) {
  const commands = tracePath(trace);
  context.clearRect(0, 0, canvas.width, canvas.height);
  context.beginPath();
  for (const command of commands) {
    if (command.type === "move") context.moveTo(command.to.x, command.to.y);
    else if (command.type === "line") context.lineTo(command.to.x, command.to.y);
    else if (command.type === "cubic") {
      context.bezierCurveTo(
        command.control1.x, command.control1.y,
        command.control2.x, command.control2.y, command.to.x, command.to.y,
      );
    } else if (command.type === "close") context.closePath();
  }
  context.strokeStyle = "#0066cc";
  context.lineWidth = 7;
  context.lineCap = "round";
  context.lineJoin = "round";
  context.stroke();
  state.commandCount = commands.length;
  return commands;
}

function apply(update) {
  state.update = update;
  const commands = draw(update.trace);
  if (update.matchId !== null) {
    state.matches++;
    finalTrace = update.trace;
    finalCommands = commands;
    state.traceFrozen = Object.isFrozen(finalTrace) && Object.isFrozen(finalTrace.points) &&
      finalTrace.points.every(Object.isFrozen) && Object.isFrozen(finalTrace.directions);
    status.textContent = `Matched ${update.matchId} · ${update.trace.directions.join(" → ")}`;
  } else {
    status.textContent = update.active ? "Capturing gesture" : "Ready";
  }
}

function point(event) {
  const box = canvas.getBoundingClientRect();
  return {
    x: (event.clientX - box.left) * canvas.width / box.width,
    y: (event.clientY - box.top) * canvas.height / box.height,
  };
}

function admitted(event) {
  state.eventCount++;
  state.trustedInput &&= event.isTrusted && event.pointerType === "mouse";
  event.preventDefault();
}

await init(); // Fetches the colocated Wasm from the installed package over HTTP.
session = new GestureSession({ totalDurationMs: 0, idleDurationMs: 0 });
session.register(["down", "right"], 42);
canvas.addEventListener("pointerdown", (event) => {
  if (event.button !== 0 || state.activePointer !== null) return;
  admitted(event);
  state.activePointer = state.lastPointer = event.pointerId;
  canvas.setPointerCapture(event.pointerId);
  state.capturedOnDown = canvas.hasPointerCapture(event.pointerId);
  apply(session.begin(point(event), performance.now()));
}, { signal: listeners.signal });
canvas.addEventListener("pointermove", (event) => {
  if (event.pointerId !== state.activePointer) return;
  admitted(event);
  apply(session.move(point(event), performance.now()));
}, { signal: listeners.signal });
canvas.addEventListener("pointerup", (event) => {
  if (event.pointerId !== state.activePointer) return;
  admitted(event);
  const box = canvas.getBoundingClientRect();
  state.releasedOutside = event.clientX > box.right;
  apply(session.end(point(event), performance.now()));
  state.activePointer = null;
  if (canvas.hasPointerCapture(event.pointerId)) canvas.releasePointerCapture(event.pointerId);
}, { signal: listeners.signal });
canvas.addEventListener("pointercancel", (event) => {
  if (event.pointerId !== state.activePointer) return;
  admitted(event);
  apply(session.cancel(performance.now()));
  state.activePointer = null;
}, { signal: listeners.signal });
for (const type of ["gotpointercapture", "lostpointercapture"]) {
  canvas.addEventListener(type, (event) => {
    state[type === "gotpointercapture" ? "gotCapture" : "lostCapture"]++;
    state.trustedInput &&= event.isTrusted;
  }, { signal: listeners.signal });
}
dispose.addEventListener("click", () => {
  if (state.activePointer !== null) {
    apply(session.cancel(performance.now()));
    if (canvas.hasPointerCapture(state.activePointer)) canvas.releasePointerCapture(state.activePointer);
    state.activePointer = null;
  }
  listeners.abort();
  session.dispose();
  session.dispose();
  try {
    session.begin({ x: 0, y: 0 }, performance.now());
  } catch (error) {
    state.disposalRejected = /disposed/.test(error.message);
  }
  state.retainedTraceAfterDisposal = finalTrace !== null &&
    JSON.stringify(tracePath(finalTrace)) === JSON.stringify(finalCommands);
  state.disposed = true;
  status.textContent = "Disposed · retained trace";
  dispose.disabled = true;
}, { once: true });
state.ready = true;
status.textContent = "Ready";
dispose.disabled = false;
