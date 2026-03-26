mod support;

use self::support::{
    generate_fixture_package, install_fixture_package_dependencies, remove_dir_all, run_node_script,
};

#[test]
fn runs_plain_js_smoke_script_against_generated_basic_fixture_package() {
    let generated = generate_fixture_package("basic");
    let package_dir = &generated.package_dir;

    install_fixture_package_dependencies(package_dir);
    run_node_script(
        package_dir,
        "smoke.mjs",
        r#"
import assert from "node:assert/strict";
import { Flavor, ScanResult, Store, echo_bytes, echo_record } from "./index.js";

const seed = {
  name: "seed",
  value: new Uint8Array([1, 2]),
  maybe_value: undefined,
  chunks: [new Uint8Array([3]), new Uint8Array([4, 5])],
};

const echoedBytes = echo_bytes(new Uint8Array([7, 8, 9]));
assert.deepStrictEqual(Array.from(echoedBytes), [7, 8, 9]);

const echoedRecord = echo_record(seed);
assert.equal(echoedRecord.name, "seed");
assert.deepStrictEqual(Array.from(echoedRecord.value), [1, 2]);
assert.equal(echoedRecord.maybe_value, undefined);
assert.deepStrictEqual(
  echoedRecord.chunks.map((chunk) => Array.from(chunk)),
  [[3], [4, 5]],
);

const store = new Store(seed);
const current = store.current();
assert.equal(current.name, "seed");
assert.deepStrictEqual(Array.from(current.value), [1, 2]);

const previous = store.replace(new Uint8Array([9, 8]));
assert.deepStrictEqual(Array.from(previous), [1, 2]);

assert.ok(Object.values(Flavor).includes(store.flavor()));
assert.equal(store.flavor().toLowerCase(), "vanilla");
const scanResult = store.inspect(true);
assert.equal(scanResult.tag, "Hit");
assert.deepStrictEqual(Array.from(scanResult.value), [9, 8]);
assert.deepStrictEqual(ScanResult.Miss(), { tag: "Miss" });

const asyncRecord = await store.fetch_async(true);
assert.equal(asyncRecord.name, "seed");
assert.deepStrictEqual(Array.from(asyncRecord.value), [9, 8]);
assert.deepStrictEqual(
  asyncRecord.chunks.map((chunk) => Array.from(chunk)),
  [[3], [4, 5], [9, 8]],
);
"#,
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}
