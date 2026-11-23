import init, {
  initSync,
  GesturePatternError,
  GestureSession,
  patternPath,
  tracePath,
  validatePattern,
  type Direction,
  type GestureTuning,
  type GestureUpdate,
  type PathCommand,
} from "@naviapps/pointer-gestures";

// This file is checked with noEmit. Initialization and session code never execute.
async function compileOnly(bytes: ArrayBuffer, module: WebAssembly.Module): Promise<void> {
  await init(new URL("https://example.invalid/owned.wasm"));
  await init(new Uint8Array(bytes));
  initSync(module);
  // Consumers can substitute this public session shape without a Wasm handle.
  const implementation: Pick<GestureSession, keyof GestureSession> = new GestureSession({
    recording: true,
    totalDurationMs: 0,
  });
  const session: GestureSession = implementation;
  const pattern: readonly Direction[] = ["down", "right"];
  session.register(pattern, 42);
  const tuning: GestureTuning = {
    startDistance: 10,
    changeDistance: 25,
    maximumPoints: 256,
    maximumDirections: 64,
    totalDurationMs: 1000,
    idleDurationMs: 0,
  };
  session.updateTuning(tuning, 10, { resetUnchanged: true });
  session.setRecording(false, 11);
  const update: GestureUpdate = session.end({ x: 100, y: 80 }, 12);
  if (update.matchId !== null) {
    const identifier: number = update.matchId;
    session.register(pattern, identifier);
  }
  if (update.lastSession !== null) {
    const generation: bigint = update.lastSession.generation;
    const pointCount: bigint = update.lastSession.pointCount;
    const report: string = `${generation}:${pointCount}:${update.lastSession.outcome}`;
    void report;
  }
  const commands: PathCommand[] = tracePath(update.trace);
  commands.push(...patternPath(pattern, { width: 100, height: 80 }).commands);
  for (const command of commands) {
    if (command.type === "cubic") {
      const control: number = command.control1.x + command.control2.y;
      void control;
    } else if (command.type !== "close") {
      const endpoint: number = command.to.x;
      void endpoint;
    }
  }
  const failure = validatePattern(pattern);
  if (failure instanceof GesturePatternError) {
    const reason: "empty" | "too-long" | "repeated-direction" | "invalid-direction" = failure.reason;
    const index: number | null = failure.index;
    void reason;
    void index;
  }

  // @ts-expect-error Only the declared cardinal directions are accepted.
  session.register(["diagonal"], 1);
  // @ts-expect-error Identifiers are numbers, not serialized strings.
  session.register(pattern, "42");
  // @ts-expect-error Updating tuning requires the complete tuning value.
  session.updateTuning({ startDistance: 10 }, 20);
  // @ts-expect-error Pattern dimensions include both width and height.
  patternPath(pattern, { width: 100 });
  // @ts-expect-error A nullable match must be narrowed before numeric use.
  const match: number = update.matchId;
  if (update.trace !== null && update.trace.points.length > 0) {
    // @ts-expect-error Copied trace coordinates are readonly.
    update.trace.points[0].x = 0;
  }
  if (update.lastSession !== null) {
    // @ts-expect-error Session generation preserves its bigint range.
    const generation: number = update.lastSession.generation;
    void generation;
  }
  void match;
  session.dispose();
}

void compileOnly;
