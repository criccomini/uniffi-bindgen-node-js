mod support;

use self::support::{
    generate_slatedb_package, install_generated_package_dependencies, remove_dir_all,
    run_node_script,
};

#[test]
fn runs_plain_js_smoke_script_against_generated_slatedb_package() {
    let generated = generate_slatedb_package();
    let package_dir = &generated.package_dir;

    install_generated_package_dependencies(package_dir);
    run_node_script(
        package_dir,
        "slatedb-smoke.mjs",
        r#"
import assert from "node:assert/strict";
import { Buffer } from "node:buffer";

const module = await import("./index.js");
const { LogLevel, Settings, WriteBatch, init_logging } = module;

assert.equal(typeof Settings.default, "function");
assert.equal(typeof WriteBatch, "function");
assert.equal(typeof init_logging, "function");

const settings = Settings.default();
settings.set("compactor_options.max_sst_size", "33554432");
const settingsJson = JSON.parse(settings.to_json_string());
assert.equal(settingsJson.compactor_options.max_sst_size, 33554432);

const batch = new WriteBatch();
batch.put(Buffer.from("alpha"), Buffer.from("bravo"));
batch.delete(Buffer.from("alpha"));

const records = [];
init_logging(LogLevel.Info, {
  log(record) {
    records.push(record);
  },
});

assert.deepStrictEqual(records, [
  {
    level: LogLevel.Info,
    target: "slatedb",
    message: "logging initialized",
    module_path: "slatedb::logging",
    file: undefined,
    line: undefined,
  },
]);
"#,
    );

    remove_dir_all(&generated.built_slatedb.target_dir);
    remove_dir_all(package_dir);
}
