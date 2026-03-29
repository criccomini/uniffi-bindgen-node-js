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
  requirePackageDir,
  snapshotMemory,
  warnIfNodeVersionExceeds,
} from "./common.mjs";

const options = parseProbeArgs();

if (options.help) {
  console.log(`Usage: node --expose-gc scripts/leaks/runtime-callback-soak.mjs [--baseline-only] [--pause] [--batches N] [--ops-per-batch N]

Environment:
  UNIFFI_LEAK_PACKAGE_DIR   Path to a generated callback fixture package.
`);
  process.exit(0);
}

warnIfNodeVersionExceeds();

const packageDir = requirePackageDir();
const api = await importPackageModule(packageDir, "index.js");
const callbackRuntime = await importPackageModule(packageDir, "runtime/callbacks.js");
const asyncRuntime = await importPackageModule(packageDir, "runtime/async-rust-call.js");

const {
  AsyncLogErrorRejected,
  LogLevel,
  Settings,
  WriteBatch,
  cancel_emit_async,
  emit,
  emit_async,
  emit_async_fallible,
  flush_async,
  init_logging,
  last_message,
} = api;
const { foreignFutureHandleCount } = callbackRuntime;
const { rustFutureHandleCount } = asyncRuntime;

const explicitSamples = [];
const gcOnlySamples = [];

await runWarmup();

if (options.pause) {
  await maybePauseForInspection(
    true,
    "Callback runtime warmup complete. Inspect the live process before measured batches.",
  );
}

for (let batch = 1; batch <= options.batches; batch += 1) {
  if (!options.baselineOnly) {
    await runExplicitCleanupBatch(batch, options.opsPerBatch);
  }
  await forceGc();
  assert.equal(foreignFutureHandleCount(), 0);
  assert.equal(rustFutureHandleCount(), 0);
  const explicitSnapshot = snapshotMemory(
    `[callbacks][explicit] batch=${batch}`,
    "explicit",
    batch,
  );
  explicitSamples.push(explicitSnapshot);
  printMemorySnapshot(explicitSnapshot);

  if (!options.baselineOnly) {
    await runGcOnlyBatch(batch, options.opsPerBatch);
  }
  await forceGc();
  assert.equal(foreignFutureHandleCount(), 0);
  assert.equal(rustFutureHandleCount(), 0);
  const gcSnapshot = snapshotMemory(
    `[callbacks][gc-only] batch=${batch}`,
    "gc-only",
    batch,
  );
  gcOnlySamples.push(gcSnapshot);
  printMemorySnapshot(gcSnapshot);
}

if (options.pause) {
  await maybePauseForInspection(
    true,
    "Callback runtime probe complete. Inspect the live process before exit.",
  );
}

printMemorySummary("callbacks explicit", explicitSamples);
printMemorySummary("callbacks gc-only", gcOnlySamples);
assertNoLeakCandidates("callbacks explicit", explicitSamples);
assertNoLeakCandidates("callbacks gc-only", gcOnlySamples);
assertPlateauClose(
  "callbacks explicit vs gc-only",
  explicitSamples,
  gcOnlySamples,
  "heapUsed",
  8 * MiB,
);
assertPlateauClose(
  "callbacks explicit vs gc-only",
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

  await runExplicitCleanupBatch(0, Math.min(25, options.opsPerBatch));
  await runGcOnlyBatch(0, Math.min(25, options.opsPerBatch));
  await forceGc();
  assert.equal(foreignFutureHandleCount(), 0);
  assert.equal(rustFutureHandleCount(), 0);
}

async function runExplicitCleanupBatch(batch, opsPerBatch) {
  for (let index = 0; index < opsPerBatch; index += 1) {
    const sampleIndex = batch * opsPerBatch + index;

    const messages = [];
    const syncSink = {
      write(message) {
        messages.push(message);
      },
      latest() {
        return messages.at(-1);
      },
    };

    emit(syncSink, `message-${sampleIndex}`);
    assert.equal(last_message(syncSink), `message-${sampleIndex}`);

    const records = [];
    init_logging(LogLevel.Info, {
      log(record) {
        records.push(record);
      },
    });
    assert.equal(records.length, 1);
    init_logging(LogLevel.Info, undefined);

    const settings = Settings.default();
    settings.set("writer.cache_size", String(1_024 + (sampleIndex % 32)));
    settings.set("logging.level", "\"debug\"");
    settings.to_json_string();

    const batchObject = new WriteBatch();
    batchObject.put(Buffer.from([1, sampleIndex % 256]), Buffer.from([3, 4]));
    batchObject.delete(Buffer.from([5, 6]));
    batchObject.operation_count();

    const successSink = {
      write(message) {
        return Promise.resolve(`ok:${message}`);
      },
      write_fallible(message) {
        return Promise.resolve(`fallible:${message}`);
      },
      flush() {
        return Promise.resolve();
      },
    };
    await emit_async(successSink, `async-${sampleIndex}`);
    await flush_async(successSink);

    const typedSink = {
      write(message) {
        return Promise.resolve(message);
      },
      write_fallible(message) {
        return Promise.reject(new AsyncLogErrorRejected(`typed:${message}`));
      },
      flush() {
        return Promise.resolve();
      },
    };
    await assert.rejects(emit_async_fallible(typedSink, `typed-${sampleIndex}`));

    const unexpectedSink = {
      write(message) {
        return Promise.resolve(message);
      },
      write_fallible(message) {
        return Promise.reject(`unexpected:${message}`);
      },
      flush() {
        return Promise.resolve();
      },
    };
    await assert.rejects(
      emit_async_fallible(unexpectedSink, `unexpected-${sampleIndex}`),
    );

    const deferred = createDeferred();
    const cancellationSink = {
      write(message) {
        return deferred.promise.then(() => `cancelled:${message}`);
      },
      write_fallible(message) {
        return Promise.resolve(message);
      },
      flush() {
        return Promise.resolve();
      },
    };
    cancel_emit_async(cancellationSink, `cancel-${sampleIndex}`);
    deferred.resolve();
    await deferred.promise;
    await Promise.resolve();

    batchObject.dispose();
    settings.dispose();
  }
}

async function runGcOnlyBatch(batch, opsPerBatch) {
  const retained = [];

  for (let index = 0; index < opsPerBatch; index += 1) {
    const sampleIndex = batch * opsPerBatch + index;
    const settings = Settings.default();
    settings.set("writer.cache_size", String(2_048 + (sampleIndex % 32)));
    settings.to_json_string();

    const batchObject = new WriteBatch();
    batchObject.put(Buffer.from([1, 2]), Buffer.from([3, sampleIndex % 256]));
    batchObject.operation_count();

    emit(
      {
        write() {},
        latest() {
          return undefined;
        },
      },
      `sync-${sampleIndex}`,
    );

    await emit_async(
      {
        write(message) {
          return Promise.resolve(`gc:${message}`);
        },
        write_fallible(message) {
          return Promise.resolve(message);
        },
        flush() {
          return Promise.resolve();
        },
      },
      `gc-${sampleIndex}`,
    );

    retained.push(settings, batchObject);
  }

  retained.length = 0;
}

function createDeferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}
