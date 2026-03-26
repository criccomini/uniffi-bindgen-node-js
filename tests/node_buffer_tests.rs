mod support;

use self::support::{
    generate_fixture_package, install_fixture_package_dependencies, remove_dir_all, run_node_script,
};

#[test]
fn accepts_node_buffer_values_for_uint8array_parameters() {
    let generated = generate_fixture_package("basic");
    let package_dir = &generated.package_dir;

    install_fixture_package_dependencies(package_dir);
    run_node_script(
        package_dir,
        "buffer-smoke.mjs",
        r#"
import assert from "node:assert/strict";
import { Store, echo_bytes, echo_record } from "./index.js";

const echoedBytes = echo_bytes(Buffer.from([7, 8, 9]));
assert.ok(echoedBytes instanceof Uint8Array);
assert.deepStrictEqual(Array.from(echoedBytes), [7, 8, 9]);

const seededRecord = echo_record({
  name: "buffer-seed",
  value: Buffer.from([1, 2]),
  maybe_value: Buffer.from([3, 4]),
  chunks: [Buffer.from([5]), Buffer.from([6, 7])],
});
assert.deepStrictEqual(Array.from(seededRecord.value), [1, 2]);
assert.deepStrictEqual(Array.from(seededRecord.maybe_value ?? []), [3, 4]);
assert.deepStrictEqual(
  seededRecord.chunks.map((chunk) => Array.from(chunk)),
  [[5], [6, 7]],
);

const store = new Store(seededRecord);
const previous = store.replace(Buffer.from([9, 8, 7]));
assert.deepStrictEqual(Array.from(previous), [1, 2]);

const current = store.current();
assert.ok(current.value instanceof Uint8Array);
assert.ok(current.maybe_value instanceof Uint8Array);
assert.deepStrictEqual(Array.from(current.value), [9, 8, 7]);
assert.deepStrictEqual(Array.from(current.maybe_value ?? []), [9, 8, 7]);
assert.deepStrictEqual(
  current.chunks.map((chunk) => Array.from(chunk)),
  [[5], [6, 7], [9, 8, 7]],
);
"#,
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}
