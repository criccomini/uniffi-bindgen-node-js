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
