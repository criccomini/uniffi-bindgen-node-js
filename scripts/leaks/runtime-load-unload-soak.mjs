import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

import {
  assertNoLeakCandidates,
  forceGc,
  importPackageModule,
  maybePauseForInspection,
  parseProbeArgs,
  printMemorySnapshot,
  printMemorySummary,
  requirePackageDir,
  snapshotMemory,
  warnIfNodeVersionExceeds,
} from "./common.mjs";

const execFileAsync = promisify(execFile);
const options = parseProbeArgs();

if (options.help) {
  console.log(`Usage: node --expose-gc scripts/leaks/runtime-load-unload-soak.mjs [--baseline-only] [--pause] [--batches N] [--ops-per-batch N] [--child-iterations N]

Environment:
  UNIFFI_LEAK_PACKAGE_DIR   Path to a generated basic fixture package produced with --manual-load.
`);
  process.exit(0);
}

warnIfNodeVersionExceeds();

const packageDir = requirePackageDir();
const api = await importPackageModule(packageDir, "index.js");
const ffi = await importPackageModule(packageDir, "fixture-ffi.js");
const callbackRuntime = await importPackageModule(packageDir, "runtime/callbacks.js");
const asyncRuntime = await importPackageModule(packageDir, "runtime/async-rust-call.js");

const { foreignFutureHandleCount } = callbackRuntime;
const { rustFutureHandleCount } = asyncRuntime;
const samples = [];

await runWarmup();

if (options.pause) {
  await maybePauseForInspection(
    true,
    "Load/unload warmup complete. Inspect the live process before measured batches.",
  );
}

for (let batch = 1; batch <= options.batches; batch += 1) {
  for (let index = 0; index < options.opsPerBatch; index += 1) {
    const sampleIndex = batch * options.opsPerBatch + index;
    const bindings = api.load();
    assert.ok(bindings != null);
    assert.equal(ffi.isLoaded(), true);

    if (!options.baselineOnly) {
      const store = new api.Store(createSeed(sampleIndex));
      await store.fetch_async(true);
      store.dispose();
      api.echo_bytes(new Uint8Array([1, sampleIndex % 256, 9]));
    }

    assert.equal(api.unload(), true);
    assert.equal(ffi.isLoaded(), false);
  }

  if (!options.baselineOnly) {
    await runChildCycles(options.childIterations);
  }

  await forceGc();
  assert.equal(foreignFutureHandleCount(), 0);
  assert.equal(rustFutureHandleCount(), 0);
  assert.equal(ffi.isLoaded(), false);

  const snapshot = snapshotMemory(`[load-unload] batch=${batch}`, "load-unload", batch);
  samples.push(snapshot);
  printMemorySnapshot(snapshot);
}

if (options.pause) {
  await maybePauseForInspection(
    true,
    "Load/unload probe complete. Inspect the live process before exit.",
  );
}

printMemorySummary("load/unload", samples);
assertNoLeakCandidates("load/unload", samples);

async function runWarmup() {
  api.load();
  assert.equal(ffi.isLoaded(), true);
  assert.equal(api.unload(), true);
  await forceGc();
  assert.equal(foreignFutureHandleCount(), 0);
  assert.equal(rustFutureHandleCount(), 0);
}

async function runChildCycles(childIterations) {
  await execFileAsync(
    process.execPath,
    [
      "--expose-gc",
      fileURLToPath(new URL("./runtime-load-unload-child.mjs", import.meta.url)),
      packageDir,
      String(childIterations),
      options.baselineOnly ? "baseline" : "workload",
    ],
    {
      cwd: packageDir,
      env: process.env,
    },
  );
}

function createSeed(index) {
  return {
    name: `load-${index}`,
    value: new Uint8Array([1, index % 256]),
    maybe_value: undefined,
    chunks: [new Uint8Array([2, 3])],
  };
}
