import initWasm, {
  initSync as initializeSync,
  RawSession,
  rawLiveTrace,
  rawPattern,
  rawValidatePattern,
} from "./pointer_gestures_wasm.js";

/** Initialize the colocated Wasm file, or a caller-supplied URL/module/response. */
export default async function init(input) {
  await initWasm(input === undefined ? undefined : { module_or_path: input });
}

/** Initialize from verified bytes or a WebAssembly.Module without a network request. */
export function initSync(module) {
  initializeSync({ module });
}

const directionNames = [undefined, "up", "down", "left", "right"];
const outcomes = [
  undefined,
  "matched",
  "recorded",
  "below-threshold",
  "unmatched",
  "unmatched-prefix",
  "cancelled",
  "stopped",
  "superseded",
  "total-deadline",
  "idle-deadline",
  "invalid-release",
  "direction-limit",
];
function decodeSession(report) {
  if (!report) return null;
  try {
    return {
      generation: report.generation,
      outcome: outcomes[report.outcome],
      elapsedMs: report.elapsedMs,
      idleMs: report.idleMs,
      pointCount: report.pointCount,
      maximumAxisDistance: report.maximumAxisDistance,
      captured: report.captured,
      recording: report.recording,
      directions: Array.from(
        report.directions(),
        (code) => directionNames[code],
      ),
      rejectedDirection: directionNames[report.rejectedDirection] ?? null,
    };
  } finally {
    report.free();
  }
}
/** Rust-owned rejection reason and optional zero-based direction index. */
export class GesturePatternError extends RangeError {
  constructor(reason, index) {
    super(
      `Invalid gesture pattern: ${reason}${index === null ? "" : ` at index ${index}`}`,
    );
    this.name = "GesturePatternError";
    this.reason = reason;
    this.index = index;
  }
}

/** Validate without mutation; returns null when valid. Initialize Wasm first. */
export function validatePattern(pattern) {
  return patternError(rawValidatePattern(directionCodes(pattern, false)));
}

function patternError([code, index]) {
  return code === 0
    ? null
    : new GesturePatternError(
        [
          undefined,
          "empty",
          "too-long",
          "repeated-direction",
          "invalid-direction",
        ][code],
        index === 0xffffffff ? null : index,
      );
}

function directionCodes(pattern, strict = true) {
  return Uint8Array.from(pattern, (direction) => {
    const code = directionNames.indexOf(direction);
    if (code < 1 && strict) throw new TypeError("Unknown gesture direction");
    return Math.max(0, code);
  });
}

function decodeCommands(values) {
  const result = [];
  let cursor = 0;
  const point = () => ({ x: values[cursor++], y: values[cursor++] });
  while (cursor < values.length) {
    const kind = values[cursor++];
    switch (kind) {
      case 1:
        result.push({ type: "move", to: point() });
        break;
      case 2:
        result.push({ type: "line", to: point() });
        break;
      case 3:
        result.push({
          type: "cubic",
          control1: point(),
          control2: point(),
          to: point(),
        });
        break;
      case 4:
        result.push({ type: "close" });
        break;
      default:
        throw new Error("Unsupported gesture path command");
    }
  }
  return result;
}

function decodeUpdate(update) {
  try {
    return {
      matchId: update.matched ?? null,
      trace: decodeTrace(update.takeTrace()),
      active: update.active,
      deadlineMs: update.deadlineMs ?? null,
      discardPendingVisibleTrace: update.discardPendingVisibleTrace,
      expired: update.expired,
      lastSession: decodeSession(update.takeLastSession()),
    };
  } finally {
    update.free();
  }
}

function decodeTrace(trace) {
  if (!trace) return null;
  try {
    const coordinates = trace.points();
    const points = [];
    for (let i = 0; i < coordinates.length; i += 2)
      points.push(Object.freeze({ x: coordinates[i], y: coordinates[i + 1] }));
    const tail = trace.tail();
    return Object.freeze({
      directions: Object.freeze(
        Array.from(trace.directions(), (code) => directionNames[code]),
      ),
      points: Object.freeze(points),
      tailPoint: tail.length ? Object.freeze({ x: tail[0], y: tail[1] }) : null,
    });
  } finally {
    trace.free();
  }
}

function pointLimits(maximumPoints, maximumDirections) {
  if (
    !Number.isInteger(maximumPoints) ||
    maximumPoints < 1 ||
    maximumPoints > 0xffffffff
  )
    throw new RangeError("maximumPoints must be a positive uint32");
  if (
    !Number.isInteger(maximumDirections) ||
    maximumDirections < 0 ||
    maximumDirections > 0xffffffff
  )
    throw new RangeError("maximumDirections must fit uint32");
}

// Preserve Wasm/runtime exceptions; numeric and geometry validation uses standard JS errors.
function throwNative(error) {
  throw typeof error === "string" ? new RangeError(error) : error;
}

/** Serial adapter to shared recognition and geometry. Hosts own input, time and UI. */
export class GestureSession {
  #raw;

  constructor({
    startDistance = 10,
    changeDistance = 25,
    maximumPoints = 256,
    maximumDirections = 64,
    totalDurationMs = 1000,
    idleDurationMs = 0,
    recording = false,
  } = {}) {
    if (typeof recording !== "boolean")
      throw new TypeError("recording must be a boolean");
    pointLimits(maximumPoints, maximumDirections);
    try {
      this.#raw = new RawSession(
        startDistance,
        changeDistance,
        maximumPoints,
        maximumDirections,
        totalDurationMs,
        idleDurationMs,
        recording,
      );
    } catch (error) {
      throwNative(error);
    }
  }

  #session() {
    if (!this.#raw) throw new Error("GestureSession has been disposed");
    return this.#raw;
  }

  register(pattern, identifier) {
    if (
      !Number.isInteger(identifier) ||
      identifier < 0 ||
      identifier > 0xffffffff
    )
      throw new RangeError("Gesture identifiers must be uint32 values");
    const session = this.#session();
    const error = patternError(
      session.register(directionCodes(pattern, false), identifier),
    );
    if (error) throw error;
  }

  /** Remove one exact future registration; returns its uint32 ID or null. */
  remove(pattern) {
    return this.#session().remove(directionCodes(pattern)) ?? null;
  }

  /** Clear future registrations. Active input retains its captured matcher. */
  clear() {
    this.#session().clear();
  }

  /** Change future recording mode. Active input keeps its captured mode. */
  setRecording(enabled, nowMs) {
    if (typeof enabled !== "boolean")
      throw new TypeError("recording must be a boolean");
    try {
      return decodeUpdate(this.#session().set_recording(enabled, nowMs));
    } catch (error) {
      throwNative(error);
    }
  }

  /** Replace complete mutable tuning; return current geometry and rescheduled deadline. */
  updateTuning(
    {
      startDistance,
      changeDistance,
      maximumPoints,
      maximumDirections,
      totalDurationMs,
      idleDurationMs,
    },
    nowMs,
    { resetUnchanged = false } = {},
  ) {
    pointLimits(maximumPoints, maximumDirections);
    if (typeof resetUnchanged !== "boolean")
      throw new TypeError("resetUnchanged must be a boolean");
    try {
      return decodeUpdate(
        this.#session().update_tuning(
          startDistance,
          changeDistance,
          maximumPoints,
          maximumDirections,
          totalDurationMs,
          idleDurationMs,
          nowMs,
          resetUnchanged,
        ),
      );
    } catch (error) {
      throwNative(error);
    }
  }

  #step(method, point, nowMs) {
    const raw = this.#session();
    try {
      return decodeUpdate(
        point === null
          ? raw[method](nowMs)
          : raw[method](point.x, point.y, nowMs),
      );
    } catch (error) {
      throwNative(error);
    }
  }

  begin(point, nowMs) {
    return this.#step("begin", point, nowMs);
  }
  move(point, nowMs) {
    return this.#step("moved", point, nowMs);
  }
  end(point, nowMs) {
    return this.#step("end", point, nowMs);
  }
  cancel(nowMs) {
    return this.#step("cancel", null, nowMs);
  }
  tick(nowMs) {
    return this.#step("tick", null, nowMs);
  }

  /** Release Wasm memory without processing input. Repeated disposal is harmless. */
  dispose() {
    if (!this.#raw) return;
    const raw = this.#raw;
    this.#raw = undefined;
    raw.free();
  }
}

/** Plan live geometry on presentation demand; does not retain or mutate its snapshot. */
export function tracePath(trace, { cornerRadiusScale = 1 } = {}) {
  if (trace === null) return [];
  const coordinates = new Float64Array(trace.points.length * 2);
  for (let i = 0; i < trace.points.length; i++) {
    coordinates[i * 2] = trace.points[i].x;
    coordinates[i * 2 + 1] = trace.points[i].y;
  }
  try {
    return decodeCommands(
      rawLiveTrace(
        directionCodes(trace.directions),
        coordinates,
        trace.tailPoint === null
          ? new Float64Array()
          : new Float64Array([trace.tailPoint.x, trace.tailPoint.y]),
        cornerRadiusScale,
      ),
    );
  } catch (error) {
    throwNative(error);
  }
}

/** Shared pattern geometry; styling, SVG serialization and drawing stay with hosts. */
export function patternPath(
  directions,
  {
    width,
    height,
    padding = 8,
    lineWidth = 3,
    showsArrowhead = true,
    cornerRadiusScale = 1,
  },
) {
  let geometry;
  try {
    geometry = rawPattern(
      directionCodes(directions),
      width,
      height,
      padding,
      lineWidth,
      showsArrowhead,
      cornerRadiusScale,
    );
  } catch (error) {
    throwNative(error);
  }
  try {
    return {
      commands: decodeCommands(geometry.takeCommands()),
      arrowhead: decodeCommands(geometry.takeArrowhead()),
    };
  } finally {
    geometry.free();
  }
}
