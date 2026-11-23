/** Ordinary client: only package-name imports and the explicitly exported Wasm asset. */
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import {
  initSync,
  GesturePatternError,
  GestureSession,
  tracePath,
  patternPath,
} from "@naviapps/pointer-gestures";

const wasm = import.meta.resolve("@naviapps/pointer-gestures/pointer_gestures_wasm_bg.wasm");
const provenance = import.meta.resolve("@naviapps/pointer-gestures/provenance.json");
const receipt = JSON.parse(await readFile(new URL(provenance), "utf8"));
assert.match(receipt.sourceRevision, /^[a-f0-9]{40}$/);
initSync(await readFile(new URL(wasm)));
assert.throws(() => new GestureSession({ maximumPoints: 0 }), RangeError);
const session = new GestureSession({ totalDurationMs: 0 });
let trace;
try {
  assert.throws(
    () => session.register(["right", "right"], 1),
    (error) => error instanceof GesturePatternError &&
      error.reason === "repeated-direction" && error.index === 1,
  );
  session.register(["down", "right"], 42);
  session.begin({ x: 0, y: 0 }, 0);
  session.move({ x: 0, y: 80 }, 20);
  session.move({ x: 100, y: 80 }, 40);
  const result = session.end({ x: 120, y: 80 }, 60);
  assert.equal(result.matchId, 42);
  trace = result.trace;
} finally {
  session.dispose();
}
assert.doesNotThrow(() => session.dispose());
assert.throws(() => session.begin({ x: 0, y: 0 }, 80), /disposed/);
assert.ok(Object.isFrozen(trace));
assert.ok(tracePath(trace).length > 0);
assert.ok(patternPath(["down", "right"], { width: 100, height: 100 }).commands.length > 0);
console.log(JSON.stringify({
  packageNameImport: true,
  explicitWasmSubpath: true,
  provenanceSubpath: true,
  recognitionAndRendering: "passed",
  typedRejectionAndDisposal: "passed",
  copiedTraceAfterDisposal: "passed",
  compilerOrNativeLibraryPath: false,
}));
