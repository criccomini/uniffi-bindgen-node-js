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
  Flavor,
  ScanResult,
  Store,
  echo_bytes,
  echo_record,
  type BlobRecord,
} from "./index.js";

const seed: BlobRecord = {
  name: "seed",
  value: new Uint8Array([1, 2]),
  maybe_value: undefined,
  chunks: [new Uint8Array([3]), new Uint8Array([4, 5])],
};

const store = new Store(seed);
const current: BlobRecord = store.current();
const flavor: Flavor = store.flavor();
const scanResult: ScanResult = store.inspect(true);
const echoedBytes: Uint8Array = echo_bytes(new Uint8Array([7, 8, 9]));
const echoedRecord: BlobRecord = echo_record(seed);
const asyncRecord: Promise<BlobRecord> = store.fetch_async(true);

void current;
void flavor;
void scanResult;
void echoedBytes;
void echoedRecord;
void asyncRecord;
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
  LogLevel,
  Settings,
  WriteBatch,
  emit,
  init_logging,
  last_message,
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

emit(sink, "hello");
const latestMessage: string | undefined = last_message(sink);
const missingMessage: string | undefined = last_message(undefined);
init_logging(level, collector);
init_logging(LogLevel.Info, undefined);

void settingsJson;
void operationCount;
void latestMessage;
void missingMessage;
void records;
"#,
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}
