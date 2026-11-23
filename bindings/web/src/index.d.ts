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
/** Latest completed retained input; recognition/recovery requests, not OS action success.
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
  /** Persists across later updates. Compare generation; this is not an event history. */
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
/** Initialize once before constructing sessions or planning paths. */
export default function init(
  input?: RequestInfo | URL | Response | BufferSource | WebAssembly.Module,
): Promise<void>;
/** Initialize from verified bytes or a compiled module, including build-time/Node hosts. */
export function initSync(module: BufferSource | WebAssembly.Module): void;
export class GestureSession {
  constructor(configuration?: SessionConfiguration);
  /** Register 1..4096 direction runs without equal adjacent directions.
   * IDs are integers in [0, 4294967295]. Duplicate patterns replace earlier IDs. */
  register(pattern: readonly Direction[], identifier: number): void;
  /** Remove an exact future registration; active input keeps its snapshot. */
  remove(pattern: readonly Direction[]): number | null;
  /** Clear future registrations without cancelling active input. */
  clear(): void;
  /** Affects future strokes; active input keeps its captured mode. Times must be monotonic. */
  setRecording(enabled: boolean, nowMs: number): GestureUpdate;
  /** Atomically replace tuning. Changes restart active deadlines; unchanged values do so
   * only with resetUnchanged. Invalid settings/time preserve both input and configuration. */
  updateTuning(
    tuning: GestureTuning,
    nowMs: number,
    options?: { resetUnchanged?: boolean },
  ): GestureUpdate;
  begin(point: Point, nowMs: number): GestureUpdate;
  move(point: Point, nowMs: number): GestureUpdate;
  /** A non-finite release point interrupts retained input and discards geometry without matching. */
  end(point: Point, nowMs: number): GestureUpdate;
  /** Cancel retained input and return its update; the session remains reusable. */
  cancel(nowMs: number): GestureUpdate;
  tick(nowMs: number): GestureUpdate;
  /** Release Wasm memory without processing input. Repeated calls are harmless. */
  dispose(): void;
}
/** Rust path planning on demand. A null trace produces no commands. */
export function tracePath(
  trace: GestureTrace | null,
  configuration?: { cornerRadiusScale?: number },
): PathCommand[];
export function patternPath(
  directions: readonly Direction[],
  configuration: PatternConfiguration,
): {
  commands: PathCommand[];
  arrowhead: PathCommand[];
};

/** Rust-owned exact-pattern rejection. Indexes are zero-based; length failures have no index. */
export class GesturePatternError extends RangeError {
  constructor(
    reason: "empty" | "too-long" | "repeated-direction" | "invalid-direction",
    index: number | null,
  );
  readonly reason:
    "empty" | "too-long" | "repeated-direction" | "invalid-direction";
  readonly index: number | null;
}
/** Validate without mutation. Initialize Wasm first; null means valid. */
export function validatePattern(
  pattern: readonly Direction[],
): GesturePatternError | null;
