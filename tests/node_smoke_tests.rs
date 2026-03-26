mod support;

use self::support::{
    FixturePackageOptions, current_bundled_prebuild_target, generate_fixture_package,
    generate_fixture_package_with_options, install_fixture_package_dependencies, remove_dir_all,
    run_node_script,
};

#[test]
fn runs_plain_js_smoke_script_against_generated_basic_fixture_package() {
    let generated = generate_fixture_package("basic");
    let package_dir = &generated.package_dir;

    install_fixture_package_dependencies(package_dir);
    run_node_script(
        package_dir,
        "smoke.mjs",
        &format!(
            r#"
import assert from "node:assert/strict";
import {{ Flavor, ScanResult, Store, echo_bytes, echo_record }} from "./index.js";

{}
"#,
            basic_fixture_api_smoke_body()
        ),
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

#[test]
fn runs_plain_js_smoke_script_against_generated_callback_fixture_package() {
    let generated = generate_fixture_package("callbacks");
    let package_dir = &generated.package_dir;

    install_fixture_package_dependencies(package_dir);
    run_node_script(
        package_dir,
        "callback-smoke.mjs",
        &format!(
            r#"
import assert from "node:assert/strict";
import {{ LogLevel, Settings, WriteBatch, emit, init_logging, last_message }} from "./index.js";

{}
"#,
            callback_fixture_api_smoke_body()
        ),
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

#[test]
fn runs_plain_js_smoke_script_against_generated_bundled_basic_fixture_package() {
    let generated = generate_fixture_package_with_options(
        "basic",
        FixturePackageOptions {
            bundled_prebuilds: true,
            stage_root_sibling_library: false,
            stage_host_prebuild: true,
            ..FixturePackageOptions::default()
        },
    );
    let package_dir = &generated.package_dir;
    let expected_library_path = generated
        .bundled_prebuild_path
        .as_ref()
        .expect("bundled-mode fixture package should record the staged prebuild path");

    install_fixture_package_dependencies(package_dir);
    run_node_script(
        package_dir,
        "bundled-smoke.mjs",
        &format!(
            r#"
import assert from "node:assert/strict";
import {{ realpathSync }} from "node:fs";
import {{ Flavor, ScanResult, Store, echo_bytes, echo_record }} from "./index.js";
import {{ ffiMetadata, getFfiBindings, isLoaded }} from "./fixture-ffi.js";

assert.equal(ffiMetadata.bundledPrebuilds, true);
assert.equal(isLoaded(), true);
assert.equal(realpathSync(getFfiBindings().libraryPath), realpathSync({}));

{}
"#,
            serde_json::to_string(expected_library_path.as_str())
                .expect("bundled prebuild path should serialize"),
            basic_fixture_api_smoke_body()
        ),
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

#[test]
fn manual_load_explicit_path_overrides_missing_bundled_prebuild_and_is_idempotent() {
    let generated = generate_fixture_package_with_options(
        "basic",
        FixturePackageOptions {
            bundled_prebuilds: true,
            manual_load: true,
            stage_root_sibling_library: true,
            stage_host_prebuild: false,
        },
    );
    let package_dir = &generated.package_dir;
    let expected_library_path = generated
        .sibling_library_path
        .as_ref()
        .expect("manual-load regression fixture should stage a sibling library");

    install_fixture_package_dependencies(package_dir);
    run_node_script(
        package_dir,
        "manual-load-smoke.mjs",
        &format!(
            r#"
import assert from "node:assert/strict";
import {{ realpathSync }} from "node:fs";
import {{ Flavor, ScanResult, Store, echo_bytes, echo_record, load, unload }} from "./index.js";
import {{ ffiMetadata, getFfiBindings, isLoaded }} from "./fixture-ffi.js";

assert.equal(ffiMetadata.bundledPrebuilds, true);
assert.equal(ffiMetadata.manualLoad, true);
assert.equal(isLoaded(), false);

const firstBindings = load("./libfixture_basic.dylib");
assert.equal(isLoaded(), true);
assert.equal(realpathSync(getFfiBindings().libraryPath), realpathSync({}));

const secondBindings = load("libfixture_basic.dylib");
assert.strictEqual(secondBindings, firstBindings);

{}

assert.equal(unload(), true);
assert.equal(isLoaded(), false);
"#,
            serde_json::to_string(expected_library_path.as_str())
                .expect("sibling library path should serialize"),
            basic_fixture_api_smoke_body()
        ),
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

#[test]
fn bundled_mode_import_reports_missing_host_prebuild() {
    let generated = generate_fixture_package_with_options(
        "basic",
        FixturePackageOptions {
            bundled_prebuilds: true,
            stage_root_sibling_library: false,
            stage_host_prebuild: false,
            ..FixturePackageOptions::default()
        },
    );
    let package_dir = &generated.package_dir;
    let expected_library_filename = format!(
        "{}{}.{}",
        std::env::consts::DLL_PREFIX,
        generated.built_fixture.crate_name,
        std::env::consts::DLL_EXTENSION
    );
    let expected_target = current_bundled_prebuild_target();
    let expected_relative_path = format!("prebuilds/{expected_target}/{expected_library_filename}");

    assert!(
        generated.sibling_library_path.is_none(),
        "negative bundled fixture should not stage a root-level sibling library"
    );

    install_fixture_package_dependencies(package_dir);
    run_node_script(
        package_dir,
        "bundled-missing-prebuild.mjs",
        &format!(
            r#"
import assert from "node:assert/strict";

try {{
  await import("./index.js");
  assert.fail("expected bundled import to fail without a matching staged prebuild");
}} catch (error) {{
  const message = String(error);
  assert.ok(
    message.includes("No bundled UniFFI library was found for target"),
    `unexpected error message: ${{message}}`,
  );
  assert.ok(message.includes({}), `missing target id in error: ${{message}}`);
  assert.ok(message.includes({}), `missing package path in error: ${{message}}`);
}}
"#,
            serde_json::to_string(&expected_target).expect("target id should serialize"),
            serde_json::to_string(&expected_relative_path)
                .expect("expected package-relative path should serialize"),
        ),
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

fn basic_fixture_api_smoke_body() -> &'static str {
    r#"const seed = {
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
);"#
}

fn callback_fixture_api_smoke_body() -> &'static str {
    r#"const messages = [];
const sink = {
  write(message) {
    messages.push(message);
  },
  latest() {
    return messages.at(-1);
  },
};

assert.equal(last_message(undefined), undefined);
emit(sink, "first");
emit(sink, "second");
assert.deepStrictEqual(messages, ["first", "second"]);
assert.equal(last_message(sink), "second");

const settings = Settings.default();
settings.set("writer.cache_size", "1024");
settings.set("logging.level", "\"debug\"");
settings.set("writer.enabled", "true");
assert.equal(
  settings.to_json_string(),
  "{\"logging\":{\"level\":\"debug\"},\"writer\":{\"cache_size\":1024,\"enabled\":true}}",
);

const batch = new WriteBatch();
batch.put(Buffer.from([1, 2]), Buffer.from([3, 4]));
batch.delete(Buffer.from([5]));
batch.put(Buffer.from("k"), Buffer.from("value"));
assert.equal(batch.operation_count(), 3);

const records = [];
const collector = {
  log(record) {
    records.push(record);
  },
};

init_logging(LogLevel.Info, collector);
assert.deepStrictEqual(records, [
  {
    level: LogLevel.Info,
    target: "callbacks_fixture",
    message: "logging initialized",
    module_path: "callbacks_fixture::logging",
    file: undefined,
    line: undefined,
  },
]);

init_logging(LogLevel.Info, undefined);"#
}
