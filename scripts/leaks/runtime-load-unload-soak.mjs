import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtempSync, rmSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

import {
  assertNoLeakCandidates,
  forceGc,
  importPackageModule,
  maybePauseForInspection,
  parseProbeArgs,
  readRequiredString,
  printMemorySnapshot,
  printMemorySummary,
  requirePackageDir,
  snapshotMemory,
  warnIfNodeVersionExceeds,
} from "./common.mjs";

const execFileAsync = promisify(execFile);
const LOAD_UNLOAD_CASES = new Set(["store-fetch", "reader-build", "alias-reload"]);
const options = parseProbeArgs(process.argv.slice(2), {
  "--case": (parsedOptions, argv, index) => {
    const selectedCase = readRequiredString("--case", argv, index + 1);
    if (!LOAD_UNLOAD_CASES.has(selectedCase)) {
      throw new Error(
        `--case must be one of ${Array.from(LOAD_UNLOAD_CASES).join(", ")}, got ${JSON.stringify(selectedCase)}.`,
      );
    }
    parsedOptions.case = selectedCase;
    return index + 1;
  },
});
const selectedCase = options.case ?? "store-fetch";

if (options.help) {
  console.log(`Usage: node --expose-gc scripts/leaks/runtime-load-unload-soak.mjs [--case store-fetch|reader-build|alias-reload] [--baseline-only] [--pause] [--batches N] [--ops-per-batch N] [--child-iterations N]

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
const aliasSourceLibraryPath = resolve(packageDir, "libfixture_basic.dylib");
const aliasDirectory =
  selectedCase === "alias-reload"
    ? mkdtempSync(join(tmpdir(), "uniffi-load-alias-"))
    : null;

const { foreignFutureHandleCount } = callbackRuntime;
const { rustFutureHandleCount } = asyncRuntime;
const samples = [];

try {
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
      const loadPath = createLoadPath(sampleIndex);

      try {
        const bindings = api.load(loadPath);
        assert.ok(bindings != null);
        assert.equal(ffi.isLoaded(), true);

        if (!options.baselineOnly) {
          await runWorkloadCase(sampleIndex);
        }
      } finally {
        if (ffi.isLoaded()) {
          assert.equal(api.unload(), true);
          assert.equal(ffi.isLoaded(), false);
        }
        cleanupLoadPath(loadPath);
      }
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
} finally {
  if (aliasDirectory != null) {
    rmSync(aliasDirectory, { recursive: true, force: true });
  }
}

async function runWarmup() {
  const loadPath = createLoadPath("warmup");
  try {
    api.load(loadPath);
    assert.equal(ffi.isLoaded(), true);
    assert.equal(api.unload(), true);
  } finally {
    cleanupLoadPath(loadPath);
  }
  await forceGc();
  assert.equal(foreignFutureHandleCount(), 0);
  assert.equal(rustFutureHandleCount(), 0);
}

async function runChildCycles(childIterations) {
  if (selectedCase === "alias-reload") {
    return;
  }

  await execFileAsync(
    process.execPath,
    [
      "--expose-gc",
      fileURLToPath(new URL("./runtime-load-unload-child.mjs", import.meta.url)),
      packageDir,
      String(childIterations),
      options.baselineOnly ? "baseline" : "workload",
      selectedCase,
    ],
    {
      cwd: packageDir,
      env: process.env,
    },
  );
}

async function runWorkloadCase(sampleIndex) {
  switch (selectedCase) {
    case "store-fetch": {
      const store = new api.Store(createSeed(sampleIndex));
      await store.fetch_async(true);
      store.dispose();
      api.echo_bytes(new Uint8Array([1, sampleIndex % 256, 9]));
      return;
    }

    case "reader-build": {
      const builder = new api.ReaderBuilder(true);
      const reader = await builder.build();
      assert.equal(reader.label(), "ready");
      reader.dispose();
      builder.dispose();
      return;
    }

    case "alias-reload":
      api.echo_bytes(new Uint8Array([1, sampleIndex % 256, 9]));
      return;

    default:
      throw new Error(`Unhandled load/unload case ${JSON.stringify(selectedCase)}.`);
  }
}

function createLoadPath(sampleIndex) {
  if (selectedCase !== "alias-reload") {
    return undefined;
  }

  const aliasPath = join(aliasDirectory, `alias-${String(sampleIndex)}.dylib`);
  symlinkSync(aliasSourceLibraryPath, aliasPath);
  return aliasPath;
}

function cleanupLoadPath(loadPath) {
  if (selectedCase !== "alias-reload" || loadPath == null) {
    return;
  }

  rmSync(loadPath, { force: true });
}

function createSeed(index) {
  return {
    name: `load-${index}`,
    value: new Uint8Array([1, index % 256]),
    maybe_value: undefined,
    chunks: [new Uint8Array([2, 3])],
  };
}
