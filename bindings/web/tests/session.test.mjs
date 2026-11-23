import { test } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { pathToFileURL } from "node:url";
import { resolve } from "node:path";

const directory = resolve(
  process.env.POINTER_GESTURES_WEB_PACKAGE ?? ".web-build/package",
);
const {
  initSync,
  GestureSession,
  tracePath,
  patternPath,
  validatePattern,
  GesturePatternError,
} = await import(pathToFileURL(resolve(directory, "index.js")));
initSync(await readFile(resolve(directory, "pointer_gestures_wasm_bg.wasm")));

test("Rust numeric rejections use standard JavaScript errors without changing input", () => {
  assert.throws(() => new GestureSession({ startDistance: -1 }), RangeError);
  const session = new GestureSession({ recording: true, totalDurationMs: 0 });
  try {
    session.begin({ x: 0, y: 0 }, 10);
    assert.throws(() => session.move({ x: NaN, y: 0 }, 11), RangeError);
    assert.throws(() => session.tick(9), RangeError);
    assert.throws(() => session.setRecording(false, NaN), RangeError);
    const result = session.move({ x: 40, y: 0 }, 12);
    assert.equal(result.active, true);
    assert.throws(() => tracePath({ ...result.trace, points: [] }), RangeError);
  } finally {
    session.dispose();
  }
});

test("Wasm execution errors retain their identity", async () => {
  const { RawSession } = await import(
    pathToFileURL(resolve(directory, "pointer_gestures_wasm.js"))
  );
  const session = new GestureSession();
  const original = RawSession.prototype.tick;
  const failure = new WebAssembly.RuntimeError("Owned test trap");
  try {
    RawSession.prototype.tick = () => {
      throw failure;
    };
    assert.throws(
      () => session.tick(0),
      (error) => error === failure,
    );
  } finally {
    RawSession.prototype.tick = original;
    session.dispose();
  }
});

test("recognition returns immutable independent traces and plans only on demand", () => {
  const session = new GestureSession({ recording: true, totalDurationMs: 0 });
  session.begin({ x: 0, y: 0 }, 0);
  let result;
  for (let i = 1; i <= 512; i++) result = session.move({ x: i, y: 0 }, i);
  assert.equal("commands" in result, false);
  assert.equal("directions" in result, false);
  const trace = result.trace;
  assert.ok(
    Object.isFrozen(trace) &&
      Object.isFrozen(trace.points) &&
      Object.isFrozen(trace.points[0]),
  );
  assert.ok(
    Object.isFrozen(trace.directions) && Object.isFrozen(trace.tailPoint),
  );
  const planned = tracePath(trace);
  session.cancel(513);
  session.dispose();
  assert.deepEqual(tracePath(trace), planned);
  assert.throws(() => {
    trace.points[0].x = 99;
  }, TypeError);
  assert.throws(() => {
    trace.directions.push("down");
  }, TypeError);
  assert.throws(() => tracePath({ ...trace, points: [] }));
  assert.throws(() => tracePath({ ...trace, tailPoint: { x: NaN, y: 0 } }));
  assert.deepEqual(tracePath(null), []);
});

test("live recording changes preserve registrations and the active mode snapshot", () => {
  const session = new GestureSession({ totalDurationMs: 0 });
  try {
    session.register(["right"], 7);
    session.begin({ x: 0, y: 0 }, 0);
    session.setRecording(true, 10);
    session.move({ x: 40, y: 0 }, 20);
    assert.equal(session.end({ x: 50, y: 0 }, 30).matchId, 7);
    session.begin({ x: 0, y: 0 }, 40);
    session.setRecording(false, 50);
    session.move({ x: 0, y: 40 }, 60);
    const recorded = session.end({ x: 0, y: 50 }, 70);
    assert.equal(recorded.matchId, null);
    assert.equal(recorded.lastSession.recording, true);
    session.begin({ x: 0, y: 0 }, 80);
    session.move({ x: 40, y: 0 }, 90);
    assert.equal(session.end({ x: 50, y: 0 }, 100).matchId, 7);
    assert.throws(() => session.setRecording("yes", 110), TypeError);
  } finally {
    session.dispose();
  }
});

test("live tuning reschedules deadlines atomically without rebuilding the matcher", () => {
  const tuning = {
    startDistance: 10,
    changeDistance: 25,
    maximumPoints: 256,
    maximumDirections: 64,
    totalDurationMs: 200,
    idleDurationMs: 100,
  };
  const session = new GestureSession({ totalDurationMs: 0 });
  try {
    session.register(["right"], 42);
    session.begin({ x: 0, y: 0 }, 0);
    session.move({ x: 40, y: 0 }, 10);
    assert.equal(session.updateTuning(tuning, 40).deadlineMs, 140);
    assert.equal(session.updateTuning(tuning, 50).deadlineMs, 140);
    assert.equal(
      session.updateTuning(tuning, 60, { resetUnchanged: true }).deadlineMs,
      160,
    );
    assert.throws(() =>
      session.updateTuning({ ...tuning, startDistance: -1 }, 70),
    );
    assert.throws(() => session.updateTuning(tuning, NaN));
    assert.throws(() => session.setRecording(true, 59));
    assert.equal(session.tick(65).deadlineMs, 160);
    assert.equal(session.end({ x: 50, y: 0 }, 80).matchId, 42);
    assert.throws(
      () => session.updateTuning(tuning, 90, { resetUnchanged: 1 }),
      TypeError,
    );
  } finally {
    session.dispose();
  }
});

test("packaged runtime exactly matches its source provenance and checksums", async () => {
  const receipt = JSON.parse(
    await readFile(resolve(directory, "provenance.json"), "utf8"),
  );
  assert.match(receipt.sourceRevision, /^[a-f0-9]{40}$/);
  const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
  for (const [path, checksum] of Object.entries(receipt.outputs))
    assert.equal(sha(await readFile(resolve(directory, path))), checksum, path);
  for (const line of (await readFile(resolve(directory, "SHA256SUMS"), "utf8"))
    .trim()
    .split("\n")) {
    const [checksum, path] = line.split("  ");
    assert.equal(sha(await readFile(resolve(directory, path))), checksum, path);
  }
});

test("arbitrary registered patterns retain final curves and maximum identifiers", () => {
  const session = new GestureSession({ totalDurationMs: 0 });
  try {
    session.register(["right", "up"], 0xffffffff);
    const begun = session.begin({ x: 10, y: 180 }, 0);
    assert.equal(begun.active, true);
    assert.equal(begun.deadlineMs, null);
    session.move({ x: 100, y: 180 }, 10);
    session.move({ x: 100, y: 70 }, 20);
    const result = session.end({ x: 100, y: 40 }, 30);
    assert.equal(result.matchId, 0xffffffff);
    assert.deepEqual(result.trace?.directions ?? [], ["right", "up"]);
    assert.equal(result.active, false);
    assert.equal(result.expired, false);
    assert.ok(
      tracePath(result.trace).some((command) => command.type === "cubic"),
    );
    assert.deepEqual(tracePath(result.trace).at(-1), {
      type: "line",
      to: { x: 100, y: 40 },
    });
  } finally {
    session.dispose();
  }
});

test("zero IDs and captured matcher snapshots survive later registration", () => {
  const session = new GestureSession();
  try {
    session.register(["left"], 0);
    session.begin({ x: 100, y: 10 }, 0);
    session.register(["left"], 9);
    session.move({ x: 20, y: 10 }, 10);
    assert.equal(session.end({ x: 0, y: 10 }, 20).matchId, 0);
    session.begin({ x: 100, y: 10 }, 30);
    session.move({ x: 20, y: 10 }, 40);
    assert.equal(session.end({ x: 0, y: 10 }, 50).matchId, 9);
  } finally {
    session.dispose();
  }
});

test("cancel and prefix rejection discard pending paths without a match", () => {
  for (const interrupt of [
    (session) => session.cancel(20),
    (session) => session.move({ x: 100, y: 100 }, 20),
  ]) {
    const session = new GestureSession();
    try {
      session.register(["down"], 1);
      session.begin({ x: 0, y: 0 }, 0);
      session.move({ x: 0, y: 80 }, 10);
      const result = interrupt(session);
      assert.equal(result.matchId, null);
      assert.equal(result.discardPendingVisibleTrace, true);
      assert.equal(result.active, false);
      assert.deepEqual(tracePath(result.trace), []);
      assert.deepEqual(
        tracePath(session.end({ x: 100, y: 120 }, 30).trace),
        [],
      );
    } finally {
      session.dispose();
    }
  }
});

test("shared idle and total deadlines expire without emitting a match", () => {
  for (const configuration of [
    { totalDurationMs: 100, idleDurationMs: 0 },
    { totalDurationMs: 0, idleDurationMs: 90 },
  ]) {
    const session = new GestureSession(configuration);
    try {
      session.register(["down"], 1);
      session.begin({ x: 0, y: 0 }, 0);
      const moved = session.move({ x: 0, y: 80 }, 10);
      assert.equal(moved.deadlineMs, 100);
      const expired = session.tick(100);
      assert.equal(expired.expired, true);
      assert.equal(expired.discardPendingVisibleTrace, true);
      assert.equal(expired.active, false);
      assert.equal(expired.matchId, null);
      assert.deepEqual(tracePath(expired.trace), []);
    } finally {
      session.dispose();
    }
  }
});

test("arbitrary static previews contain cubic curves and optional closed arrowheads", () => {
  const pattern = ["down", "right", "up", "left"];
  const result = patternPath(pattern, { width: 440, height: 240 });
  assert.ok(result.commands.some((command) => command.type === "cubic"));
  assert.equal(result.arrowhead.at(-1).type, "close");
  assert.deepEqual(
    patternPath(pattern, { width: 440, height: 240, showsArrowhead: false })
      .arrowhead,
    [],
  );
  for (const command of [...result.commands, ...result.arrowhead])
    for (const point of [command.to, command.control1, command.control2].filter(
      Boolean,
    ))
      assert.ok(Number.isFinite(point.x) && Number.isFinite(point.y));
});

test("invalid input is rejected and disposal is explicit and idempotent", () => {
  assert.throws(() => new GestureSession({ maximumPoints: 0 }));
  assert.throws(() => new GestureSession({ totalDurationMs: NaN }));
  const session = new GestureSession();
  assert.throws(() => session.register(["down"], -1));
  assert.throws(() => session.register(["diagonal"], 1));
  assert.throws(() => session.register([], 1));
  assert.throws(() => session.register(["right", "right"], 1));
  session.register(["down"], 1);
  session.begin({ x: 0, y: 0 }, 10);
  assert.throws(() => session.move({ x: 0, y: 80 }, 9));
  assert.throws(() => session.move({ x: NaN, y: 80 }, 11));
  session.dispose();
  session.dispose();
  assert.throws(() => session.begin({ x: 0, y: 0 }, 20), /disposed/);
});

test("disposal releases ownership without executing input transitions", async () => {
  const { RawSession } = await import(
    pathToFileURL(resolve(directory, "pointer_gestures_wasm.js"))
  );
  const cancel = RawSession.prototype.cancel;
  const free = RawSession.prototype.free;
  try {
    RawSession.prototype.cancel = () => {
      throw new Error("Disposal must not execute a logical input transition");
    };
    for (const stage of ["idle", "pending", "capturing"]) {
      const session = new GestureSession({ recording: true, totalDurationMs: 0 });
      if (stage !== "idle") session.begin({ x: 0, y: 0 }, 1);
      const trace =
        stage === "capturing" ? session.move({ x: 40, y: 0 }, 2).trace : null;
      const commands = tracePath(trace);
      let releases = 0;
      RawSession.prototype.free = function () {
        releases++;
        // Ownership is detached before release, including synchronous reentry.
        assert.throws(() => session.tick(3), /disposed/);
        session.dispose();
        free.call(this);
      };
      assert.doesNotThrow(() => session.dispose(), stage);
      session.dispose();
      assert.equal(releases, 1, stage);
      assert.deepEqual(tracePath(trace), commands);
    }
  } finally {
    RawSession.prototype.cancel = cancel;
    RawSession.prototype.free = free;
  }
});

test("non-finite release ends pending or visible input without reviving a match", () => {
  for (const active of [false, true]) {
    for (const point of [
      { x: NaN, y: 0 },
      { x: 0, y: Infinity },
      { x: -Infinity, y: 0 },
    ]) {
      const session = new GestureSession({
        totalDurationMs: 0,
        idleDurationMs: 0,
      });
      try {
        session.register(["right"], 42);
        session.begin({ x: 0, y: 0 }, 0);
        if (active) session.move({ x: 40, y: 0 }, 1);
        assert.throws(() => session.end(point, NaN));
        assert.equal(session.tick(1).active, true);
        const result = session.end(point, 2);
        assert.equal(result.active, false);
        assert.equal(result.matchId, null);
        assert.equal(result.deadlineMs, null);
        assert.equal(result.discardPendingVisibleTrace, true);
        assert.deepEqual(tracePath(result.trace), []);
        assert.deepEqual(result.trace?.directions ?? [], []);
        assert.equal(session.end({ x: 80, y: 0 }, 3).matchId, null);
        session.begin({ x: 0, y: 0 }, 4);
        session.move({ x: 40, y: 0 }, 5);
        assert.equal(session.end({ x: 80, y: 0 }, 6).matchId, 42);
      } finally {
        session.dispose();
      }
    }
  }
});

test("recording accepts unregistered shapes, keeps final geometry and never claims an action", () => {
  const session = new GestureSession({ recording: true, totalDurationMs: 0 });
  try {
    session.register(["up"], 9);
    session.begin({ x: 40, y: 180 }, 0);
    session.move({ x: 40, y: 60 }, 2000);
    session.move({ x: 180, y: 60 }, 4000);
    const result = session.end({ x: 180, y: 170 }, 6000);
    assert.deepEqual(result.trace?.directions ?? [], ["up", "right", "down"]);
    assert.equal(result.matchId, null);
    assert.equal(result.deadlineMs, null);
    assert.equal(result.active, false);
    assert.ok(tracePath(result.trace).length > 3);
    session.begin({ x: 40, y: 180 }, 7000);
    session.move({ x: 40, y: 60 }, 7100);
    assert.deepEqual(tracePath(session.cancel(7200).trace), []);
    assert.deepEqual(
      session.end({ x: 40, y: 20 }, 7300).trace?.directions ?? [],
      [],
    );
    session.begin({ x: 40, y: 180 }, 7400);
    session.move({ x: 40, y: 60 }, 7500);
    assert.deepEqual(tracePath(session.end({ x: NaN, y: 20 }, 7600).trace), []);
  } finally {
    session.dispose();
  }
});

test("copied session reports preserve decisions, measurements and budget recovery", () => {
  for (const invalid of [0, 4097, 0xffffffff])
    assert.throws(() => new GestureSession({ maximumDirections: invalid }));
  const session = new GestureSession({
    recording: true,
    totalDurationMs: 0,
    maximumDirections: 1,
  });
  try {
    assert.equal(session.begin({ x: 0, y: 0 }, 0).lastSession, null);
    session.move({ x: 40, y: 0 }, 10);
    const report = session.end({ x: 40, y: 0 }, 20).lastSession;
    assert.equal(typeof report.generation, "bigint");
    assert.equal(report.outcome, "recorded");
    assert.equal(report.elapsedMs, 20);
    assert.equal(report.idleMs, 10);
    assert.equal(report.pointCount, 3n);
    assert.equal(report.maximumAxisDistance, 40);
    assert.deepEqual(report.directions, ["right"]);
    assert.equal(report.captured, true);
    assert.equal(report.recording, true);
    assert.equal(report.rejectedDirection, null);
    assert.deepEqual(session.begin({ x: 0, y: 0 }, 30).lastSession, report);
    session.move({ x: 40, y: 0 }, 40);
    const limited = session.move({ x: 40, y: 40 }, 50);
    assert.equal(limited.lastSession.outcome, "direction-limit");
    assert.equal(limited.lastSession.rejectedDirection, "down");
    assert.deepEqual(limited.lastSession.directions, ["right"]);
    assert.notEqual(limited.lastSession.generation, report.generation);
    assert.equal(limited.active, false);
    assert.deepEqual(tracePath(limited.trace), []);
    assert.equal(session.end({ x: 40, y: 40 }, 60).matchId, null);
    assert.deepEqual(report.directions, ["right"]);
  } finally {
    session.dispose();
  }
});

test("remove and clear preserve captured registrations and affect only future sessions", () => {
  const session = new GestureSession();
  try {
    session.register(["right"], 0);
    session.register(["down"], 0xffffffff);
    session.begin({ x: 0, y: 0 }, 0);
    assert.equal(session.remove(["right"]), 0);
    assert.equal(session.remove(["right"]), null);
    session.clear();
    session.clear();
    session.move({ x: 80, y: 0 }, 1);
    assert.equal(session.end({ x: 80, y: 0 }, 2).matchId, 0);
    assert.equal(session.begin({ x: 0, y: 0 }, 3).active, false);
    session.register(["down"], 0xffffffff);
    assert.equal(session.remove(["down"]), 0xffffffff);
    assert.throws(() => session.remove(["diagonal"]));
  } finally {
    session.dispose();
  }
  assert.throws(() => session.clear(), /disposed/);
  assert.throws(() => session.remove(["right"]), /disposed/);
});

test("structured registration errors preserve earlier registrations", () => {
  const session = new GestureSession({ totalDurationMs: 0 });
  try {
    session.register(["right"], 0);
    assert.equal(validatePattern(["right", "down"]), null);
    assert.deepEqual(
      [validatePattern([]).reason, validatePattern([]).index],
      ["empty", null],
    );
    assert.equal(validatePattern(["invalid"]).reason, "invalid-direction");
    const error = validatePattern(["right", "down", "down"]);
    assert.ok(error instanceof GesturePatternError);
    assert.equal(error.reason, "repeated-direction");
    assert.equal(error.index, 2);
    assert.throws(
      () => session.register(["right", "right"], 1),
      (error) => error instanceof GesturePatternError && error.index === 1,
    );
    assert.equal(session.remove(["right"]), 0);
  } finally {
    session.dispose();
  }
});
