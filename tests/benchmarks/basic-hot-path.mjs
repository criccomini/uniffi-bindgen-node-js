import { Bench } from "tinybench";

import {
  Config,
  FixtureErrorMissing,
  ReaderBuilder,
  Store,
  echo_byte_map,
  echo_bytes,
  echo_record,
} from "../index.js";
import { blackBox, hotPathBenchOptions, maybeGc, printBenchResults } from "./common.mjs";

const SMALL_BYTES = new Uint8Array([1, 2, 3, 4]);
const LARGE_BYTES = Uint8Array.from({ length: 4096 }, (_, index) => index % 256);
const SMALL_BUFFER = Buffer.from(SMALL_BYTES);
const SEED_RECORD = {
  name: "seed",
  value: SMALL_BYTES,
  maybe_value: LARGE_BYTES.subarray(0, 32),
  chunks: [SMALL_BYTES, LARGE_BYTES.subarray(0, 64)],
};
const BYTE_MAP = new Map([
  ["alpha", SMALL_BYTES],
  ["beta", LARGE_BYTES.subarray(0, 128)],
]);

const bench = new Bench({
  ...hotPathBenchOptions("Generated Basic Package Hot Paths"),
  setup() {
    maybeGc();
  },
  teardown() {
    maybeGc();
  },
});

let storeForCurrent;
bench.add(
  "echo_bytes(small)",
  () => {
    blackBox(echo_bytes(SMALL_BYTES));
  },
);
bench.add(
  "echo_bytes(large)",
  () => {
    blackBox(echo_bytes(LARGE_BYTES));
  },
);
bench.add(
  "echo_bytes(Buffer)",
  () => {
    blackBox(echo_bytes(SMALL_BUFFER));
  },
);
bench.add(
  "echo_record(record)",
  () => {
    blackBox(echo_record(SEED_RECORD));
  },
);
bench.add(
  "echo_byte_map(map)",
  () => {
    blackBox(echo_byte_map(BYTE_MAP));
  },
);
bench.add(
  "new Store(seed)",
  () => {
    blackBox(new Store(SEED_RECORD));
  },
);
bench.add(
  "store.current()",
  () => {
    blackBox(storeForCurrent.current());
  },
  {
    beforeAll() {
      storeForCurrent = new Store(SEED_RECORD);
    },
  },
);

let storeForReplace;
bench.add(
  "store.replace(bytes)",
  () => {
    blackBox(storeForReplace.replace(LARGE_BYTES));
  },
  {
    beforeEach() {
      storeForReplace = new Store(SEED_RECORD);
    },
  },
);

let storeForFlavor;
bench.add(
  "store.flavor()",
  () => {
    blackBox(storeForFlavor.flavor());
  },
  {
    beforeAll() {
      storeForFlavor = new Store(SEED_RECORD);
    },
  },
);

let storeForInspect;
bench.add(
  "store.inspect(true)",
  () => {
    blackBox(storeForInspect.inspect(true));
  },
  {
    beforeAll() {
      storeForInspect = new Store(SEED_RECORD);
    },
  },
);

let storeForFetchAsync;
bench.add(
  "store.fetch_async(true)",
  async () => {
    blackBox(await storeForFetchAsync.fetch_async(true));
  },
  {
    beforeAll() {
      storeForFetchAsync = new Store(SEED_RECORD);
    },
  },
);

let storeForRequireValue;
bench.add(
  "store.require_value(false)",
  () => {
    const error = (() => {
      try {
        storeForRequireValue.require_value(false);
      } catch (caught) {
        if (!(caught instanceof FixtureErrorMissing)) {
          throw caught;
        }
        return caught;
      }
      throw new Error("expected store.require_value(false) to throw");
    })();
    blackBox(error);
  },
  {
    beforeAll() {
      storeForRequireValue = new Store(SEED_RECORD);
    },
  },
);

bench.add(
  "Config.from_json(\"ok\")",
  () => {
    blackBox(Config.from_json("ok"));
  },
);

let readerBuilder;
bench.add(
  "ReaderBuilder(true).build()",
  async () => {
    blackBox(await readerBuilder.build());
  },
  {
    beforeEach() {
      readerBuilder = new ReaderBuilder(true);
    },
  },
);

let reader;
bench.add(
  "reader.label()",
  () => {
    blackBox(reader.label());
  },
  {
    async beforeAll() {
      reader = await new ReaderBuilder(true).build();
    },
  },
);
bench.add(
  "reader.label_async()",
  async () => {
    blackBox(await reader.label_async());
  },
  {
    async beforeAll() {
      reader = await new ReaderBuilder(true).build();
    },
  },
);

await bench.run();
printBenchResults(bench);
