/** Real browser acceptance of an installed SDK in an isolated temporary client. */
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cp, mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const fixtures = dirname(fileURLToPath(import.meta.url));
const root = resolve(fixtures, "../../..");
const fixture = join(fixtures, "browser");
const packageDirectory = resolve(process.env.POINTER_GESTURES_WEB_PACKAGE);
const work = resolve(process.env.POINTER_GESTURES_BROWSER_WORK);
const output = resolve(process.env.POINTER_GESTURES_BROWSER_ARTIFACTS);
const tooling = join(work, "tooling");
const consumer = join(work, "consumer");
const expectedSource = process.env.POINTER_GESTURES_WEB_EXPECTED_SOURCE;
const specification = JSON.parse(await readFile(join(fixture, "package.json"), "utf8"));
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
const json = async (path, value) => writeFile(path, JSON.stringify(value, null, 2) + "\n");
const npm = (args, cwd) => execFileSync(process.env.NPM_BIN, args, {
  cwd,
  env: { ...process.env, npm_config_offline: "true", npm_config_registry: "http://127.0.0.1:9/" },
  encoding: "utf8",
  timeout: 60000,
  stdio: ["ignore", "pipe", "inherit"],
});
for (const path of [work, output, packageDirectory]) assert.ok(isAbsolute(path));
assert.ok(relative(root, output).startsWith(".."), "Artifacts must be outside source");
assert.ok(relative(root, work).startsWith(".."), "Temporary client must be outside source");
await mkdir(consumer);
const packed = join(work, "packed");
await mkdir(packed);
const [archive] = JSON.parse(npm([
  "pack", "--ignore-scripts", "--offline", "--json", "--pack-destination", packed,
], packageDirectory));
assert.equal(archive.name, "@naviapps/pointer-gestures");
const tarball = join(packed, archive.filename);
await json(join(consumer, "package.json"), { name: "pointer-browser-consumer", private: true, type: "module" });
npm(["install", "--ignore-scripts", "--offline", "--no-audit", "--no-fund", "--package-lock=false", tarball], consumer);
const installed = join(consumer, "node_modules/@naviapps/pointer-gestures");
const provenance = JSON.parse(await readFile(join(installed, "provenance.json"), "utf8"));
assert.equal(provenance.sourceRevision, expectedSource);
for (const [name, checksum] of Object.entries(provenance.outputs)) {
  const path = resolve(installed, name);
  assert.ok(relative(installed, path) && !relative(installed, path).startsWith(".."));
  assert.equal(sha(await readFile(path)), checksum, name);
  assert.deepEqual(await readFile(path), await readFile(join(packageDirectory, name)));
}
for (const name of ["provenance.json", "SHA256SUMS"]) {
  assert.deepEqual(await readFile(join(installed, name)), await readFile(join(packageDirectory, name)));
}
for (const name of ["index.html", "consumer.mjs"]) await cp(join(fixture, name), join(consumer, name));
const { chromium } = await import(pathToFileURL(join(tooling, "node_modules/playwright/index.mjs")));
const executable = await realpath(chromium.executablePath());
assert.ok(!relative(await realpath(process.env.PLAYWRIGHT_BROWSERS_PATH), executable).startsWith(".."));
const served = [];
const resources = new Map([
  ["/", [join(consumer, "index.html"), "text/html; charset=utf-8"]],
  ["/consumer.mjs", [join(consumer, "consumer.mjs"), "text/javascript"]],
  ...["index.js", "pointer_gestures_wasm.js", "pointer_gestures_wasm_bg.wasm"].map((name) => [
    "/sdk/" + name, [join(installed, name), name.endsWith(".wasm") ? "application/wasm" : "text/javascript"],
  ]),
]);
const server = createServer(async (request, response) => {
  const resource = resources.get(request.url);
  if (!resource || request.method !== "GET") {
    response.writeHead(404).end();
    return;
  }
  try {
    const bytes = await readFile(resource[0]);
    served.push({ path: request.url, sha256: sha(bytes), bytes: bytes.length });
    response.writeHead(200, { "Content-Type": resource[1], "Cache-Control": "no-store" }).end(bytes);
  } catch (error) {
    response.writeHead(500).end(String(error));
  }
});
await new Promise((accept) => server.listen(0, "127.0.0.1", accept));
const origin = `http://127.0.0.1:${server.address().port}`;
let browser;
const results = [];
const receipt = {
  result: "failed",
  fixtureSourceRevision: process.env.POINTER_GESTURES_BROWSER_FIXTURE_SOURCE,
  sdkSourceRevision: provenance.sourceRevision,
  packageProvenanceSHA256: sha(await readFile(join(installed, "provenance.json"))),
  tarballSHA256: sha(await readFile(tarball)),
  fallbackReason: specification.browserGate.fallbackReason,
  playwright: specification.devDependencies.playwright,
  chromiumRevision: specification.browserGate.chromiumRevision,
  browserExecutableSHA256: sha(await readFile(executable)),
  views: results,
};
const timer = setTimeout(() => {
  receipt.timedOut = true;
  receipt.result = "failed";
  void browser?.close().catch(() => {});
}, 120000);

async function snapshot(page) {
  return page.evaluate(() => window.pointerConsumer.snapshot());
}

async function drag(page) {
  const box = await page.locator("#gesture").boundingBox();
  assert.ok(box && box.width > 250 && box.height > 100);
  await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.25);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.65, { steps: 5 });
  await page.mouse.move(box.x + box.width + 12, box.y + box.height * 0.65, { steps: 8 });
  await page.mouse.up();
}

try {
  browser = await chromium.launch({ headless: true, channel: "chromium", timeout: 30000 });
  receipt.chromiumVersion = browser.version();
  assert.equal(browser.version(), specification.browserGate.chromiumVersion);
  for (const [name, viewport] of [["desktop", { width: 1000, height: 760 }], ["narrow", { width: 390, height: 844 }]]) {
    const record = { name, viewport, result: "failed", console: [], pageErrors: [], requestFailures: [], responses: [], screenshots: [] };
    results.push(record);
    const context = await browser.newContext({ viewport, locale: "en-US", serviceWorkers: "block" });
    await context.tracing.start({ screenshots: true, snapshots: true, sources: false });
    const page = await context.newPage();
    page.setDefaultTimeout(15000);
    page.setDefaultNavigationTimeout(15000);
    page.on("console", (message) => {
      if (["error", "warning"].includes(message.type())) record.console.push({ type: message.type(), text: message.text() });
    });
    page.on("pageerror", (error) => record.pageErrors.push(String(error)));
    page.on("requestfailed", (request) => record.requestFailures.push({ url: request.url(), failure: request.failure() }));
    page.on("response", (response) => record.responses.push({ url: response.url(), status: response.status(), contentType: response.headers()["content-type"] }));
    await context.route("**/*", (route) => route.request().url().startsWith(origin + "/") ? route.continue() : route.abort("blockedbyclient"));
    const screenshot = async (stage) => {
      const filename = `${name}-${stage}.png`;
      const bytes = await page.screenshot({ path: join(output, filename), fullPage: false });
      record.screenshots.push({ path: filename, sha256: sha(bytes), bytes: bytes.length });
    };
    try {
      await page.goto(origin + "/", { waitUntil: "networkidle" });
      assert.equal(page.url(), origin + "/");
      assert.equal(await page.title(), "Pointer Gestures standalone browser acceptance");
      await page.waitForFunction(() => window.pointerConsumer?.snapshot().ready === true);
      assert.equal(await page.locator("h1").innerText(), "Pointer Gestures standalone consumer");
      assert.equal(await page.locator("#status").innerText(), "Ready");
      assert.equal(await page.locator("vite-error-overlay, nextjs-portal, #webpack-dev-server-client-overlay, #vite-error-overlay").count(), 0);
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
      record.initial = await snapshot(page);
      assert.equal(record.initial.paintedPixels, 0);
      await screenshot("ready");
      await drag(page);
      await page.waitForFunction(() => window.pointerConsumer.snapshot().lostCapture === 1);
      record.matched = await snapshot(page);
      assert.equal(record.matched.trustedInput, true);
      assert.equal(record.matched.capturedOnDown, true);
      assert.equal(record.matched.gotCapture, 1);
      assert.equal(record.matched.lostCapture, 1);
      assert.equal(record.matched.hasCapture, false);
      assert.equal(record.matched.releasedOutside, true);
      assert.equal(record.matched.activePointer, null);
      assert.equal(record.matched.matches, 1);
      assert.equal(record.matched.update.matchId, 42);
      assert.equal(record.matched.update.active, false);
      assert.equal(record.matched.update.lastSession.outcome, "matched");
      assert.equal(record.matched.update.lastSession.captured, true);
      assert.deepEqual(record.matched.update.trace.directions, ["down", "right"]);
      assert.equal(record.matched.traceFrozen, true);
      assert.ok(record.matched.commandCount > 2 && record.matched.paintedPixels > 200);
      assert.equal(record.matched.status, "Matched 42 · down → right");
      await screenshot("matched");
      await page.getByRole("button", { name: "Dispose session" }).click();
      record.disposed = await snapshot(page);
      assert.equal(record.disposed.disposed, true);
      assert.equal(record.disposed.disposalRejected, true);
      assert.equal(record.disposed.retainedTraceAfterDisposal, true);
      assert.equal(record.disposed.hasCapture, false);
      await drag(page);
      record.afterDisposalInput = await snapshot(page);
      assert.deepEqual(record.afterDisposalInput, record.disposed);
      await screenshot("disposed");
      assert.deepEqual(record.console, []);
      assert.deepEqual(record.pageErrors, []);
      assert.deepEqual(record.requestFailures, []);
      assert.ok(record.responses.every((response) => response.status === 200));
      assert.ok(record.responses.some((response) => response.url === origin + "/sdk/pointer_gestures_wasm_bg.wasm" && response.contentType === "application/wasm"));
      record.checks = { identity: true, nonblank: true, noOverlay: true, console: true, screenshots: true, interaction: true, captureAndRelease: true, recognitionAndCanvas: true, disposal: true };
      record.result = "passed";
    } catch (error) {
      record.error = String(error.stack ?? error);
      await screenshot("failure").catch(() => {});
      throw error;
    } finally {
      try {
        await context.tracing.stop({ path: join(output, `${name}-trace.zip`) });
      } catch (error) {
        record.traceStopError = String(error);
        record.result = "failed";
      }
      await context.close().catch((error) => {
        record.closeError = String(error);
        record.result = "failed";
      });
      await json(join(output, `${name}.json`), record);
      assert.equal(record.result, "passed", JSON.stringify(record));
    }
  }
  assert.equal(receipt.timedOut, undefined, "Browser acceptance deadline exceeded");
  receipt.result = "passed";
} finally {
  clearTimeout(timer);
  await browser?.close().catch((error) => {
    receipt.closeError = String(error);
    receipt.result = "failed";
  });
  await new Promise((accept) => server.close(accept));
  receipt.served = served;
  await json(join(output, "browser-consumer.json"), receipt);
}
assert.equal(receipt.result, "passed");
console.log(JSON.stringify(receipt));
