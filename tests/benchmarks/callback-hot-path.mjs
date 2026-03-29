import assert from "node:assert/strict";

import { Bench } from "tinybench";

import {
  AsyncLogErrorRejected,
  LogLevel,
  Settings,
  WriteBatch,
  emit,
  emit_async,
  emit_async_fallible,
  flush_async,
  init_logging,
  last_message,
} from "../index.js";
import { foreignFutureHandleCount } from "../runtime/callbacks.js";
import { blackBox, hotPathBenchOptions, printBenchResults } from "./common.mjs";

const SMALL_BYTES = new Uint8Array([1, 2, 3, 4]);
const LARGE_BYTES = Uint8Array.from({ length: 1024 }, (_, index) => index % 256);

class SyncSink {
  constructor() {
    this.messages = [];
  }

  reset(messages = []) {
    this.messages = [...messages];
  }

  write(message) {
    this.messages.push(message);
  }

  latest() {
    return this.messages.at(-1);
  }
}

class LogCollector {
  constructor() {
    this.records = [];
  }

  reset() {
    this.records = [];
  }

  log(record) {
    this.records.push(record);
  }
}

const bench = new Bench({
  ...hotPathBenchOptions("Generated Callback Package Hot Paths"),
  setup() {
    assert.equal(foreignFutureHandleCount(), 0);
  },
  teardown() {
    assert.equal(foreignFutureHandleCount(), 0);
  },
});

const syncSink = new SyncSink();
bench.add(
  "emit(sync sink)",
  () => {
    emit(syncSink, "message");
    blackBox(syncSink.messages.length);
  },
  {
    beforeEach() {
      syncSink.reset();
    },
  },
);

const latestSink = new SyncSink();
bench.add(
  "last_message(sync sink)",
  () => {
    blackBox(last_message(latestSink));
  },
  {
    beforeEach() {
      latestSink.reset(["first", "second"]);
    },
  },
);

const collector = new LogCollector();
bench.add(
  "init_logging(collector)",
  () => {
    init_logging(LogLevel.Info, collector);
    blackBox(collector.records.length);
  },
  {
    beforeEach() {
      collector.reset();
    },
  },
);

let settingsForSet;
bench.add(
  "Settings.set(...)",
  () => {
    settingsForSet.set("writer.cache_size", "1024");
    blackBox(settingsForSet);
  },
  {
    beforeEach() {
      settingsForSet = Settings.default();
    },
  },
);

let settingsForSerialize;
bench.add(
  "Settings.to_json_string()",
  () => {
    blackBox(settingsForSerialize.to_json_string());
  },
  {
    beforeEach() {
      settingsForSerialize = Settings.default();
      settingsForSerialize.set("writer.cache_size", "1024");
      settingsForSerialize.set("writer.enabled", "true");
      settingsForSerialize.set("logging.level", "\"debug\"");
    },
  },
);

let writeBatchForPut;
bench.add(
  "WriteBatch.put(bytes, bytes)",
  () => {
    writeBatchForPut.put(SMALL_BYTES, LARGE_BYTES);
    blackBox(writeBatchForPut.operation_count());
  },
  {
    beforeEach() {
      writeBatchForPut = new WriteBatch();
    },
  },
);

let writeBatchForDelete;
bench.add(
  "WriteBatch.delete(bytes)",
  () => {
    writeBatchForDelete.delete(SMALL_BYTES);
    blackBox(writeBatchForDelete.operation_count());
  },
  {
    beforeEach() {
      writeBatchForDelete = new WriteBatch();
    },
  },
);

const resolvingAsyncSink = {
  write(message) {
    return Promise.resolve(message);
  },
  write_fallible(message) {
    return Promise.resolve(message);
  },
  flush() {
    return Promise.resolve();
  },
};

bench.add(
  "emit_async(async sink)",
  async () => {
    blackBox(await emit_async(resolvingAsyncSink, "async message"));
  },
);
bench.add(
  "flush_async(async sink)",
  async () => {
    await flush_async(resolvingAsyncSink);
    blackBox(true);
  },
);

const rejectingAsyncSink = {
  write(message) {
    return Promise.resolve(message);
  },
  write_fallible() {
    return Promise.reject(new AsyncLogErrorRejected("typed rejection"));
  },
  flush() {
    return Promise.resolve();
  },
};

bench.add(
  "emit_async_fallible(typed rejection)",
  async () => {
    const error = await emit_async_fallible(rejectingAsyncSink, "typed rejection").catch(
      (rejection) => {
        if (!(rejection instanceof AsyncLogErrorRejected)) {
          throw rejection;
        }
        return rejection;
      },
    );
    blackBox(error);
  },
);

await bench.run();
printBenchResults(bench);
