mod support;

use std::fs;

use self::support::{
    FixturePackageOptions, component_with_namespace, current_bundled_prebuild_target,
    generate_fixture_package, generate_fixture_package_with_options, generation_settings,
    generator, install_fixture_package_dependencies, remove_dir_all, run_node_script,
};
use uniffi_bindgen::BindingGenerator;

#[test]
fn runtime_object_factory_keeps_generic_pointer_handles_until_clone() {
    let settings = generation_settings("runtime-object-factory-generic-pointer");
    let output_dir = settings.out_dir.clone();

    generator()
        .write_bindings(&settings, &[component_with_namespace("example")])
        .expect("write_bindings should succeed");

    fs::write(
        output_dir.join("package.json").as_std_path(),
        r#"{"type":"module"}"#,
    )
    .expect("package.json should be writable");

    let koffi_dir = output_dir.join("node_modules").join("koffi");
    fs::create_dir_all(koffi_dir.as_std_path()).expect("koffi fixture dir should be creatable");
    fs::write(
        koffi_dir.join("package.json").as_std_path(),
        r#"{"name":"koffi","type":"module","main":"./index.js"}"#,
    )
    .expect("koffi package.json should be writable");
    fs::write(
        koffi_dir.join("index.js").as_std_path(),
        r#"function normalizePointerAddress(value) {
  if (typeof value === "bigint") {
    return value;
  }
  if (typeof value === "number") {
    return BigInt(value);
  }
  if (typeof value === "object" && value != null && typeof value.__addr === "bigint") {
    return value.__addr;
  }
  throw new TypeError(`expected a pointer-compatible value, got ${typeof value}`);
}

const koffi = {
  opaque() {
    return { kind: "opaque" };
  },
  pointer(typeOrName, maybeType) {
    return {
      kind: "pointer",
      name: maybeType == null ? null : typeOrName,
      to: maybeType ?? typeOrName,
    };
  },
  struct(name, fields) {
    return {
      kind: "struct",
      name,
      fields,
    };
  },
  as(value, type) {
    if (typeof value !== "object" || value == null || typeof value.__addr !== "bigint") {
      throw new TypeError("Invalid argument");
    }
    return {
      __addr: normalizePointerAddress(value),
      __pointer: value,
      __type: type,
    };
  },
  address(pointer) {
    return normalizePointerAddress(pointer);
  },
};

export default koffi;
"#,
    )
    .expect("koffi index.js should be writable");

    run_node_script(
        &output_dir,
        "objects-generic-pointer-smoke.mjs",
        r#"
import assert from "node:assert/strict";
import koffi from "koffi";
import { createObjectFactory } from "./runtime/objects.js";

const genericHandleType = koffi.pointer("RustArcPtr", koffi.opaque());
const resourceHandleType = koffi.pointer("RustArcPtrResource", koffi.opaque());

class Resource {
  ping() {
    return resourceFactory.cloneHandle(this);
  }
}

const resourceFactory = createObjectFactory({
  typeName: "Resource",
  createInstance: () => Object.create(Resource.prototype),
  handleType: () => resourceHandleType,
  cloneHandle(handle) {
    assert.equal(handle.__type?.name, "RustArcPtrResource");
    return handle.__pointer;
  },
});

const rawHandle = {
  __addr: 42n,
  __type: genericHandleType,
};

const resource = resourceFactory.create(rawHandle);
assert.equal(typeof resource.ping, "function");
assert.doesNotThrow(() => resource.ping());
assert.equal(resourceFactory.peekHandle(resource).__type?.name, "RustArcPtr");
"#,
    );

    remove_dir_all(&output_dir);
}

#[test]
fn runtime_object_factory_keeps_raw_handles_for_follow_up_calls() {
    let settings = generation_settings("runtime-object-factory-retyped-handles");
    let output_dir = settings.out_dir.clone();

    generator()
        .write_bindings(&settings, &[component_with_namespace("example")])
        .expect("write_bindings should succeed");

    fs::write(
        output_dir.join("package.json").as_std_path(),
        r#"{"type":"module"}"#,
    )
    .expect("package.json should be writable");

    let koffi_dir = output_dir.join("node_modules").join("koffi");
    fs::create_dir_all(koffi_dir.as_std_path()).expect("koffi fixture dir should be creatable");
    fs::write(
        koffi_dir.join("package.json").as_std_path(),
        r#"{"name":"koffi","type":"module","main":"./index.js"}"#,
    )
    .expect("koffi package.json should be writable");
    fs::write(
        koffi_dir.join("index.js").as_std_path(),
        r#"function normalizePointerAddress(value) {
  if (typeof value === "bigint") {
    return value;
  }
  if (typeof value === "number") {
    return BigInt(value);
  }
  if (typeof value === "object" && value != null && typeof value.__addr === "bigint") {
    return value.__addr;
  }
  throw new TypeError(`expected a pointer-compatible value, got ${typeof value}`);
}

const koffi = {
  opaque() {
    return { kind: "opaque" };
  },
  pointer(typeOrName, maybeType) {
    return {
      kind: "pointer",
      name: maybeType == null ? null : typeOrName,
      to: maybeType ?? typeOrName,
    };
  },
  struct(name, fields) {
    return {
      kind: "struct",
      name,
      fields,
    };
  },
  as(value, type) {
    if (typeof value !== "object" || value == null || typeof value.__addr !== "bigint") {
      throw new TypeError("Invalid argument");
    }
    return {
      __addr: value.__addr,
      __pointer: value,
      __retagged: true,
      __type: type,
    };
  },
  address(pointer) {
    if (pointer?.__retagged === true) {
      throw new TypeError(
        `Unexpected ${pointer.__type?.name ?? "pointer"} value for ptr, expected external pointer`,
      );
    }
    return normalizePointerAddress(pointer);
  },
};

export default koffi;
"#,
    )
    .expect("koffi index.js should be writable");

    run_node_script(
        &output_dir,
        "objects-external-pointer-fallback-smoke.mjs",
        r#"
import assert from "node:assert/strict";
import koffi from "koffi";
import { createObjectFactory } from "./runtime/objects.js";

const genericHandleType = koffi.pointer("RustArcPtr", koffi.opaque());
const resourceHandleType = koffi.pointer("RustArcPtrResource", koffi.opaque());

class Resource {
  ping() {
    return resourceFactory.cloneHandle(this);
  }
}

const resourceFactory = createObjectFactory({
  typeName: "Resource",
  createInstance: () => Object.create(Resource.prototype),
  handleType: () => resourceHandleType,
  cloneHandle(handle) {
    assert.equal(handle.__type?.name, "RustArcPtrResource");
    assert.equal(handle.__addr, 42n);
    assert.equal(handle.__retagged, true);
    return handle.__pointer;
  },
});

const rawHandle = {
  __addr: 42n,
  __type: genericHandleType,
};

const resource = resourceFactory.createRetyped(rawHandle);
assert.equal(typeof resource.ping, "function");
assert.doesNotThrow(() => resource.ping());
assert.equal(resourceFactory.peekHandle(resource).__type?.name, "RustArcPtr");
"#,
    );

    remove_dir_all(&output_dir);
}

#[test]
fn runtime_object_factory_decodes_numeric_handles_before_pointer_cast() {
    let settings = generation_settings("runtime-object-factory-numeric-handles");
    let output_dir = settings.out_dir.clone();

    generator()
        .write_bindings(&settings, &[component_with_namespace("example")])
        .expect("write_bindings should succeed");

    fs::write(
        output_dir.join("package.json").as_std_path(),
        r#"{"type":"module"}"#,
    )
    .expect("package.json should be writable");

    let koffi_dir = output_dir.join("node_modules").join("koffi");
    fs::create_dir_all(koffi_dir.as_std_path()).expect("koffi fixture dir should be creatable");
    fs::write(
        koffi_dir.join("package.json").as_std_path(),
        r#"{"name":"koffi","type":"module","main":"./index.js"}"#,
    )
    .expect("koffi package.json should be writable");
    fs::write(
        koffi_dir.join("index.js").as_std_path(),
        r#"const koffi = {
  opaque() {
    return { kind: "opaque" };
  },
  pointer(typeOrName, maybeType) {
    return {
      kind: "pointer",
      name: maybeType == null ? null : typeOrName,
      to: maybeType ?? typeOrName,
    };
  },
  struct(name, fields) {
    return {
      kind: "struct",
      name,
      fields,
    };
  },
  as(value, _type) {
    if (typeof value === "bigint" || typeof value === "number") {
      throw new TypeError("Invalid argument");
    }
    return value;
  },
  decode(value, type) {
    if (!(value instanceof BigUint64Array)) {
      throw new TypeError("expected BigUint64Array");
    }
    return {
      __addr: value[0],
      __decoded: true,
      __type: type,
    };
  },
  address(pointer) {
    return pointer?.__addr ?? BigInt(pointer);
  },
};

export default koffi;
"#,
    )
    .expect("koffi index.js should be writable");

    run_node_script(
        &output_dir,
        "objects-numeric-handle-smoke.mjs",
        r#"
import assert from "node:assert/strict";
import koffi from "koffi";
import { createObjectFactory } from "./runtime/objects.js";

const resourceHandleType = koffi.pointer("RustArcPtrResource", koffi.opaque());

class Resource {}

const resourceFactory = createObjectFactory({
  typeName: "Resource",
  createInstance: () => Object.create(Resource.prototype),
  handleType: () => resourceHandleType,
});

const resource = resourceFactory.create(42n);
const typedHandle = resourceFactory.handle(resource);
assert.equal(typedHandle.__decoded, true);
assert.equal(typedHandle.__addr, 42n);
assert.equal(typedHandle.__type?.name, "RustArcPtrResource");
"#,
    );

    remove_dir_all(&output_dir);
}

#[test]
fn runtime_object_converter_retypes_deserialized_handles() {
    let settings = generation_settings("runtime-object-converter-deserialized-handles");
    let output_dir = settings.out_dir.clone();

    generator()
        .write_bindings(&settings, &[component_with_namespace("example")])
        .expect("write_bindings should succeed");

    fs::write(
        output_dir.join("package.json").as_std_path(),
        r#"{"type":"module"}"#,
    )
    .expect("package.json should be writable");

    let koffi_dir = output_dir.join("node_modules").join("koffi");
    fs::create_dir_all(koffi_dir.as_std_path()).expect("koffi fixture dir should be creatable");
    fs::write(
        koffi_dir.join("package.json").as_std_path(),
        r#"{"name":"koffi","type":"module","main":"./index.js"}"#,
    )
    .expect("koffi package.json should be writable");
    fs::write(
        koffi_dir.join("index.js").as_std_path(),
        r#"const koffi = {
  struct(name, fields) {
    return {
      kind: "struct",
      name,
      fields,
    };
  },
  address(pointer) {
    return pointer?.__addr ?? BigInt(pointer);
  },
};

export default koffi;
"#,
    )
    .expect("koffi index.js should be writable");

    run_node_script(
        &output_dir,
        "objects-converter-deserialized-handle-smoke.mjs",
        r#"
import assert from "node:assert/strict";
import { createObjectConverter, createObjectFactory } from "./runtime/objects.js";

class Resource {}

const resourceFactory = createObjectFactory({
  typeName: "Resource",
  createInstance: () => Object.create(Resource.prototype),
  cloneHandle() {
    throw new Error("typed clone should not be used for deserialized handles");
  },
  cloneHandleGeneric(handle) {
    assert.equal(handle, 42n);
    return 43n;
  },
});

const converter = createObjectConverter(resourceFactory);
const resource = converter.read({
  readUInt64() {
    return 42n;
  },
});

assert.equal(resourceFactory.usesGenericAbi(resource), true);
assert.equal(resourceFactory.peekHandle(resource), 42n);
assert.equal(resourceFactory.cloneHandle(resource), 43n);
"#,
    );

    remove_dir_all(&output_dir);
}

#[test]
fn runtime_object_factory_keeps_raw_external_handles_for_follow_up_calls() {
    let settings = generation_settings("runtime-object-factory-raw-external-handles");
    let output_dir = settings.out_dir.clone();

    generator()
        .write_bindings(&settings, &[component_with_namespace("example")])
        .expect("write_bindings should succeed");

    fs::write(
        output_dir.join("package.json").as_std_path(),
        r#"{"type":"module"}"#,
    )
    .expect("package.json should be writable");

    let koffi_dir = output_dir.join("node_modules").join("koffi");
    fs::create_dir_all(koffi_dir.as_std_path()).expect("koffi fixture dir should be creatable");
    fs::write(
        koffi_dir.join("package.json").as_std_path(),
        r#"{"name":"koffi","type":"module","main":"./index.js"}"#,
    )
    .expect("koffi package.json should be writable");
    fs::write(
        koffi_dir.join("index.js").as_std_path(),
        r#"function normalizePointerAddress(value) {
  if (typeof value === "bigint") {
    return value;
  }
  if (typeof value === "number") {
    return BigInt(value);
  }
  if (typeof value === "object" && value != null && typeof value.__addr === "bigint") {
    return value.__addr;
  }
  throw new TypeError(`expected a pointer-compatible value, got ${typeof value}`);
}

const koffi = {
  opaque() {
    return { kind: "opaque" };
  },
  pointer(typeOrName, maybeType) {
    return {
      kind: "pointer",
      name: maybeType == null ? null : typeOrName,
      to: maybeType ?? typeOrName,
    };
  },
  struct(name, fields) {
    return {
      kind: "struct",
      name,
      fields,
    };
  },
  as(value, type) {
    if (typeof value !== "object" || value == null || typeof value.__addr !== "bigint") {
      throw new TypeError("Invalid argument");
    }
    return {
      __addr: value.__addr,
      __pointer: value,
      __retagged: true,
      __type: type,
    };
  },
  address(pointer) {
    if (pointer?.__retagged === true) {
      throw new TypeError(
        `Unexpected ${pointer.__type?.name ?? "pointer"} value for ptr, expected external pointer`,
      );
    }
    return normalizePointerAddress(pointer);
  },
};

export default koffi;
"#,
    )
    .expect("koffi index.js should be writable");

    run_node_script(
        &output_dir,
        "objects-raw-external-pointer-smoke.mjs",
        r#"
import assert from "node:assert/strict";
import koffi from "koffi";
import { createObjectFactory } from "./runtime/objects.js";

const resourceHandleType = koffi.pointer("RustArcPtrResource", koffi.opaque());

class Resource {
  ping() {
    return resourceFactory.cloneHandle(this);
  }
}

const rawHandle = {
  __addr: 42n,
};
const adoptedHandle = {
  __addr: 84n,
};
const freedHandles = [];

const resourceFactory = createObjectFactory({
  typeName: "Resource",
  createInstance: () => Object.create(Resource.prototype),
  handleType: () => resourceHandleType,
  cloneHandle() {
    throw new Error("typed clone should not be used for raw external handles");
  },
  cloneHandleRawExternal(handle) {
    assert.equal(handle.__retagged, undefined);
    return handle === rawHandle
      ? adoptedHandle
      : handle;
  },
  freeHandle() {
    throw new Error("typed free should not be used for raw external handles");
  },
  freeHandleRawExternal(handle) {
    assert.equal(handle.__retagged, undefined);
    freedHandles.push(handle.__addr);
  },
});

const resource = resourceFactory.createRawExternal(rawHandle);
assert.equal(typeof resource.ping, "function");
assert.deepEqual(freedHandles, [42n]);
const cloned = resource.ping();
assert.equal(cloned.__retagged, true);
assert.equal(cloned.__type?.name, "RustArcPtrResource");
assert.equal(cloned.__pointer, adoptedHandle);
assert.equal(resourceFactory.handle(resource).__type?.name, "RustArcPtrResource");
assert.strictEqual(resourceFactory.peekHandle(resource), adoptedHandle);
assert.equal(resourceFactory.usesRawExternal(resource), true);
assert.equal(resourceFactory.destroy(resource), true);
assert.deepEqual(freedHandles, [42n, 84n]);
"#,
    );

    remove_dir_all(&output_dir);
}

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
import {{
  Config,
  FixtureErrorInvalidState,
  FixtureErrorParse,
  Flavor,
  ReaderBuilder,
  ScanResult,
  Store,
  echo_byte_map,
  echo_bytes,
  echo_duration,
  echo_record,
  echo_timestamp,
}} from "./index.js";

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
import {{
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
}} from "./index.js";
import {{ foreignFutureHandleCount }} from "./runtime/callbacks.js";

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
import {{
  Config,
  FixtureErrorInvalidState,
  FixtureErrorParse,
  Flavor,
  ReaderBuilder,
  ScanResult,
  Store,
  echo_byte_map,
  echo_bytes,
  echo_duration,
  echo_record,
  echo_timestamp,
}} from "./index.js";
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
    let expected_library_filename = expected_library_path
        .file_name()
        .expect("manual-load regression fixture library should have a filename");

    install_fixture_package_dependencies(package_dir);
    run_node_script(
        package_dir,
        "manual-load-smoke.mjs",
        &format!(
            r#"
import assert from "node:assert/strict";
import {{ realpathSync }} from "node:fs";
import koffi from "koffi";
import {{
  Config,
  FixtureErrorInvalidState,
  FixtureErrorParse,
  Flavor,
  ReaderBuilder,
  ScanResult,
  Store,
  echo_byte_map,
  echo_bytes,
  echo_duration,
  echo_record,
  echo_timestamp,
  load,
  unload,
}} from "./index.js";
import {{ ffiMetadata, getFfiBindings, isLoaded }} from "./fixture-ffi.js";

assert.equal(ffiMetadata.bundledPrebuilds, true);
assert.equal(ffiMetadata.manualLoad, true);
assert.equal(isLoaded(), false);

const firstBindings = load({});
assert.equal(isLoaded(), true);
assert.equal(realpathSync(getFfiBindings().libraryPath), realpathSync({}));

const secondBindings = load({});
assert.strictEqual(secondBindings, firstBindings);
assert.equal(koffi.registeredCallbackCount(), 0);

{}

assert.equal(koffi.registeredCallbackCount(), 1);
assert.equal(unload(), true);
assert.equal(isLoaded(), false);
assert.equal(koffi.registeredCallbackCount(), 0);
"#,
            serde_json::to_string(&format!("./{expected_library_filename}"))
                .expect("relative library path should serialize"),
            serde_json::to_string(expected_library_path.as_str())
                .expect("sibling library path should serialize"),
            serde_json::to_string(expected_library_filename)
                .expect("library filename should serialize"),
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

const when = new Date("2024-01-02T03:04:05.678Z");
const echoedWhen = echo_timestamp(when);
assert.ok(echoedWhen instanceof Date);
assert.equal(echoedWhen.getTime(), when.getTime());
assert.throws(() => echo_timestamp(new Date(Number.NaN)), (error) => {
  assert.ok(error instanceof TypeError);
  assert.equal(error.message, "timestamp values must be valid Date instances.");
  return true;
});

const echoedDelayMs = echo_duration(1_500);
assert.equal(typeof echoedDelayMs, "number");
assert.equal(echoedDelayMs, 1_500);
assert.throws(() => echo_duration(-1), (error) => {
  assert.equal(error?.name, "ConverterRangeError");
  assert.equal(
    error?.message,
    "duration values must be non-negative integer millisecond counts.",
  );
  return true;
});

const echoedRecord = echo_record(seed);
assert.equal(echoedRecord.name, "seed");
assert.deepStrictEqual(Array.from(echoedRecord.value), [1, 2]);
assert.equal(echoedRecord.maybe_value, undefined);
assert.deepStrictEqual(
  echoedRecord.chunks.map((chunk) => Array.from(chunk)),
  [[3], [4, 5]],
);

const echoedMap = echo_byte_map(
  new Map([
    ["alpha", new Uint8Array([6, 7, 8])],
    ["beta", new Uint8Array([9])],
  ]),
);
assert.ok(echoedMap instanceof Map);
assert.deepStrictEqual(
  Array.from(echoedMap.entries(), ([key, value]) => [key, Array.from(value)]),
  [["alpha", [6, 7, 8]], ["beta", [9]]],
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

const config = Config.from_json("ok");
assert.equal(config.value(), "ok");
assert.throws(() => Config.from_json("not-json"), (error) => {
  assert.ok(error instanceof FixtureErrorParse);
  assert.equal(error.name, "FixtureErrorParse");
  assert.equal(error.message, "invalid json");
  return true;
});

const reader = await new ReaderBuilder(true).build();
assert.equal(reader.label(), "ready");
assert.equal(await reader.label_async(), "ready");
assert.equal(reader.label(), "ready");
await assert.rejects(new ReaderBuilder(false).build(), (error) => {
  assert.ok(error instanceof FixtureErrorInvalidState);
  assert.equal(error.name, "FixtureErrorInvalidState");
  assert.equal(error.message, "builder rejected");
  return true;
});"#
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

init_logging(LogLevel.Info, undefined);

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

assert.equal(foreignFutureHandleCount(), 0);

const successWrite = deferred();
const successSink = {
  write(message) {
    assert.equal(message, "async-success");
    return successWrite.promise;
  },
  write_fallible(message) {
    return Promise.resolve(`fallible:${message}`);
  },
  flush() {
    return Promise.resolve();
  },
};

const successPromise = emit_async(successSink, "async-success");
assert.equal(foreignFutureHandleCount(), 1);
successWrite.resolve("async-success:done");
assert.equal(await successPromise, "async-success:done");
assert.equal(foreignFutureHandleCount(), 0);

const flushCompletion = deferred();
let flushStarted = false;
const flushSink = {
  write(message) {
    return Promise.resolve(message);
  },
  write_fallible(message) {
    return Promise.resolve(message);
  },
  flush() {
    flushStarted = true;
    return flushCompletion.promise;
  },
};

const flushPromise = flush_async(flushSink);
assert.equal(flushStarted, true);
assert.equal(foreignFutureHandleCount(), 1);
flushCompletion.resolve();
await flushPromise;
assert.equal(foreignFutureHandleCount(), 0);

const typedFailure = deferred();
const typedErrorSink = {
  write(message) {
    return Promise.resolve(message);
  },
  write_fallible(message) {
    assert.equal(message, "typed-error");
    return typedFailure.promise;
  },
  flush() {
    return Promise.resolve();
  },
};

const typedPromise = emit_async_fallible(typedErrorSink, "typed-error");
assert.equal(foreignFutureHandleCount(), 1);
typedFailure.reject(new AsyncLogErrorRejected("typed rejection"));
await assert.rejects(typedPromise, (error) => {
  assert.ok(error instanceof AsyncLogErrorRejected);
  assert.equal(error.name, "AsyncLogErrorRejected");
  assert.equal(error.tag, "Rejected");
  assert.equal(error.message, "typed rejection");
  return true;
});
assert.equal(foreignFutureHandleCount(), 0);

const unexpectedFailure = deferred();
const unexpectedErrorSink = {
  write(message) {
    return Promise.resolve(message);
  },
  write_fallible(message) {
    assert.equal(message, "unexpected-error");
    return unexpectedFailure.promise;
  },
  flush() {
    return Promise.resolve();
  },
};

const unexpectedPromise = emit_async_fallible(unexpectedErrorSink, "unexpected-error");
assert.equal(foreignFutureHandleCount(), 1);
unexpectedFailure.reject("unexpected async rejection");
await assert.rejects(unexpectedPromise, (error) => {
  assert.equal(error.name, "RustPanic");
  assert.equal(error.message, "unexpected async rejection");
  return true;
});
assert.equal(foreignFutureHandleCount(), 0);

const cancelledWrite = deferred();
const cancellationWrites = [];
const cancellationSink = {
  write(message) {
    cancellationWrites.push(message);
    return cancelledWrite.promise;
  },
  write_fallible(message) {
    return Promise.resolve(message);
  },
  flush() {
    return Promise.resolve();
  },
};

cancel_emit_async(cancellationSink, "cancelled");
assert.deepStrictEqual(cancellationWrites, ["cancelled"]);
assert.equal(foreignFutureHandleCount(), 0);
cancelledWrite.resolve("ignored after cancellation");
await cancelledWrite.promise;
await Promise.resolve();
assert.equal(foreignFutureHandleCount(), 0);"#
}
