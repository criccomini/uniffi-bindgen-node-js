mod support;

use std::fs;

use self::support::{
    FixturePackageOptions, generate_fixture_package, generate_fixture_package_with_options,
    install_fixture_package_dependencies, remove_dir_all, run_node_script,
};

#[test]
fn runtime_object_factory_keeps_generic_pointer_handles_until_clone() {
    let generated = generate_fixture_package("basic");
    let output_dir = generated.package_dir.clone();

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

const genericHandleType = koffi.pointer("ForeignHandle", koffi.opaque());
const resourceHandleType = koffi.pointer("ResourceHandle", koffi.opaque());

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
    assert.equal(handle.__type?.name, "ResourceHandle");
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
assert.equal(resourceFactory.peekHandle(resource).__type?.name, "ForeignHandle");
"#,
    );

    remove_dir_all(&output_dir);
    remove_dir_all(&generated.built_fixture.workspace_dir);
}

#[test]
fn runtime_object_factory_keeps_raw_handles_for_follow_up_calls() {
    let generated = generate_fixture_package("basic");
    let output_dir = generated.package_dir.clone();

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

const genericHandleType = koffi.pointer("ForeignHandle", koffi.opaque());
const resourceHandleType = koffi.pointer("ResourceHandle", koffi.opaque());

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
    assert.equal(handle.__type?.name, "ResourceHandle");
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
assert.equal(resourceFactory.peekHandle(resource).__type?.name, "ForeignHandle");
"#,
    );

    remove_dir_all(&output_dir);
    remove_dir_all(&generated.built_fixture.workspace_dir);
}

#[test]
fn runtime_object_factory_decodes_numeric_handles_before_pointer_cast() {
    let generated = generate_fixture_package("basic");
    let output_dir = generated.package_dir.clone();

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

const resourceHandleType = koffi.pointer("ResourceHandle", koffi.opaque());

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
assert.equal(typedHandle.__type?.name, "ResourceHandle");
"#,
    );

    remove_dir_all(&output_dir);
    remove_dir_all(&generated.built_fixture.workspace_dir);
}

#[test]
fn runtime_object_converter_retypes_deserialized_handles() {
    let generated = generate_fixture_package("basic");
    let output_dir = generated.package_dir.clone();

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
    remove_dir_all(&generated.built_fixture.workspace_dir);
}

#[test]
fn runtime_object_factory_clone_and_free_can_use_raw_uniffi_handles() {
    let generated = generate_fixture_package("basic");
    let output_dir = generated.package_dir.clone();

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
  decode(value, type) {
    return {
      __decoded: true,
      __type: type,
      __value: value,
    };
  },
  as(value, type) {
    return {
      __coerced: true,
      __type: type,
      __value: value,
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
        "objects-uniffi-handle-clone-free-smoke.mjs",
        r#"
import assert from "node:assert/strict";
import { createObjectFactory } from "./runtime/objects.js";

class Resource {}

const clonedHandles = [];
const freedHandles = [];

const resourceFactory = createObjectFactory({
  typeName: "Resource",
  createInstance: () => Object.create(Resource.prototype),
  cloneFreeUsesUniffiHandle: true,
  handleType: () => "ResourceHandle",
  cloneHandle(handle) {
    clonedHandles.push(handle);
    return handle + 1n;
  },
  freeHandle(handle) {
    freedHandles.push(handle);
  },
});

const resource = resourceFactory.create(42n);
assert.equal(resourceFactory.handle(resource).__decoded, true);
assert.equal(resourceFactory.cloneHandle(resource), 43n);
assert.deepEqual(clonedHandles, [42n]);
assert.equal(resourceFactory.destroy(resource), true);
assert.deepEqual(freedHandles, [42n]);
"#,
    );

    remove_dir_all(&output_dir);
    remove_dir_all(&generated.built_fixture.workspace_dir);
}

#[test]
fn runtime_object_factory_keeps_raw_external_handles_for_follow_up_calls() {
    let generated = generate_fixture_package("basic");
    let output_dir = generated.package_dir.clone();

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

const resourceHandleType = koffi.pointer("ResourceHandle", koffi.opaque());

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
assert.equal(cloned.__type?.name, "ResourceHandle");
assert.equal(cloned.__pointer, adoptedHandle);
assert.equal(resourceFactory.handle(resource).__type?.name, "ResourceHandle");
assert.strictEqual(resourceFactory.peekHandle(resource), adoptedHandle);
assert.equal(resourceFactory.usesRawExternal(resource), true);
assert.equal(resourceFactory.destroy(resource), true);
assert.deepEqual(freedHandles, [42n, 84n]);
"#,
    );

    remove_dir_all(&output_dir);
    remove_dir_all(&generated.built_fixture.workspace_dir);
}

#[test]
fn runtime_callback_registry_clone_preserves_the_registered_implementation() {
    let generated = generate_fixture_package("callbacks");
    let output_dir = generated.package_dir.clone();

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
};

export default koffi;
"#,
    )
    .expect("koffi index.js should be writable");

    run_node_script(
        &output_dir,
        "callbacks-registry-clone-smoke.mjs",
        r#"
import assert from "node:assert/strict";
import { createCallbackRegistry } from "./runtime/callbacks.js";

const registry = createCallbackRegistry({
  interfaceName: "LogSink",
});
const sink = {
  latest() {
    return "latest";
  },
};

const originalHandle = registry.register(sink);
const clonedHandle = registry.cloneHandle(originalHandle);

assert.notEqual(clonedHandle, originalHandle);
assert.strictEqual(registry.get(originalHandle), sink);
assert.strictEqual(registry.get(clonedHandle), sink);

registry.remove(originalHandle);
assert.strictEqual(registry.get(clonedHandle), sink);
assert.equal(registry.size, 1);
assert.equal(registry.take(clonedHandle).latest(), "latest");
assert.equal(registry.size, 0);
"#,
    );

    remove_dir_all(&output_dir);
    remove_dir_all(&generated.built_fixture.workspace_dir);
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
import {{
  UniffiCallbackRegistry,
  foreignFutureHandleCount,
  invokeAsyncCallbackMethod,
  writeCallbackError,
}} from "./runtime/callbacks.js";
import {{ EMPTY_RUST_BUFFER }} from "./runtime/ffi-types.js";
import {{
  CALL_ERROR,
  CALL_UNEXPECTED_ERROR,
  createRustCallStatus,
}} from "./runtime/rust-call.js";

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
        },
    );
    let package_dir = &generated.package_dir;
    let staged_prebuild_path = generated
        .bundled_prebuild_path
        .as_ref()
        .expect("bundled manual-load fixture should stage a bundled prebuild");
    fs::remove_file(staged_prebuild_path.as_std_path())
        .expect("bundled manual-load regression fixture should allow removing the staged prebuild");
    let expected_library_path = &generated.built_fixture.library_path;
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
import {{ copyFileSync, mkdtempSync, realpathSync, rmSync, symlinkSync }} from "node:fs";
import {{ tmpdir }} from "node:os";
import {{ join }} from "node:path";
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

const stagedOverridePath = join(process.cwd(), {1});
copyFileSync({0}, stagedOverridePath);

const firstBindings = load(stagedOverridePath);
assert.equal(isLoaded(), true);
assert.equal(realpathSync(getFfiBindings().libraryPath), realpathSync(stagedOverridePath));

const secondBindings = load(stagedOverridePath);
assert.strictEqual(secondBindings, firstBindings);
assert.equal(koffi.registeredCallbackCount(), 0);

{2}

assert.equal(koffi.registeredCallbackCount(), 1);
assert.equal(unload(), true);
assert.equal(isLoaded(), false);
assert.equal(koffi.registeredCallbackCount(), 0);

const reloadedBindings = load(stagedOverridePath);
assert.equal(isLoaded(), true);
assert.equal(realpathSync(getFfiBindings().libraryPath), realpathSync(stagedOverridePath));
assert.notStrictEqual(reloadedBindings, firstBindings);
assert.strictEqual(reloadedBindings.library, firstBindings.library);
assert.strictEqual(reloadedBindings.ffiFunctions, firstBindings.ffiFunctions);
assert.deepStrictEqual(Array.from(echo_bytes(new Uint8Array([4, 5, 6]))), [4, 5, 6]);
const reloadedStore = new Store(seed);
await reloadedStore.fetch_async(true);
reloadedStore.dispose();
assert.equal(koffi.registeredCallbackCount(), 1);
assert.equal(unload(), true);
assert.equal(isLoaded(), false);
assert.equal(koffi.registeredCallbackCount(), 0);

const copiedDir = mkdtempSync(join(process.cwd(), "copied-library-"));
const aliasDir = mkdtempSync(join(tmpdir(), "uniffi-manual-load-alias-"));
try {{
  const copiedPath = join(copiedDir, {1});
  copyFileSync(stagedOverridePath, copiedPath);

  const copiedBindings = load(copiedPath);
  assert.equal(isLoaded(), true);
  assert.equal(realpathSync(getFfiBindings().libraryPath), realpathSync(copiedPath));
  assert.notStrictEqual(copiedBindings, reloadedBindings);
  assert.notStrictEqual(copiedBindings.library, firstBindings.library);
  assert.notStrictEqual(copiedBindings.ffiFunctions, firstBindings.ffiFunctions);
  const copiedStore = new Store(seed);
  await copiedStore.fetch_async(true);
  copiedStore.dispose();
  assert.equal(koffi.registeredCallbackCount(), 1);
  assert.equal(unload(), true);
  assert.equal(isLoaded(), false);
  assert.equal(koffi.registeredCallbackCount(), 0);

  const aliasPath = join(aliasDir, {1});
  symlinkSync(stagedOverridePath, aliasPath);

  const aliasBindings = load(aliasPath);
  assert.equal(isLoaded(), true);
  assert.equal(realpathSync(getFfiBindings().libraryPath), realpathSync(stagedOverridePath));
  assert.notStrictEqual(aliasBindings, reloadedBindings);
  assert.notStrictEqual(aliasBindings.library, copiedBindings.library);
  assert.notStrictEqual(aliasBindings.ffiFunctions, copiedBindings.ffiFunctions);

  const canonicalBindings = load(stagedOverridePath);
  assert.strictEqual(canonicalBindings, aliasBindings);
  const aliasStore = new Store(seed);
  await aliasStore.fetch_async(true);
  aliasStore.dispose();
  assert.equal(koffi.registeredCallbackCount(), 1);
  assert.equal(unload(), true);
  assert.equal(isLoaded(), false);
  assert.equal(koffi.registeredCallbackCount(), 0);
}} finally {{
  rmSync(copiedDir, {{ recursive: true, force: true }});
  rmSync(aliasDir, {{ recursive: true, force: true }});
}}
"#,
            serde_json::to_string(expected_library_path.as_str())
                .expect("fixture library path should serialize"),
            serde_json::to_string(expected_library_filename)
                .expect("sibling library filename should serialize"),
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
            ..FixturePackageOptions::default()
        },
    );
    let package_dir = &generated.package_dir;
    let expected_target = generated
        .bundled_prebuild_target
        .clone()
        .expect("bundled fixture should report the generated host target");
    let expected_relative_path = generated
        .bundled_prebuild_path
        .as_ref()
        .and_then(|path| path.strip_prefix(package_dir).ok())
        .map(|path| path.as_str().to_string())
        .expect("bundled fixture should report the generated package-relative prebuild path");
    let staged_prebuild_path = generated
        .bundled_prebuild_path
        .as_ref()
        .expect("bundled fixture should report the staged prebuild path");
    fs::remove_file(staged_prebuild_path.as_std_path())
        .expect("negative bundled fixture should allow removing the staged prebuild");

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

async function withTimeout(promise, message) {
  let timeoutId;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timeoutId = setTimeout(() => reject(new Error(message)), 1_000);
      }),
    ]);
  } finally {
    clearTimeout(timeoutId);
  }
}

assert.equal(foreignFutureHandleCount(), 0);

const callbackLowererErrors = [];
const callbackStatus = createRustCallStatus();
const normalizedCallbackError = writeCallbackError(callbackStatus, "sync string failure", {
  lowerError(error) {
    callbackLowererErrors.push(error);
    assert.ok(error instanceof Error);
    assert.equal(error.message, "sync string failure");
    return EMPTY_RUST_BUFFER;
  },
  lowerString(message) {
    assert.fail(`unexpected lowerString call for lowered sync error: ${message}`);
  },
});
assert.equal(callbackStatus.code, CALL_ERROR);
assert.ok(normalizedCallbackError instanceof Error);
assert.equal(normalizedCallbackError.message, "sync string failure");
assert.deepStrictEqual(
  callbackLowererErrors.map((error) => error.message),
  ["sync string failure"],
);

const unexpectedCallbackMessages = [];
const unexpectedCallbackStatus = createRustCallStatus();
const unexpectedCallbackError = writeCallbackError(
  unexpectedCallbackStatus,
  "sync unexpected failure",
  {
    lowerError(error) {
      assert.ok(error instanceof Error);
      assert.equal(error.message, "sync unexpected failure");
      return null;
    },
    lowerString(message) {
      unexpectedCallbackMessages.push(message);
      return EMPTY_RUST_BUFFER;
    },
  },
);
assert.equal(unexpectedCallbackStatus.code, CALL_UNEXPECTED_ERROR);
assert.ok(unexpectedCallbackError instanceof Error);
assert.equal(unexpectedCallbackError.message, "sync unexpected failure");
assert.deepStrictEqual(unexpectedCallbackMessages, ["sync unexpected failure"]);

const runtimeLowererErrors = [];
const runtimeCompletion = deferred();
const runtimeRegistry = new UniffiCallbackRegistry({
  interfaceName: "RuntimeAsyncSink",
});
const runtimeHandle = runtimeRegistry.register({
  write(message) {
    assert.equal(message, "runtime-string-failure");
    return Promise.reject("runtime string failure");
  },
});

const runtimeFutureHandle = invokeAsyncCallbackMethod({
  registry: runtimeRegistry,
  handle: runtimeHandle,
  methodName: "write",
  args: ["runtime-string-failure"],
  callbackData: 99n,
  complete(callbackData, result) {
    runtimeCompletion.resolve({ callbackData, result });
  },
  lowerError(error) {
    runtimeLowererErrors.push(error);
    assert.ok(error instanceof Error);
    assert.equal(error.message, "runtime string failure");
    return EMPTY_RUST_BUFFER;
  },
  lowerString(message) {
    assert.fail(`unexpected lowerString call for lowered async error: ${message}`);
  },
});
assert.equal(typeof runtimeFutureHandle, "bigint");
assert.equal(foreignFutureHandleCount(), 1);
const completedRuntimeFailure = await withTimeout(
  runtimeCompletion.promise,
  "timed out waiting for async callback failure completion",
);
assert.equal(completedRuntimeFailure.callbackData, 99n);
assert.equal(completedRuntimeFailure.result.call_status.code, CALL_ERROR);
assert.equal(foreignFutureHandleCount(), 0);
assert.deepStrictEqual(
  runtimeLowererErrors.map((error) => error.message),
  ["runtime string failure"],
);
runtimeRegistry.remove(runtimeHandle);

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
