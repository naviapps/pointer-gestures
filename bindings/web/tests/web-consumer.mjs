/** Install and check a generated browser package from an isolated offline npm feed. */
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cp, mkdtemp, mkdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const fixtures = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const typesOnly = args[0] === "--types-only";
if (typesOnly) args.shift();
assert.ok(args.length <= 1, "Usage: web-consumer.mjs [--types-only] [generated-package]");
const directory = resolve(
  args[0] ?? process.env.POINTER_GESTURES_WEB_PACKAGE ?? ".web-build/package",
);
const userEnvironment = {};
for (const name of ["HOME", "CFFIXED_USER_HOME"]) {
  if (process.env[name] !== undefined) userEnvironment[name] = process.env[name];
}
const work = await mkdtemp(join(tmpdir(), "pointer-web-consumer-"));
try {
  const packed = join(work, "packed");
  const consumer = join(work, "consumer");
  const emptyPath = join(work, "empty-path");
  await Promise.all([packed, consumer, emptyPath].map((path) => mkdir(path)));
  const config = join(work, "npmrc");
  await writeFile(config, "");
  const env = {
    ...userEnvironment,
    PATH: process.env.PATH,
    TMPDIR: work,
    npm_config_userconfig: config,
    npm_config_globalconfig: join(work, "global-npmrc"),
    npm_config_cache: join(work, "cache"),
    npm_config_registry: "http://127.0.0.1:9/",
    npm_config_offline: "true",
  };
  await writeFile(env.npm_config_globalconfig, "");
  const npm = process.env.NPM_BIN ?? "npm";
  const run = (command, args, cwd, environment = env) =>
    execFileSync(command, args, {
      cwd,
      env: environment,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "inherit"],
    });
  const [packageInfo] = JSON.parse(
    run(
      npm,
      ["pack", "--ignore-scripts", "--offline", "--json", "--pack-destination", packed],
      directory,
    ),
  );
  assert.equal(packageInfo.name, "@naviapps/pointer-gestures");
  const tarball = join(packed, packageInfo.filename);
  await writeFile(
    join(consumer, "package.json"),
    JSON.stringify({ name: "standalone-pointer-consumer", private: true, type: "module" }),
  );
  run(
    npm,
    ["install", "--ignore-scripts", "--offline", "--no-audit", "--no-fund", "--package-lock=false", tarball],
    consumer,
  );
  const typeFixture = join(fixtures, "typescript");
  await cp(join(typeFixture, "consumer.mts"), join(consumer, "consumer.mts"));
  await cp(join(typeFixture, "tsconfig.json"), join(consumer, "tsconfig.json"));
  const compiler = process.env.TSC_BIN ?? "tsc";
  const compilerVersion = run(compiler, ["--version"], consumer).trim();
  const version = /^Version (\S+)$/.exec(compilerVersion)?.[1];
  assert.ok(version, "TypeScript compiler did not report its version");
  const typeTrace = run(
    compiler,
    ["--project", "tsconfig.json", "--noEmit", "--pretty", "false", "--traceResolution", "--locale", "en"],
    consumer,
  );
  const installed = join(consumer, "node_modules/@naviapps/pointer-gestures");
  const resolvedTypes = await realpath(join(installed, "index.d.ts"));
  assert.ok(typeTrace.includes(
    `Module name '@naviapps/pointer-gestures' was successfully resolved to '${resolvedTypes}'`,
  ), "TypeScript did not resolve the installed package declaration export");
  const typeScript = {
    version,
    compilerVersion,
    compileExitCode: 0,
    compileOnly: true,
    noEmit: true,
    installedPackage: true,
    packageDeclarationResolved: true,
    resolutionTraceSHA256: createHash("sha256").update(typeTrace).digest("hex"),
    expectedMisuseCases: 7,
  };
  console.log("POINTER_TYPESCRIPT_ACCEPTANCE " + JSON.stringify(typeScript));
  let result = { run: false, reason: "compile-only acceptance" };
  // The installed client has no source checkout, compiler path or native-library override.
  const runtime = { ...userEnvironment, PATH: emptyPath, TMPDIR: work };
  if (!typesOnly) {
    await cp(join(fixtures, "package-consumer.mjs"), join(consumer, "consumer.mjs"));
    result = JSON.parse(run(process.execPath, ["consumer.mjs"], consumer, runtime));
  }
  const declaration = run(
    process.execPath,
    [
      "--conditions=types",
      "--input-type=module",
      "--eval",
      'console.log(import.meta.resolve("@naviapps/pointer-gestures"))',
    ],
    consumer,
    runtime,
  ).trim();
  assert.equal(new URL(declaration).pathname.endsWith("/index.d.ts"), true);
  assert.deepEqual(
    await readFile(fileURLToPath(declaration)),
    await readFile(join(directory, "index.d.ts")),
  );
  const receipt = JSON.parse(await readFile(join(installed, "provenance.json"), "utf8"));
  for (const path of ["provenance.json", "SHA256SUMS"]) {
    assert.deepEqual(await readFile(join(installed, path)), await readFile(join(directory, path)));
  }
  for (const [path, checksum] of Object.entries(receipt.outputs)) {
    const bytes = await readFile(join(installed, path));
    assert.equal(createHash("sha256").update(bytes).digest("hex"), checksum, path);
  }
  console.log(JSON.stringify({
    result: "passed",
    sourceRevision: receipt.sourceRevision,
    node: process.version,
    npm: run(npm, ["--version"], consumer).trim(),
    tarballSHA256: createHash("sha256").update(await readFile(tarball)).digest("hex"),
    packedFiles: packageInfo.files.map((entry) => entry.path),
    declarationExport: "index.d.ts resolved, byte-checked and compiled from the installed package",
    typeScript,
    consumer: result,
  }));
} finally {
  await rm(work, { recursive: true, force: true });
}
