mod support;

use self::support::{
    generate_fixture_package, install_fixture_package_dependencies, remove_dir_all,
    run_typescript_check,
};

#[test]
fn typechecks_generated_basic_fixture_package_declarations() {
    let generated = generate_fixture_package("basic");
    let package_dir = &generated.package_dir;

    install_fixture_package_dependencies(package_dir);
    run_typescript_check(
        package_dir,
        "smoke.ts",
        r#"
import {
  Config,
  FixtureError,
  FixtureErrorInvalidState,
  FixtureErrorMissing,
  FixtureErrorParse,
  Flavor,
  Reader,
  ReaderBuilder,
  ScanResult,
  Store,
  echo_byte_map,
  echo_bytes,
  echo_duration,
  echo_record,
  echo_temporal_record,
  echo_timestamp,
  type BlobRecord,
  type TemporalRecord,
} from "./index.js";
import {
  type FfiBindings,
  type FfiIntegrity,
  type FfiMetadata,
  ffiIntegrity,
  ffiMetadata,
  getFfiBindings,
  isLoaded,
  load,
  unload,
} from "./fixture-ffi.js";
import {
  ChecksumMismatchError,
  ContractVersionMismatchError,
  UniffiInternalError,
  type UniffiErrorOptions,
} from "./runtime/errors.js";

const seed: BlobRecord = {
  name: "seed",
  value: new Uint8Array([1, 2]),
  maybe_value: undefined,
  chunks: [new Uint8Array([3]), new Uint8Array([4, 5])],
};

const store = new Store(seed);
const current: BlobRecord = store.current();
const when = new Date("2024-01-02T03:04:05.678Z");
const echoedWhen: Date = echo_timestamp(when);
const echoedDelayMs: number = echo_duration(1_500);
const scheduled: TemporalRecord = store.schedule(when, echoedDelayMs);
const scheduledDelayMs: number = scheduled.delay_ms;
const scheduledDelays: Array<number> = scheduled.delays_ms;
const echoedTemporalRecord: TemporalRecord = echo_temporal_record({
  when,
  delay_ms: echoedDelayMs,
  maybe_when: when,
  delays_ms: [echoedDelayMs],
  reminders: new Map<string, Date>([["seed", when]]),
});
declare const temporalRecord: TemporalRecord;
const optionalWhen: Date | undefined = temporalRecord.maybe_when;
const reminders: Map<string, Date> = temporalRecord.reminders;
const flavor: Flavor = store.flavor();
const scanResult: ScanResult = store.inspect(true);
const echoedMap: Map<string, Uint8Array> = echo_byte_map(
  new Map<string, Uint8Array>([["alpha", new Uint8Array([6, 7, 8])]]),
);
const echoedBytes: Uint8Array = echo_bytes(new Uint8Array([7, 8, 9]));
const echoedRecord: BlobRecord = echo_record(seed);
const asyncRecord: Promise<BlobRecord> = store.fetch_async(true);
const config: Config = Config.from_json("ok");
const configValue: string = config.value();
const readerBuilder = new ReaderBuilder(true);
const asyncReader: Promise<Reader> = readerBuilder.build();
const fixtureError: FixtureError = new FixtureErrorMissing();
const missingError = new FixtureErrorMissing();
const missingTag: "Missing" = missingError.tag;
const invalidStateError: FixtureErrorInvalidState = new FixtureErrorInvalidState("bad state");
const invalidStateTag: "InvalidState" = invalidStateError.tag;
const invalidStateMessage: string = invalidStateError.message;
const parseError: FixtureErrorParse = new FixtureErrorParse("bad parse");
const parseTag: "Parse" = parseError.tag;
const parseMessage: string = parseError.message;
const ffiMetadataValue: Readonly<FfiMetadata> = ffiMetadata;
const ffiIntegrityValue: Readonly<FfiIntegrity> = ffiIntegrity;
const maybeBindings: Readonly<FfiBindings> = isLoaded() ? getFfiBindings() : load();
const stillLoaded: boolean = isLoaded();
const didUnload: boolean = unload();
const errorOptions: UniffiErrorOptions = {
  details: {
    libraryPath: "/tmp/libfixture",
    packageRelativePath: "fixture.node",
  },
};
const checksumError: ChecksumMismatchError = new ChecksumMismatchError(
  "fixture_checksum",
  1,
  2,
  errorOptions,
);
const contractError: ContractVersionMismatchError = new ContractVersionMismatchError(
  1,
  2,
  errorOptions,
);
const runtimeChecksumErrorCtor: typeof ChecksumMismatchError =
  UniffiInternalError.ChecksumMismatchError;
const runtimeContractErrorCtor: typeof ContractVersionMismatchError =
  UniffiInternalError.ContractVersionMismatchError;

void current;
void echoedWhen;
void echoedDelayMs;
void scheduledDelayMs;
void scheduledDelays;
void echoedTemporalRecord;
void optionalWhen;
void reminders;
void flavor;
void scanResult;
void echoedMap;
void echoedBytes;
void echoedRecord;
void asyncRecord;
void configValue;
void asyncReader;
void fixtureError;
void missingTag;
void invalidStateTag;
void invalidStateMessage;
void parseTag;
void parseMessage;
void ffiMetadataValue;
void ffiIntegrityValue;
void maybeBindings;
void stillLoaded;
void didUnload;
void checksumError;
void contractError;
void runtimeChecksumErrorCtor;
void runtimeContractErrorCtor;
"#,
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

#[test]
fn typechecks_generated_callback_fixture_package_declarations() {
    let generated = generate_fixture_package("callbacks");
    let package_dir = &generated.package_dir;

    install_fixture_package_dependencies(package_dir);
    run_typescript_check(
        package_dir,
        "smoke.ts",
        r#"
import {
  AsyncLogError,
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
  type AsyncLogSink,
  type LogCollector,
  type LogRecord,
} from "./index.js";

const settings = Settings.default();
settings.set("writer.enabled", "true");
const settingsJson: string = settings.to_json_string();

const batch = new WriteBatch();
batch.put(new Uint8Array([1, 2]), new Uint8Array([3, 4]));
batch.delete(new Uint8Array([5]));
const operationCount: number = batch.operation_count();

const level: LogLevel = LogLevel.Info;
const seedRecord: LogRecord = {
  level,
  target: "callbacks_fixture",
  message: "seed",
  module_path: undefined,
  file: undefined,
  line: undefined,
};

const records: Array<LogRecord> = [seedRecord];
const collector: LogCollector = {
  log(record) {
    records.push(record);
  },
};

const sink = {
  write(message: string) {
    records.push({
      ...seedRecord,
      message,
    });
  },
  latest(): string | undefined {
    return records.length === 0 ? undefined : records[records.length - 1].message;
  },
};

const asyncSink: AsyncLogSink = {
  write(message: string): Promise<string> {
    return Promise.resolve(message);
  },
  write_fallible(message: string): Promise<string> {
    return Promise.reject(new AsyncLogErrorRejected(message));
  },
  flush(): Promise<void> {
    return Promise.resolve();
  },
};

const asyncWrite: (message: string) => Promise<string> = asyncSink.write;
const asyncWriteFallible: (message: string) => Promise<string> = asyncSink.write_fallible;
const asyncFlush: () => Promise<void> = asyncSink.flush;
const asyncLogError: AsyncLogError = new AsyncLogErrorRejected("bad write");
const rejectedTag: "Rejected" = new AsyncLogErrorRejected("bad write").tag;

emit(sink, "hello");
const latestMessage: string | undefined = last_message(sink);
const missingMessage: string | undefined = last_message(undefined);
init_logging(level, collector);
init_logging(LogLevel.Info, undefined);
const emittedAsyncMessage: Promise<string> = emit_async(asyncSink, "async hello");
const emittedAsyncFallibleMessage: Promise<string> = emit_async_fallible(
  asyncSink,
  "async hello",
);
const flushedAsyncSink: Promise<void> = flush_async(asyncSink);

void settingsJson;
void operationCount;
void latestMessage;
void missingMessage;
void asyncWrite;
void asyncWriteFallible;
void asyncFlush;
void asyncLogError;
void rejectedTag;
void emittedAsyncMessage;
void emittedAsyncFallibleMessage;
void flushedAsyncSink;
void records;
"#,
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}
