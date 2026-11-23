import initWasm, {
  initSync as initializeSync,
  RawSession,
  rawLiveTrace,
  rawPattern,
  rawValidatePattern,
  type RawPattern,
  type RawSessionReport,
  type RawTrace,
  type RawUpdate,
} from "./pointer_gestures_wasm.js";

export type Direction = "up" | "down" | "left" | "right";
export interface Point {
  x: number;
  y: number;
}
export type PathCommand =
  | { type: "move" | "line"; to: Point }
  | { type: "cubic"; control1: Point; control2: Point; to: Point }
  | { type: "close" };
/** Deeply immutable copied coordinates, valid after session changes or disposal. */
export interface GestureTrace {
  readonly directions: readonly Direction[];
  readonly points: readonly Readonly<Point>[];
  readonly tailPoint: Readonly<Point> | null;
}
export type SessionOutcome =
  | "matched"
  | "recorded"
  | "below-threshold"
  | "unmatched"
  | "unmatched-prefix"
  | "cancelled"
  | "stopped"
  | "superseded"
  | "total-deadline"
  | "idle-deadline"
  | "invalid-release"
  | "direction-limit";
/** Latest completed retained input. This records recognition and recovery requests.
 * Generation and pointCount are exact BigInts; use an explicit string conversion for JSON.
 */
export interface SessionReport {
  generation: bigint;
  outcome: SessionOutcome;
  elapsedMs: number;
  idleMs: number;
  pointCount: bigint;
  maximumAxisDistance: number;
  captured: boolean;
  recording: boolean;
  directions: Direction[];
  rejectedDirection: Direction | null;
}
export interface GestureUpdate {
  /** Present only on an exact completed match; zero is a valid identifier. */
  matchId: number | null;
  /** Includes final geometry on successful completion. Recognition performs no path planning. */
  trace: GestureTrace | null;
  /** A pending or recognized pointer session is still retained. */
  active: boolean;
  /** Absolute deadline on the supplied millisecond clock. Call tick at this time. */
  deadlineMs: number | null;
  /** Clear host-coalesced visible geometry before applying this update. */
  discardPendingVisibleTrace: boolean;
  expired: boolean;
  /** Persists across later updates. Compare generation to detect its replacement. */
  lastSession: SessionReport | null;
}
export interface GestureTuning {
  startDistance: number;
  changeDistance: number;
  maximumPoints: number;
  /** 1..4096 direction runs; default 64, captured at the start of each session. */
  maximumDirections: number;
  /** Zero disables this deadline. */
  totalDurationMs: number;
  /** Zero disables this deadline. */
  idleDurationMs: number;
}
export interface SessionConfiguration extends Partial<GestureTuning> {
  /** Record arbitrary patterns without matching or rejecting unregistered prefixes. */
  recording?: boolean;
}
export interface PatternConfiguration {
  width: number;
  height: number;
  padding?: number;
  lineWidth?: number;
  showsArrowhead?: boolean;
  cornerRadiusScale?: number;
}

/** Initialize once from the colocated Wasm file or a caller-supplied input. */
export default async function init(
  input?: RequestInfo | URL | Response | BufferSource | WebAssembly.Module,
): Promise<void> {
  await initWasm(input === undefined ? undefined : { module_or_path: input });
}

/** Initialize from verified bytes or a WebAssembly.Module without a network request. */
export function initSync(module: BufferSource | WebAssembly.Module): void {
  initializeSync({ module });
}

const directionNames: readonly (Direction | undefined)[] = [undefined, "up", "down", "left", "right"];
const outcomes: readonly (SessionOutcome | undefined)[] = [
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

// These lookups decode codes emitted by the Rust adapter for admitted input.
function decodeDirections(values: Uint8Array): Direction[] {
  return Array.from(values, (code) => directionNames[code]!);
}

function decodeSession(report: RawSessionReport | undefined): SessionReport | null {
  if (!report) return null;
  try {
    return {
      generation: report.generation,
      outcome: outcomes[report.outcome]!,
      elapsedMs: report.elapsedMs,
      idleMs: report.idleMs,
      pointCount: report.pointCount,
      maximumAxisDistance: report.maximumAxisDistance,
      captured: report.captured,
      recording: report.recording,
      directions: decodeDirections(report.directions()),
      rejectedDirection: directionNames[report.rejectedDirection ?? 0] ?? null,
    };
  } finally {
    report.free();
  }
}
type PatternRejection = "empty" | "too-long" | "repeated-direction" | "invalid-direction";

/** Rust-owned exact-pattern rejection. Indexes are zero-based; length failures have no index. */
export class GesturePatternError extends RangeError {
  readonly reason: PatternRejection;
  readonly index: number | null;

  constructor(reason: PatternRejection, index: number | null) {
    super(
      `Invalid gesture pattern: ${reason}${index === null ? "" : ` at index ${index}`}`,
    );
    this.name = "GesturePatternError";
    this.reason = reason;
    this.index = index;
  }
}

/** Validate without mutation; returns null when valid. Initialize Wasm first. */
export function validatePattern(pattern: readonly Direction[]): GesturePatternError | null {
  return patternError(rawValidatePattern(directionCodes(pattern, false)));
}

const patternRejections: readonly (PatternRejection | undefined)[] = [
  undefined,
  "empty",
  "too-long",
  "repeated-direction",
  "invalid-direction",
];

function patternError([code, index]: Uint32Array): GesturePatternError | null {
  return code === 0
    ? null
    : new GesturePatternError(patternRejections[code]!, index === 0xffffffff ? null : index);
}

function directionCodes(pattern: readonly Direction[], strict = true): Uint8Array {
  return Uint8Array.from(pattern, (direction) => {
    const code = directionNames.indexOf(direction);
    if (code < 1 && strict) throw new TypeError("Unknown gesture direction");
    return Math.max(0, code);
  });
}

function decodeCommands(values: Float64Array): PathCommand[] {
  const result: PathCommand[] = [];
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

function decodeUpdate(update: RawUpdate): GestureUpdate {
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

function decodeTrace(trace: RawTrace | undefined): GestureTrace | null {
  if (!trace) return null;
  try {
    const coordinates = trace.points();
    const points = [];
    for (let i = 0; i < coordinates.length; i += 2)
      points.push(Object.freeze({ x: coordinates[i], y: coordinates[i + 1] }));
    const tail = trace.tail();
    return Object.freeze({
      directions: Object.freeze(decodeDirections(trace.directions())),
      points: Object.freeze(points),
      tailPoint: tail.length ? Object.freeze({ x: tail[0], y: tail[1] }) : null,
    });
  } finally {
    trace.free();
  }
}

function pointLimits(maximumPoints: number, maximumDirections: number): void {
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
function throwNative(error: unknown): never {
  throw typeof error === "string" ? new RangeError(error) : error;
}

// Keep the native handles private and the generated public class structurally typed.
const sessionHandles = new WeakMap<GestureSession, RawSession>();

function sessionHandle(session: GestureSession): RawSession {
  const raw = sessionHandles.get(session);
  if (!raw) throw new Error("GestureSession has been disposed");
  return raw;
}

function step(
  session: GestureSession,
  method: "begin" | "moved" | "end",
  point: Point,
  nowMs: number,
): GestureUpdate;
function step(
  session: GestureSession,
  method: "cancel" | "tick",
  point: null,
  nowMs: number,
): GestureUpdate;
function step(
  session: GestureSession,
  method: "begin" | "moved" | "end" | "cancel" | "tick",
  point: Point | null,
  nowMs: number,
): GestureUpdate {
  const raw = sessionHandle(session);
  try {
    return decodeUpdate(
      method === "cancel" || method === "tick"
        ? raw[method](nowMs)
        : raw[method](point!.x, point!.y, nowMs),
    );
  } catch (error) {
    throwNative(error);
  }
}

/** Serial adapter to shared recognition and geometry. Hosts own input, time and UI. */
export class GestureSession {
  constructor({
    startDistance = 10,
    changeDistance = 25,
    maximumPoints = 256,
    maximumDirections = 64,
    totalDurationMs = 1000,
    idleDurationMs = 0,
    recording = false,
  }: SessionConfiguration = {}) {
    if (typeof recording !== "boolean")
      throw new TypeError("recording must be a boolean");
    pointLimits(maximumPoints, maximumDirections);
    try {
      sessionHandles.set(
        this,
        new RawSession(
          startDistance,
          changeDistance,
          maximumPoints,
          maximumDirections,
          totalDurationMs,
          idleDurationMs,
          recording,
        ),
      );
    } catch (error) {
      throwNative(error);
    }
  }

  /** Register 1..4096 direction runs without equal adjacent directions.
   * IDs are integers in [0, 4294967295]. Duplicate patterns replace earlier IDs.
   */
  register(pattern: readonly Direction[], identifier: number): void {
    if (
      !Number.isInteger(identifier) ||
      identifier < 0 ||
      identifier > 0xffffffff
    )
      throw new RangeError("Gesture identifiers must be uint32 values");
    const session = sessionHandle(this);
    const error = patternError(
      session.register(directionCodes(pattern, false), identifier),
    );
    if (error) throw error;
  }

  /** Remove one exact future registration; returns its uint32 ID or null. */
  remove(pattern: readonly Direction[]): number | null {
    return sessionHandle(this).remove(directionCodes(pattern)) ?? null;
  }

  /** Clear future registrations. Active input retains its captured matcher. */
  clear(): void {
    sessionHandle(this).clear();
  }

  /** Change future recording mode. Active input keeps its captured mode. Times must be monotonic. */
  setRecording(enabled: boolean, nowMs: number): GestureUpdate {
    if (typeof enabled !== "boolean")
      throw new TypeError("recording must be a boolean");
    try {
      return decodeUpdate(sessionHandle(this).set_recording(enabled, nowMs));
    } catch (error) {
      throwNative(error);
    }
  }

  /** Atomically replace tuning. Changes restart active deadlines; unchanged values do so
   * only with resetUnchanged. Invalid settings or time preserve both input and configuration.
   */
  updateTuning(
    {
      startDistance,
      changeDistance,
      maximumPoints,
      maximumDirections,
      totalDurationMs,
      idleDurationMs,
    }: GestureTuning,
    nowMs: number,
    { resetUnchanged = false }: { resetUnchanged?: boolean } = {},
  ): GestureUpdate {
    pointLimits(maximumPoints, maximumDirections);
    if (typeof resetUnchanged !== "boolean")
      throw new TypeError("resetUnchanged must be a boolean");
    try {
      return decodeUpdate(
        sessionHandle(this).update_tuning(
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

  begin(point: Point, nowMs: number): GestureUpdate {
    return step(this, "begin", point, nowMs);
  }
  move(point: Point, nowMs: number): GestureUpdate {
    return step(this, "moved", point, nowMs);
  }
  /** A non-finite release point interrupts retained input and discards geometry without matching. */
  end(point: Point, nowMs: number): GestureUpdate {
    return step(this, "end", point, nowMs);
  }
  /** Cancel retained input and return its update; the session remains reusable. */
  cancel(nowMs: number): GestureUpdate {
    return step(this, "cancel", null, nowMs);
  }
  tick(nowMs: number): GestureUpdate {
    return step(this, "tick", null, nowMs);
  }

  /** Release Wasm memory without processing input. Repeated disposal is harmless. */
  dispose(): void {
    const raw = sessionHandles.get(this);
    if (!raw) return;
    sessionHandles.delete(this);
    raw.free();
  }
}

/** Plan live geometry on presentation demand; does not retain or mutate its snapshot. */
export function tracePath(
  trace: GestureTrace | null,
  { cornerRadiusScale = 1 }: { cornerRadiusScale?: number } = {},
): PathCommand[] {
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
  directions: readonly Direction[],
  {
    width,
    height,
    padding = 8,
    lineWidth = 3,
    showsArrowhead = true,
    cornerRadiusScale = 1,
  }: PatternConfiguration,
): { commands: PathCommand[]; arrowhead: PathCommand[] } {
  let geometry: RawPattern;
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
