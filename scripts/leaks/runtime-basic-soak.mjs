import assert from "node:assert/strict";

import {
  MiB,
  assertNoLeakCandidates,
  assertPlateauClose,
  forceGc,
  importPackageModule,
  maybePauseForInspection,
  parseProbeArgs,
  printMemorySnapshot,
  printMemorySummary,
  readRequiredString,
  requirePackageDir,
  snapshotMemory,
  warnIfNodeVersionExceeds,
} from "./common.mjs";

const SCENARIOS = new Set(["full", "bytes", "objects", "async"]);
const BYTE_CASES = new Set(["all", "echo-bytes", "echo-record", "echo-byte-map", "temporal"]);
const ASYNC_CASES = new Set(["all", "store-fetch", "reader-build", "reader-label"]);

const options = parseProbeArgs(process.argv.slice(2), {
  "--scenario": (parsedOptions, argv, index) => {
    const scenario = readRequiredString("--scenario", argv, index + 1);
    if (!SCENARIOS.has(scenario)) {
      throw new Error(
        `--scenario must be one of ${Array.from(SCENARIOS).join(", ")}, got ${JSON.stringify(scenario)}.`,
      );
    }
    parsedOptions.scenario = scenario;
    return index + 1;
  },
  "--case": (parsedOptions, argv, index) => {
    parsedOptions.case = readRequiredString("--case", argv, index + 1);
    return index + 1;
  },
});

const scenario = options.scenario ?? "full";
const selectedCase = normalizeSelectedCase(scenario, options.case ?? "all");
const probeLabel = selectedCase === "all"
  ? `basic ${scenario}`
  : `basic ${scenario} ${selectedCase}`;
const probeLabelPath = selectedCase === "all"
  ? `[basic][${scenario}]`
  : `[basic][${scenario}][${selectedCase}]`;

if (options.help) {
  console.log(`Usage: node --expose-gc scripts/leaks/runtime-basic-soak.mjs [--scenario full|bytes|objects|async] [--case CASE] [--baseline-only] [--pause] [--batches N] [--ops-per-batch N]

Environment:
  UNIFFI_LEAK_PACKAGE_DIR   Path to a generated basic fixture package.

Cases:
  bytes: all, echo-bytes, echo-record, echo-byte-map, temporal
  async: all, store-fetch, reader-build, reader-label
`);
  process.exit(0);
}

warnIfNodeVersionExceeds();

const packageDir = requirePackageDir();
const api = await importPackageModule(packageDir, "index.js");
const asyncRuntime = await importPackageModule(packageDir, "runtime/async-rust-call.js");

const {
  Config,
  ReaderBuilder,
  Store,
  echo_byte_map,
  echo_bytes,
  echo_duration,
  echo_record,
  echo_timestamp,
} = api;
const { rustFutureHandleCount } = asyncRuntime;

const explicitSamples = [];
const gcOnlySamples = [];

await runWarmup();

if (options.pause) {
  await maybePauseForInspection(
    true,
    `Basic runtime warmup complete for ${probeLabel}. Inspect the live process before measured batches.`,
  );
}

for (let batch = 1; batch <= options.batches; batch += 1) {
  if (!options.baselineOnly) {
    await runExplicitCleanupBatch(batch, options.opsPerBatch);
  }
  await forceGc();
  assert.equal(rustFutureHandleCount(), 0);
  const explicitSnapshot = snapshotMemory(
    `${probeLabelPath}[explicit] batch=${batch}`,
    "explicit",
    batch,
  );
  explicitSamples.push(explicitSnapshot);
  printMemorySnapshot(explicitSnapshot);

  if (!options.baselineOnly) {
    await runGcOnlyBatch(batch, options.opsPerBatch);
  }
  await forceGc();
  assert.equal(rustFutureHandleCount(), 0);
  const gcSnapshot = snapshotMemory(
    `${probeLabelPath}[gc-only] batch=${batch}`,
    "gc-only",
    batch,
  );
  gcOnlySamples.push(gcSnapshot);
  printMemorySnapshot(gcSnapshot);
}

if (options.pause) {
  await maybePauseForInspection(
    true,
    `Basic runtime probe complete for ${probeLabel}. Inspect the live process before exit.`,
  );
}

printMemorySummary(`${probeLabel} explicit`, explicitSamples);
printMemorySummary(`${probeLabel} gc-only`, gcOnlySamples);
assertNoLeakCandidates(`${probeLabel} explicit`, explicitSamples);
assertNoLeakCandidates(`${probeLabel} gc-only`, gcOnlySamples);
assertPlateauClose(
  `${probeLabel} explicit vs gc-only`,
  explicitSamples,
  gcOnlySamples,
  "heapUsed",
  8 * MiB,
);
assertPlateauClose(
  `${probeLabel} explicit vs gc-only`,
  explicitSamples,
  gcOnlySamples,
  "external",
  8 * MiB,
);

async function runWarmup() {
  if (options.baselineOnly) {
    await forceGc();
    return;
  }

  await runExplicitCleanupBatch(0, Math.min(50, options.opsPerBatch));
  await runGcOnlyBatch(0, Math.min(50, options.opsPerBatch));
  await forceGc();
  assert.equal(rustFutureHandleCount(), 0);
}

async function runExplicitCleanupBatch(batch, opsPerBatch) {
  for (let index = 0; index < opsPerBatch; index += 1) {
    const sampleIndex = batch * opsPerBatch + index;
    if (scenario === "full" || scenario === "bytes") {
      runBytesCase(sampleIndex, selectedCase);
    }
    if (scenario === "full" || scenario === "objects") {
      runObjectLifecycle(sampleIndex);
    }
    if (scenario === "full" || scenario === "async") {
      await runAsyncCase(sampleIndex, selectedCase);
    }
  }
}

async function runGcOnlyBatch(batch, opsPerBatch) {
  const retained = [];

  for (let index = 0; index < opsPerBatch; index += 1) {
    const sampleIndex = batch * opsPerBatch + index;
    if (scenario === "full" || scenario === "bytes") {
      retained.push(runBytesCase(sampleIndex, selectedCase));
    }
    if (scenario === "full" || scenario === "objects") {
      retained.push(runRetainedObjectLifecycle(sampleIndex));
    }
    if (scenario === "full" || scenario === "async") {
      retained.push(...(await runRetainedAsyncCase(sampleIndex, selectedCase)));
    }
  }

  retained.length = 0;
}

function runBytesCase(sampleIndex, caseName) {
  switch (caseName) {
    case "all":
      return Object.freeze({
        echoedBytes: runEchoBytes(sampleIndex),
        echoedRecord: runEchoRecord(sampleIndex),
        echoedMap: runEchoByteMap(sampleIndex),
        temporal: runTemporalRoundTrip(sampleIndex),
      });

    case "echo-bytes":
      return runEchoBytes(sampleIndex);

    case "echo-record":
      return runEchoRecord(sampleIndex);

    case "echo-byte-map":
      return runEchoByteMap(sampleIndex);

    case "temporal":
      return runTemporalRoundTrip(sampleIndex);

    default:
      throw new Error(`Unhandled bytes case ${JSON.stringify(caseName)}.`);
  }
}

function runEchoBytes(sampleIndex) {
  const echoedBytes = echo_bytes(new Uint8Array([sampleIndex % 256, 7, 8, 9]));
  assert.equal(echoedBytes[0], sampleIndex % 256);
  return echoedBytes;
}

function runEchoRecord(sampleIndex) {
  const seed = createSeed(sampleIndex);
  const echoedRecord = echo_record(seed);
  assert.equal(echoedRecord.name, seed.name);
  return echoedRecord;
}

function runEchoByteMap(sampleIndex) {
  const echoedMap = echo_byte_map(
    new Map([
      ["alpha", new Uint8Array([1, 2, 3])],
      ["beta", new Uint8Array([4, sampleIndex % 256])],
    ]),
  );
  assert.equal(echoedMap.size, 2);
  return echoedMap;
}

function runTemporalRoundTrip(sampleIndex) {
  const echoedWhen = echo_timestamp(new Date("2024-01-02T03:04:05.678Z"));
  assert.ok(echoedWhen instanceof Date);

  const echoedDelay = echo_duration(1_500 + (sampleIndex % 10));
  assert.equal(typeof echoedDelay, "number");

  return Object.freeze({
    echoedDelay,
    echoedWhen,
  });
}

function runObjectLifecycle(sampleIndex) {
  const seed = createSeed(sampleIndex);
  const store = new Store(seed);
  const current = store.current();
  assert.equal(current.name, seed.name);
  store.replace(new Uint8Array([9, 8, sampleIndex % 256]));
  assert.ok(Object.values(api.Flavor).includes(store.flavor()));
  const scanResult = store.inspect(true);
  assert.equal(scanResult.tag, "Hit");

  const config = Config.from_json("ok");
  assert.equal(config.value(), "ok");

  if (sampleIndex % 100 === 0) {
    assert.throws(() => Config.from_json("not-json"));
  }

  config.dispose();
  store.dispose();
}

async function runAsyncCase(sampleIndex, caseName) {
  switch (caseName) {
    case "all":
      await runStoreFetchAsync(sampleIndex);
      await runReaderBuildAsync(sampleIndex);
      await runReaderLabelAsync(sampleIndex);
      return;

    case "store-fetch":
      await runStoreFetchAsync(sampleIndex);
      return;

    case "reader-build":
      await runReaderBuildAsync(sampleIndex);
      return;

    case "reader-label":
      await runReaderLabelAsync(sampleIndex);
      return;

    default:
      throw new Error(`Unhandled async case ${JSON.stringify(caseName)}.`);
  }
}

async function runStoreFetchAsync(sampleIndex) {
  const seed = createSeed(sampleIndex);
  const store = new Store(seed);
  const asyncRecord = await store.fetch_async(true);
  assert.equal(asyncRecord.name, seed.name);
  store.dispose();
}

async function runReaderBuildAsync(sampleIndex) {
  const builder = new ReaderBuilder(true);
  const reader = await builder.build();
  assert.equal(reader.label(), "ready");

  if (sampleIndex % 100 === 0) {
    const rejectedBuilder = new ReaderBuilder(false);
    await assert.rejects(rejectedBuilder.build());
    rejectedBuilder.dispose();
  }

  reader.dispose();
  builder.dispose();
}

async function runReaderLabelAsync(_sampleIndex) {
  const builder = new ReaderBuilder(true);
  const reader = await builder.build();
  assert.equal(await reader.label_async(), "ready");
  reader.dispose();
  builder.dispose();
}

function runRetainedObjectLifecycle(sampleIndex) {
  const seed = createSeed(sampleIndex);
  const store = new Store(seed);
  store.current();
  store.replace(new Uint8Array([9, 8, sampleIndex % 256]));
  store.inspect(true);

  const config = Config.from_json("ok");
  config.value();

  return Object.freeze({
    config,
    store,
  });
}

async function runRetainedAsyncCase(sampleIndex, caseName) {
  switch (caseName) {
    case "all":
      return [
        ...(await runRetainedStoreFetchAsync(sampleIndex)),
        ...(await runRetainedReaderBuildAsync(sampleIndex)),
        ...(await runRetainedReaderLabelAsync(sampleIndex)),
      ];

    case "store-fetch":
      return runRetainedStoreFetchAsync(sampleIndex);

    case "reader-build":
      return runRetainedReaderBuildAsync(sampleIndex);

    case "reader-label":
      return runRetainedReaderLabelAsync(sampleIndex);

    default:
      throw new Error(`Unhandled async case ${JSON.stringify(caseName)}.`);
  }
}

async function runRetainedStoreFetchAsync(sampleIndex) {
  const seed = createSeed(sampleIndex);
  const store = new Store(seed);
  const asyncRecord = await store.fetch_async(true);

  return [
    Object.freeze({
      asyncRecord,
      store,
    }),
  ];
}

async function runRetainedReaderBuildAsync(_sampleIndex) {
  const builder = new ReaderBuilder(true);
  const reader = await builder.build();
  reader.label();

  return [
    Object.freeze({
      builder,
      reader,
    }),
  ];
}

async function runRetainedReaderLabelAsync(_sampleIndex) {
  const builder = new ReaderBuilder(true);
  const reader = await builder.build();
  const label = await reader.label_async();
  assert.equal(label, "ready");

  return [
    Object.freeze({
      builder,
      label,
      reader,
    }),
  ];
}

function normalizeSelectedCase(activeScenario, rawCase) {
  if (activeScenario === "full" || activeScenario === "objects") {
    if (rawCase !== "all") {
      throw new Error(
        `--case is only supported with --scenario bytes or --scenario async, got scenario=${JSON.stringify(activeScenario)}.`,
      );
    }
    return "all";
  }

  const allowedCases = activeScenario === "bytes"
    ? BYTE_CASES
    : ASYNC_CASES;
  if (!allowedCases.has(rawCase)) {
    throw new Error(
      `--case must be one of ${Array.from(allowedCases).join(", ")} for scenario=${JSON.stringify(activeScenario)}, got ${JSON.stringify(rawCase)}.`,
    );
  }
  return rawCase;
}

function createSeed(index) {
  return {
    name: `seed-${index}`,
    value: new Uint8Array([1, 2, index % 256]),
    maybe_value: undefined,
    chunks: [
      new Uint8Array([3, 4]),
      new Uint8Array([5, index % 256]),
    ],
  };
}
