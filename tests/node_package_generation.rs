mod support;

use std::{collections::BTreeMap, fs};

use self::support::{
    FixturePackageOptions, build_fixture_cdylib, build_off_workspace_udl_fixture_cdylib,
    build_proc_macro_multi_component_cdylib, fixtures::fixture_spec, generate_fixture_package,
    generate_fixture_package_with_options, install_fixture_package_dependencies,
    load_fixture_component_interface, read_package_file_tree, remove_dir_all, run_node_script,
    temp_dir_path,
};
use serde_json::Value;
use uniffi_bindgen::interface::{Callable, ComponentInterface};
use uniffi_bindgen_node_js::{GenerateNodePackageOptions, generate_node_package};
use uniffi_meta::{clone_fn_symbol_name, free_fn_symbol_name};

fn read_generated_component_js(package_dir: &camino::Utf8PathBuf, namespace: &str) -> String {
    fs::read_to_string(package_dir.join(format!("{namespace}.js")).as_std_path())
        .expect("component js should be readable")
}

fn read_generated_component_dts(package_dir: &camino::Utf8PathBuf, namespace: &str) -> String {
    fs::read_to_string(package_dir.join(format!("{namespace}.d.ts")).as_std_path())
        .expect("component d.ts should be readable")
}

fn read_generated_component_ffi_js(package_dir: &camino::Utf8PathBuf, namespace: &str) -> String {
    fs::read_to_string(
        package_dir
            .join(format!("{namespace}-ffi.js"))
            .as_std_path(),
    )
    .expect("component ffi js should be readable")
}

fn extract_generated_block<'a>(contents: &'a str, start_marker: &str, end_marker: &str) -> &'a str {
    let start = contents
        .find(start_marker)
        .unwrap_or_else(|| panic!("generated source should contain block marker {start_marker}"));
    let end = contents[start..]
        .find(end_marker)
        .map(|offset| start + offset + end_marker.len())
        .unwrap_or_else(|| panic!("generated source should terminate block {start_marker}"));
    &contents[start..end]
}

fn assert_substrings_in_order(haystack: &str, expected: &[&str]) {
    let mut offset = 0;
    for needle in expected {
        let relative_index = haystack[offset..]
            .find(needle)
            .unwrap_or_else(|| panic!("expected to find {needle:?} after:\n{}", &haystack[..offset]));
        offset += relative_index + needle.len();
    }
}

fn extract_generated_ffi_integrity_block(ffi_js: &str) -> &str {
    let start = ffi_js
        .find("export const ffiIntegrity = Object.freeze({")
        .expect("generated ffi js should include ffiIntegrity metadata");
    let end = ffi_js[start..]
        .find("\n\nlet loadedBindings = null;")
        .map(|offset| start + offset)
        .expect("generated ffi js should terminate ffiIntegrity metadata before bindings state");
    &ffi_js[start..end]
}

fn parse_generated_checksums(ffi_js: &str) -> BTreeMap<String, u16> {
    let integrity_block = extract_generated_ffi_integrity_block(ffi_js);
    let checksums_start = integrity_block
        .find("  checksums: Object.freeze({\n")
        .expect("generated ffi js should include checksum integrity metadata");
    let checksums_body_start = checksums_start + "  checksums: Object.freeze({\n".len();
    let checksums_body_end = integrity_block[checksums_body_start..]
        .find("\n  }),")
        .map(|offset| checksums_body_start + offset)
        .expect("generated ffi js should close checksum integrity metadata");

    integrity_block[checksums_body_start..checksums_body_end]
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let trimmed = line.trim().trim_end_matches(',');
            let (name_json, expected) = trimmed
                .split_once(": ")
                .expect("generated checksum entries should have a name and value");
            let name = serde_json::from_str(name_json)
                .expect("generated checksum names should deserialize as JSON strings");
            let expected = expected
                .parse()
                .expect("generated checksum values should parse as u16");
            (name, expected)
        })
        .collect()
}

fn parse_generated_contract_version_function(ffi_js: &str) -> String {
    extract_generated_ffi_integrity_block(ffi_js)
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("contractVersionFunction: ")
                .map(|value| value.trim_end_matches(','))
        })
        .map(|value| {
            serde_json::from_str(value)
                .expect("generated contract version symbol should deserialize as JSON")
        })
        .expect("generated ffi js should include a contract version function symbol")
}

fn parse_generated_expected_contract_version(ffi_js: &str) -> u32 {
    extract_generated_ffi_integrity_block(ffi_js)
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("expectedContractVersion: ")
                .map(|value| value.trim_end_matches(','))
        })
        .map(|value| {
            value
                .parse()
                .expect("generated contract version should parse as u32")
        })
        .expect("generated ffi js should include an expected contract version")
}

fn async_scaffolding_symbols<T: Callable>(callable: &T, ci: &ComponentInterface) -> [String; 4] {
    [
        callable.ffi_rust_future_poll(ci),
        callable.ffi_rust_future_cancel(ci),
        callable.ffi_rust_future_complete(ci),
        callable.ffi_rust_future_free(ci),
    ]
}

#[test]
fn generates_basic_fixture_node_package_in_a_temp_directory() {
    let generated = generate_fixture_package("basic");
    let package_dir = &generated.package_dir;
    let spec = fixture_spec("basic");
    let namespace = &generated.built_fixture.namespace;
    let staged_library_relative_path = generated.staged_library_package_relative_path.to_string();

    for relative_path in [
        "package.json",
        "index.js",
        "index.d.ts",
        &format!("{namespace}.js"),
        &format!("{namespace}.d.ts"),
        &format!("{namespace}-ffi.js"),
        &format!("{namespace}-ffi.d.ts"),
        "runtime/errors.js",
        "runtime/ffi-types.js",
        "runtime/ffi-converters.js",
        "runtime/rust-call.js",
        "runtime/async-rust-call.js",
        "runtime/handle-map.js",
        "runtime/callbacks.js",
        "runtime/objects.js",
        &staged_library_relative_path,
    ] {
        let path = package_dir.join(relative_path);
        assert!(path.is_file(), "expected generated package file at {path}");
    }

    let mut expected_paths = spec.generated_package_relative_paths();
    expected_paths.push(staged_library_relative_path);
    expected_paths.sort();

    remove_dir_all(&generated.built_fixture.workspace_dir);
    assert_eq!(
        read_package_file_tree(package_dir)
            .into_keys()
            .collect::<Vec<_>>(),
        expected_paths,
        "unexpected generated package file layout"
    );
    remove_dir_all(package_dir);
}

#[test]
fn generated_default_package_stages_the_input_cdylib_at_the_package_root() {
    let generated = generate_fixture_package("basic");
    let package_dir = &generated.package_dir;
    let input_library_path = &generated.built_fixture.library_path;
    let library_filename = input_library_path
        .file_name()
        .expect("fixture library path should have a filename");
    let staged_library_path = generated
        .sibling_library_path
        .as_ref()
        .expect("default generation should stage the input cdylib at the package root");

    assert_eq!(
        staged_library_path,
        &package_dir.join(library_filename),
        "default generation should stage the input cdylib next to the generated JS files"
    );
    assert_eq!(
        generated.staged_library_package_relative_path,
        camino::Utf8PathBuf::from(library_filename),
        "default generation should record the staged root library path in ffi metadata"
    );
    assert!(
        generated.bundled_prebuild_path.is_none(),
        "default generation should not also stage a bundled prebuild"
    );
    assert_eq!(
        fs::read(input_library_path.as_std_path()).expect("fixture library should be readable"),
        fs::read(staged_library_path.as_std_path()).expect("staged library should be readable"),
        "default generation should stage the exact input cdylib contents"
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

#[test]
fn generated_basic_fixture_emits_typed_error_class_definitions() {
    let generated = generate_fixture_package("basic");
    let package_dir = &generated.package_dir;
    let namespace = &generated.built_fixture.namespace;
    let component_js = read_generated_component_js(package_dir, namespace);
    let component_dts = read_generated_component_dts(package_dir, namespace);

    assert_substrings_in_order(
        &component_js,
        &[
            "export class FixtureError extends globalThis.Error {",
            "export class FixtureErrorMissing extends FixtureError {",
            "this.name = \"FixtureErrorMissing\";",
            "export class FixtureErrorInvalidState extends FixtureError {",
            "this.name = \"FixtureErrorInvalidState\";",
            "this[\"message\"] = message;",
            "export class FixtureErrorParse extends FixtureError {",
            "this.name = \"FixtureErrorParse\";",
            "this[\"message\"] = message;",
        ],
    );
    assert_substrings_in_order(
        &component_dts,
        &[
            "export declare class FixtureError extends globalThis.Error {",
            "readonly tag: string;",
            "export declare class FixtureErrorMissing extends FixtureError {",
            "readonly tag: \"Missing\";",
            "export declare class FixtureErrorInvalidState extends FixtureError {",
            "readonly tag: \"InvalidState\";",
            "readonly \"message\": string;",
            "export declare class FixtureErrorParse extends FixtureError {",
            "readonly tag: \"Parse\";",
            "readonly \"message\": string;",
        ],
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

#[test]
fn infers_the_only_component_when_crate_name_is_omitted() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("infer-basic-package");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: None,
        out_dir: package_dir.clone(),
        package_name: Some(format!("{}-package", built_fixture.namespace)),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect("single-component library should not require --crate-name");

    assert!(
        package_dir.join("package.json").is_file(),
        "expected generated package manifest at {}",
        package_dir.join("package.json")
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn applies_rename_config_before_rendering_generated_bindings() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("rename-config-package");
    let manifest_dir = built_fixture
        .manifest_path
        .parent()
        .expect("fixture manifest path should have a parent directory");
    let rename_config_path = manifest_dir.join("uniffi.toml");

    fs::write(
        rename_config_path.as_std_path(),
        r#"
[bindings.node.rename]
"echo_byte_map.value" = "entries"
"#,
    )
    .expect("test should write a rename config next to the fixture manifest");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: Some(format!("{}-package", built_fixture.namespace)),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect("package generation should apply loader-resolved rename config");

    let component_js = fs::read_to_string(
        package_dir
            .join(format!("{}.js", built_fixture.namespace))
            .as_std_path(),
    )
    .expect("component js should be readable");
    let component_dts = fs::read_to_string(
        package_dir
            .join(format!("{}.d.ts", built_fixture.namespace))
            .as_std_path(),
    )
    .expect("component d.ts should be readable");

    assert!(
        component_js.contains("export function echo_byte_map(entries) {"),
        "unexpected component JS contents: {component_js}"
    );
    assert!(
        !component_js.contains("export function echo_byte_map(value) {"),
        "unexpected component JS contents: {component_js}"
    );
    assert!(
        component_dts.contains("export declare function echo_byte_map(entries: Map<string, Uint8Array>): Map<string, Uint8Array>;"),
        "unexpected component DTS contents: {component_dts}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn generated_async_helpers_use_loader_derived_uniffi_symbol_names() {
    let generated = generate_fixture_package("basic");
    let ffi_js =
        read_generated_component_ffi_js(&generated.package_dir, &generated.built_fixture.namespace);
    let async_runtime_js = fs::read_to_string(
        generated
            .package_dir
            .join("runtime")
            .join("async-rust-call.js")
            .as_std_path(),
    )
    .expect("generated async runtime should be readable");
    let ci = load_fixture_component_interface(&generated.built_fixture);
    let expected_symbols = ci
        .function_definitions()
        .iter()
        .filter(|function| function.is_async())
        .flat_map(|function| async_scaffolding_symbols(function, &ci))
        .chain(
            ci.object_definitions()
                .iter()
                .flat_map(|object| object.constructors())
                .filter(|constructor| constructor.is_async())
                .flat_map(|constructor| async_scaffolding_symbols(constructor, &ci)),
        )
        .chain(
            ci.object_definitions()
                .iter()
                .flat_map(|object| object.methods())
                .filter(|method| method.is_async())
                .flat_map(|method| async_scaffolding_symbols(method, &ci)),
        )
        .collect::<Vec<_>>();

    assert!(
        !expected_symbols.is_empty(),
        "basic fixture should expose at least one async callable for symbol verification"
    );
    for symbol in expected_symbols {
        assert!(
            ffi_js.contains(&symbol),
            "generated ffi js should include the UniFFI-provided async helper symbol {symbol}"
        );
    }
    assert!(
        async_runtime_js.contains("export const RUST_FUTURE_POLL_WAKE = 1;"),
        "generated async runtime should use the UniFFI 0.30/0.31 wake poll constant: {async_runtime_js}"
    );
    assert!(
        !async_runtime_js.contains("MAYBE_READY"),
        "generated async runtime should not reference the pre-0.30 MaybeReady poll name: {async_runtime_js}"
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(&generated.package_dir);
}

#[test]
fn generated_callback_clone_free_symbols_follow_full_module_paths() {
    let generated = generate_fixture_package("callbacks");
    let component_js = fs::read_to_string(
        generated
            .package_dir
            .join(format!("{}.js", generated.built_fixture.namespace))
            .as_std_path(),
    )
    .expect("component js should be readable");
    let ci = load_fixture_component_interface(&generated.built_fixture);

    let expected_symbols = ci
        .callback_interface_definitions()
        .iter()
        .flat_map(|callback_interface| {
            [
                clone_fn_symbol_name(callback_interface.module_path(), callback_interface.name()),
                free_fn_symbol_name(callback_interface.module_path(), callback_interface.name()),
            ]
        })
        .collect::<Vec<_>>();

    assert!(
        !expected_symbols.is_empty(),
        "callback fixture should expose callback interfaces for module_path symbol verification"
    );
    for symbol in expected_symbols {
        assert!(
            component_js.contains(&symbol),
            "generated component JS should include the UniFFI-provided callback handle symbol {symbol}"
        );
    }

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(&generated.package_dir);
}

#[test]
fn generated_callback_vtables_include_uniffi_clone_and_free_slots_before_methods() {
    let generated = generate_fixture_package("callbacks");
    let component_js = fs::read_to_string(
        generated
            .package_dir
            .join(format!("{}.js", generated.built_fixture.namespace))
            .as_std_path(),
    )
    .expect("component js should be readable");

    for (interface_name, expected_slots) in [
        (
            "AsyncLogSink",
            &[
                "uniffi_free: uniffiFree,",
                "uniffi_clone: uniffiClone,",
                "\"write\": writeCallback,",
                "\"write_fallible\": write_fallibleCallback,",
                "\"flush\": flushCallback,",
            ][..],
        ),
        (
            "LogCollector",
            &[
                "uniffi_free: uniffiFree,",
                "uniffi_clone: uniffiClone,",
                "\"log\": logCallback,",
            ][..],
        ),
        (
            "LogSink",
            &[
                "uniffi_free: uniffiFree,",
                "uniffi_clone: uniffiClone,",
                "\"write\": writeCallback,",
                "\"latest\": latestCallback,",
            ][..],
        ),
    ] {
        let block = extract_generated_block(
            &component_js,
            &format!(
                "koffi.encode(uniffiVtable, bindings.ffiStructs.VTableCallbackInterface{interface_name}, {{"
            ),
            "\n  });",
        );

        assert_substrings_in_order(block, expected_slots);
    }

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(&generated.package_dir);
}

#[test]
fn generated_async_callback_prototypes_match_uniffi_031_foreign_future_layout() {
    let generated = generate_fixture_package("callbacks");
    let ffi_js =
        read_generated_component_ffi_js(&generated.package_dir, &generated.built_fixture.namespace);

    for (method_name, expected_slots) in [
        (
            "CallbackInterfaceAsyncLogSinkMethod0",
            &[
                "\"CallbackInterfaceAsyncLogSinkMethod0\"",
                "\"uint64_t\"",
                "ffiTypes.RustBuffer",
                "koffi.pointer(ffiCallbacks.ForeignFutureCompleteRustBuffer)",
                "\"uint64_t\"",
                "koffi.pointer(ffiStructs.ForeignFutureDroppedCallbackStruct)",
            ][..],
        ),
        (
            "CallbackInterfaceAsyncLogSinkMethod1",
            &[
                "\"CallbackInterfaceAsyncLogSinkMethod1\"",
                "\"uint64_t\"",
                "ffiTypes.RustBuffer",
                "koffi.pointer(ffiCallbacks.ForeignFutureCompleteRustBuffer)",
                "\"uint64_t\"",
                "koffi.pointer(ffiStructs.ForeignFutureDroppedCallbackStruct)",
            ][..],
        ),
        (
            "CallbackInterfaceAsyncLogSinkMethod2",
            &[
                "\"CallbackInterfaceAsyncLogSinkMethod2\"",
                "\"uint64_t\"",
                "koffi.pointer(ffiCallbacks.ForeignFutureCompleteVoid)",
                "\"uint64_t\"",
                "koffi.pointer(ffiStructs.ForeignFutureDroppedCallbackStruct)",
            ][..],
        ),
    ] {
        let block = extract_generated_block(
            &ffi_js,
            &format!("ffiCallbacks.{method_name} = defineCallbackPrototype("),
            "]);",
        );
        assert_substrings_in_order(block, expected_slots);
    }

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(&generated.package_dir);
}

#[test]
fn generated_object_factories_include_new_abi_clone_and_free_paths() {
    let generated = generate_fixture_package("basic");
    let component_js = fs::read_to_string(
        generated
            .package_dir
            .join(format!("{}.js", generated.built_fixture.namespace))
            .as_std_path(),
    )
    .expect("component js should be readable");

    for (type_name, clone_symbol, free_symbol) in [
        (
            "Config",
            "uniffi_fixture_basic_fn_clone_config",
            "uniffi_fixture_basic_fn_free_config",
        ),
        (
            "Reader",
            "uniffi_fixture_basic_fn_clone_reader",
            "uniffi_fixture_basic_fn_free_reader",
        ),
        (
            "ReaderBuilder",
            "uniffi_fixture_basic_fn_clone_readerbuilder",
            "uniffi_fixture_basic_fn_free_readerbuilder",
        ),
        (
            "Store",
            "uniffi_fixture_basic_fn_clone_store",
            "uniffi_fixture_basic_fn_free_store",
        ),
    ] {
        let block = extract_generated_block(
            &component_js,
            &format!("const uniffi{type_name}ObjectFactory = createObjectFactory({{"),
            "\n});",
        );

        assert_substrings_in_order(
            block,
            &[
                "cloneFreeUsesUniffiHandle: true,",
                "cloneHandleGeneric(handle) {",
                &format!("ffiFunctions.{clone_symbol}_generic_abi(handle, status)"),
                "cloneHandleRawExternal(handle) {",
                &format!("\"{clone_symbol}:raw-external\""),
                &format!("\"{clone_symbol}\""),
                "cloneHandle(handle) {",
                &format!("ffiFunctions.{clone_symbol}(handle, status)"),
                "freeHandleGeneric(handle) {",
                &format!("ffiFunctions.{free_symbol}_generic_abi(handle, status)"),
                "freeHandleRawExternal(handle) {",
                &format!("\"{free_symbol}:raw-external\""),
                &format!("\"{free_symbol}\""),
                "freeHandle(handle) {",
                &format!("ffiFunctions.{free_symbol}(handle, status)"),
            ],
        );
    }

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(&generated.package_dir);
}

#[test]
fn defaults_package_name_to_the_selected_component_namespace() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("default-package-name");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: None,
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect("package generation should default the npm package name from the namespace");

    let package_json: Value = serde_json::from_str(
        &fs::read_to_string(package_dir.join("package.json").as_std_path())
            .expect("package.json should be readable"),
    )
    .expect("package.json should parse");

    assert_eq!(
        package_json.get("name").and_then(Value::as_str),
        Some(built_fixture.namespace.as_str()),
        "unexpected package.json contents: {package_json:#}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn reports_available_crate_names_when_a_library_contains_multiple_components() {
    let built_fixture = build_proc_macro_multi_component_cdylib();
    let package_dir = temp_dir_path("multi-component-package");

    let error = generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: None,
        crate_name: None,
        out_dir: package_dir.clone(),
        package_name: None,
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect_err("multi-component libraries should require an explicit crate selector");

    assert!(
        error.to_string().contains("the library contains multiple UniFFI components"),
        "unexpected error: {error:#}"
    );
    assert!(
        error.to_string().contains("re-run with --crate-name"),
        "unexpected error: {error:#}"
    );
    assert!(
        error.to_string().contains("available crate names:"),
        "unexpected error: {error:#}"
    );
    for crate_name in &built_fixture.available_crate_names {
        assert!(
            error.to_string().contains(crate_name),
            "unexpected error: {error:#}"
        );
    }

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn selects_the_requested_component_when_crate_name_is_provided() {
    let built_fixture = build_proc_macro_multi_component_cdylib();
    let package_dir = temp_dir_path("multi-component-selected-package");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: None,
        crate_name: Some("component-alpha".to_string()),
        out_dir: package_dir.clone(),
        package_name: None,
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect("crate-name selection should allow generation for the chosen component");

    let package_json: Value = serde_json::from_str(
        &fs::read_to_string(package_dir.join("package.json").as_std_path())
            .expect("package.json should be readable"),
    )
    .expect("package.json should parse");

    assert!(
        package_dir.join("component_alpha.js").is_file(),
        "expected selected component JS at {}",
        package_dir.join("component_alpha.js")
    );
    assert!(
        package_dir.join("component_alpha.d.ts").is_file(),
        "expected selected component DTS at {}",
        package_dir.join("component_alpha.d.ts")
    );
    assert!(
        package_dir.join("component_alpha-ffi.js").is_file(),
        "expected selected component FFI JS at {}",
        package_dir.join("component_alpha-ffi.js")
    );
    assert!(
        !package_dir.join("component_beta.js").exists(),
        "unexpected non-selected component JS at {}",
        package_dir.join("component_beta.js")
    );
    assert_eq!(
        package_json.get("name").and_then(Value::as_str),
        Some("component_alpha"),
        "unexpected package.json contents: {package_json:#}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn rerunning_generation_into_fresh_empty_directories_is_deterministic() {
    let built_fixture = build_fixture_cdylib("basic");
    let first_package_dir = temp_dir_path("deterministic-package-first");
    let second_package_dir = temp_dir_path("deterministic-package-second");

    for out_dir in [&first_package_dir, &second_package_dir] {
        generate_node_package(GenerateNodePackageOptions {
            lib_source: built_fixture.library_path.clone(),
            manifest_path: Some(built_fixture.manifest_path.clone()),
            crate_name: Some(built_fixture.crate_name.clone()),
            out_dir: out_dir.clone(),
            package_name: Some(format!("{}-package", built_fixture.namespace)),
            node_engine: None,
            bundled_prebuilds: false,
            manual_load: false,
        })
        .expect("package generation should be deterministic across fresh output directories");
    }

    assert_eq!(
        read_package_file_tree(&first_package_dir),
        read_package_file_tree(&second_package_dir),
        "expected byte-for-byte identical package output across fresh directories"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&first_package_dir);
    remove_dir_all(&second_package_dir);
}

#[test]
fn package_name_override_wins_over_the_namespace_default() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("override-package-name");
    let package_name = "custom-generated-package";

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: Some(package_name.to_string()),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect("package generation should keep the explicit package-name override");

    let package_json: Value = serde_json::from_str(
        &fs::read_to_string(package_dir.join("package.json").as_std_path())
            .expect("package.json should be readable"),
    )
    .expect("package.json should parse");

    assert_eq!(
        package_json.get("name").and_then(Value::as_str),
        Some(package_name),
        "unexpected package.json contents: {package_json:#}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn node_engine_override_is_written_to_package_json() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("override-node-engine");
    let node_engine = ">=20.11.0";

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: None,
        node_engine: Some(node_engine.to_string()),
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect("package generation should keep the explicit node-engine override");

    let package_json: Value = serde_json::from_str(
        &fs::read_to_string(package_dir.join("package.json").as_std_path())
            .expect("package.json should be readable"),
    )
    .expect("package.json should parse");

    assert_eq!(
        package_json
            .get("engines")
            .and_then(Value::as_object)
            .and_then(|engines| engines.get("node"))
            .and_then(Value::as_str),
        Some(node_engine),
        "unexpected package.json contents: {package_json:#}"
    );
    assert_eq!(
        package_json.get("main").and_then(Value::as_str),
        Some("./index.js"),
        "unexpected package.json contents: {package_json:#}"
    );
    assert_eq!(
        package_json.get("types").and_then(Value::as_str),
        Some("./index.d.ts"),
        "unexpected package.json contents: {package_json:#}"
    );
    assert_eq!(
        package_json
            .get("exports")
            .and_then(Value::as_object)
            .and_then(|exports| exports.get("."))
            .and_then(Value::as_object)
            .and_then(|root_export| root_export.get("default"))
            .and_then(Value::as_str),
        Some("./index.js"),
        "unexpected package.json contents: {package_json:#}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn bundled_prebuilds_option_emits_bundled_loader_metadata() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("bundled-prebuild-loader");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: None,
        node_engine: None,
        bundled_prebuilds: true,
        manual_load: false,
    })
    .expect("package generation should keep bundled-prebuild loader support");

    let ffi_js = fs::read_to_string(
        package_dir
            .join(format!("{}-ffi.js", built_fixture.namespace))
            .as_std_path(),
    )
    .expect("component ffi js should be readable");

    assert!(
        ffi_js.contains("bundledPrebuilds: true"),
        "unexpected component FFI JS contents: {ffi_js}"
    );
    assert!(
        ffi_js.contains(&format!(
            "stagedLibraryFileName: {:?},",
            built_fixture
                .library_path
                .file_name()
                .expect("fixture library path should have a filename")
        )),
        "unexpected component FFI JS contents: {ffi_js}"
    );
    assert!(
        ffi_js.contains("const filename = ffiMetadata.stagedLibraryFileName;"),
        "unexpected component FFI JS contents: {ffi_js}"
    );
    assert!(
        ffi_js.contains("packageRelativePath: `prebuilds/${target}/${filename}`,"),
        "unexpected component FFI JS contents: {ffi_js}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn generated_bundled_package_stages_the_input_cdylib_under_prebuilds() {
    let generated = generate_fixture_package_with_options(
        "basic",
        FixturePackageOptions {
            bundled_prebuilds: true,
            manual_load: false,
        },
    );
    let package_dir = &generated.package_dir;
    let input_library_path = &generated.built_fixture.library_path;
    let library_filename = input_library_path
        .file_name()
        .expect("fixture library path should have a filename");
    let bundled_target = generated
        .bundled_prebuild_target
        .as_ref()
        .expect("bundled generation should record the staged host target");
    let expected_relative_path = format!("prebuilds/{bundled_target}/{library_filename}");
    let bundled_prebuild_path = generated
        .bundled_prebuild_path
        .as_ref()
        .expect("bundled generation should stage the input cdylib under prebuilds/");

    assert!(
        generated.sibling_library_path.is_none(),
        "bundled generation should not also stage a root-level sibling library"
    );
    assert_eq!(
        generated.staged_library_package_relative_path.as_str(),
        expected_relative_path,
        "bundled generation should record the staged host-target prebuild path in ffi metadata"
    );
    assert_eq!(
        bundled_prebuild_path,
        &package_dir.join(&generated.staged_library_package_relative_path),
        "bundled generation should stage the input cdylib inside prebuilds/<target>/"
    );
    assert_eq!(
        fs::read(input_library_path.as_std_path()).expect("fixture library should be readable"),
        fs::read(bundled_prebuild_path.as_std_path()).expect("staged prebuild should be readable"),
        "bundled generation should stage the exact input cdylib contents"
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

#[test]
fn bundled_package_resolves_the_staged_host_target_directory_at_runtime() {
    let generated = generate_fixture_package_with_options(
        "basic",
        FixturePackageOptions {
            bundled_prebuilds: true,
            manual_load: false,
        },
    );
    let package_dir = &generated.package_dir;
    let bundled_target = generated
        .bundled_prebuild_target
        .as_ref()
        .expect("bundled generation should record the staged host target");
    let staged_prebuild_path = generated
        .bundled_prebuild_path
        .as_ref()
        .expect("bundled generation should stage a host-target prebuild");
    let expected_relative_path = generated.staged_library_package_relative_path.to_string();
    let expected_library_filename = generated
        .built_fixture
        .library_path
        .file_name()
        .expect("fixture library path should have a filename")
        .to_string();
    assert_eq!(
        staged_prebuild_path,
        &package_dir.join(&generated.staged_library_package_relative_path),
        "bundled generation should stage the prebuild at the metadata-reported package path"
    );

    install_fixture_package_dependencies(package_dir);
    run_node_script(
        package_dir,
        "bundled-target-resolution.mjs",
        &format!(
            r#"
import assert from "node:assert/strict";
import "./index.js";
import {{ getFfiBindings }} from "./fixture-ffi.js";

const expectedTarget = {expected_target_json};
const expectedFilename = {expected_filename_json};
const expectedPackageRelativePath = {expected_relative_path_json};
const bindings = getFfiBindings();

assert.equal(
  bindings.packageRelativePath,
  expectedPackageRelativePath,
  "bundled bindings should resolve the staged host-target path inside the package",
);
assert.equal(
  bindings.packageRelativePath,
  `prebuilds/${{expectedTarget}}/${{expectedFilename}}`,
  "bundled bindings should use the discovered host target directory name",
);
"#,
            expected_target_json =
                serde_json::to_string(bundled_target).expect("target id should serialize"),
            expected_filename_json = serde_json::to_string(&expected_library_filename)
                .expect("library filename should serialize"),
            expected_relative_path_json = serde_json::to_string(&expected_relative_path)
                .expect("package-relative path should serialize"),
        ),
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

#[test]
fn manual_load_option_exports_manual_lifecycle_helpers() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("manual-load-helpers");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: None,
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: true,
    })
    .expect("package generation should keep manual-load support");

    let component_js = fs::read_to_string(
        package_dir
            .join(format!("{}.js", built_fixture.namespace))
            .as_std_path(),
    )
    .expect("component js should be readable");
    let ffi_js = fs::read_to_string(
        package_dir
            .join(format!("{}-ffi.js", built_fixture.namespace))
            .as_std_path(),
    )
    .expect("component ffi js should be readable");

    assert!(
        component_js.contains(&format!(
            "export {{ load, unload }} from \"./{}-ffi.js\";",
            built_fixture.namespace
        )),
        "unexpected component JS contents: {component_js}"
    );
    assert!(
        ffi_js.contains("manualLoad: true"),
        "unexpected component FFI JS contents: {ffi_js}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn manual_load_loader_codegen_is_reentrant_for_the_same_canonical_path() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("manual-load-loader-reentrancy");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: None,
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: true,
    })
    .expect("package generation should emit the manual-load reentrancy guard");

    let ffi_js = fs::read_to_string(
        package_dir
            .join(format!("{}-ffi.js", built_fixture.namespace))
            .as_std_path(),
    )
    .expect("component ffi js should be readable");

    for expected in [
        "if (libraryPath != null) {",
        "function canonicalizeExistingLibraryPath(libraryPath) {",
        "const canonicalLibraryPath = canonicalizeExistingLibraryPath(resolvedLibraryPath);",
        "if (loadedBindings.libraryPath === canonicalLibraryPath) {",
        "return loadedBindings;",
        "Call unload() before loading a different library path.",
    ] {
        assert!(
            ffi_js.contains(expected),
            "unexpected component FFI JS contents: {ffi_js}"
        );
    }

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn loader_codegen_caches_binding_core_by_canonical_library_path() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("binding-core-cache");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: None,
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: true,
    })
    .expect("package generation should emit binding-core cache handling");

    let ffi_js = fs::read_to_string(
        package_dir
            .join(format!("{}-ffi.js", built_fixture.namespace))
            .as_std_path(),
    )
    .expect("component ffi js should be readable");

    for expected in [
        "let cachedBindingCore = null;",
        "let cachedLibraryPath = null;",
        "cachedLibraryPath === canonicalLibraryPath",
        "cachedBindingCore.library.unload();",
        "clearBindingCoreCache();",
        "bindingCore = cacheBindingCore(canonicalLibraryPath, bindings);",
    ] {
        assert!(
            ffi_js.contains(expected),
            "unexpected component FFI JS contents: {ffi_js}"
        );
    }

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn generates_udl_backed_callback_fixture_when_manifest_path_is_provided() {
    let built_fixture = build_fixture_cdylib("callbacks");
    let package_dir = temp_dir_path("callbacks-manifest-path-package");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: Some(format!("{}-package", built_fixture.namespace)),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect("UDL-backed library should load with --manifest-path");

    assert!(
        package_dir.join("package.json").is_file(),
        "expected generated package manifest at {}",
        package_dir.join("package.json")
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn generates_off_workspace_udl_fixture_when_manifest_path_is_provided() {
    let built_fixture = build_off_workspace_udl_fixture_cdylib();
    let package_dir = temp_dir_path("off-workspace-udl-manifest-path-package");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: Some(format!("{}-package", built_fixture.namespace)),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect("off-workspace UDL fixture should load with --manifest-path");

    assert!(
        package_dir.join("package.json").is_file(),
        "expected generated package manifest at {}",
        package_dir.join("package.json")
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn rejects_udl_backed_generation_without_manifest_path_when_loader_cannot_resolve_udl() {
    let built_fixture = build_off_workspace_udl_fixture_cdylib();
    let package_dir = temp_dir_path("off-workspace-udl-missing-manifest-path-package");

    let error = generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: None,
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: Some(format!("{}-package", built_fixture.namespace)),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect_err("UDL-backed generation without --manifest-path should fail clearly");
    let error_message = format!("{error:#}");

    assert!(
        error_message.contains(&format!(
            "failed to load UniFFI metadata from '{}'",
            built_fixture.library_path
        )),
        "unexpected error: {error_message}"
    );
    assert!(
        error_message.contains(&format!(
            "UDL file {:?} not found for crate {:?}",
            built_fixture.namespace, built_fixture.crate_name
        )),
        "unexpected error: {error_message}"
    );
    assert!(
        !package_dir.join("package.json").exists(),
        "generation should fail before writing package output to {}",
        package_dir
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn generated_ffi_integrity_uses_loader_derived_checksums() {
    let built_fixture = build_fixture_cdylib("callbacks");
    let component_interface = load_fixture_component_interface(&built_fixture);
    let package_dir = temp_dir_path("loader-derived-checksums");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: Some(format!("{}-package", built_fixture.namespace)),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect("package generation should preserve loader-derived checksum metadata");

    let expected_checksums = component_interface
        .iter_checksums()
        .collect::<BTreeMap<_, _>>();
    let actual_checksums = parse_generated_checksums(&read_generated_component_ffi_js(
        &package_dir,
        &built_fixture.namespace,
    ));

    assert_eq!(
        actual_checksums, expected_checksums,
        "generated ffi integrity metadata should match UniFFI loader-derived checksums"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn generated_ffi_integrity_uses_loader_derived_contract_version() {
    let built_fixture = build_fixture_cdylib("basic");
    let component_interface = load_fixture_component_interface(&built_fixture);
    let package_dir = temp_dir_path("loader-derived-contract-version");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: Some(format!("{}-package", built_fixture.namespace)),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect("package generation should preserve loader-derived contract version metadata");

    let ffi_js = read_generated_component_ffi_js(&package_dir, &built_fixture.namespace);

    assert_eq!(
        parse_generated_contract_version_function(&ffi_js),
        component_interface.ffi_uniffi_contract_version().name(),
        "generated ffi integrity metadata should use the loader-derived contract version symbol"
    );
    assert_eq!(
        parse_generated_expected_contract_version(&ffi_js),
        component_interface.uniffi_contract_version(),
        "generated ffi integrity metadata should use the loader-derived contract version value"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn rejects_missing_library_source_from_programmatic_entrypoint() {
    let package_dir = temp_dir_path("missing-library-package");
    let missing_library_path = package_dir.join("missing-library.so");

    let error = generate_node_package(GenerateNodePackageOptions {
        lib_source: missing_library_path.clone(),
        manifest_path: None,
        crate_name: None,
        out_dir: package_dir.clone(),
        package_name: Some("missing-library-package".to_string()),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect_err("missing library path should be rejected by the v2 entrypoint");

    assert!(
        error.to_string().contains(&format!(
            "built UniFFI cdylib '{}' does not exist",
            missing_library_path
        )),
        "unexpected error: {error:#}"
    );

    remove_dir_all(&package_dir);
}

#[test]
fn rejects_file_out_dir_from_programmatic_entrypoint() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("file-out-dir-package");
    std::fs::write(package_dir.as_std_path(), "not a directory")
        .expect("test should create a file-backed out-dir path");

    let error = generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: Some(format!("{}-package", built_fixture.namespace)),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect_err("file-backed out-dir should be rejected by the v2 entrypoint");

    assert!(
        error.to_string().contains(&format!(
            "--out-dir '{}' exists but is not a directory",
            package_dir
        )),
        "unexpected error: {error:#}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    std::fs::remove_file(package_dir.as_std_path())
        .expect("test should remove the file-backed out-dir path");
}

#[test]
fn rejects_directory_manifest_path_from_programmatic_entrypoint() {
    let built_fixture = build_fixture_cdylib("callbacks");
    let package_dir = temp_dir_path("directory-manifest-path-package");

    let error = generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.workspace_dir.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: Some(format!("{}-package", built_fixture.namespace)),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect_err("directory manifest path should be rejected by the v2 entrypoint");

    assert!(
        error.to_string().contains(&format!(
            "--manifest-path '{}' must point to a Cargo.toml file",
            built_fixture.workspace_dir
        )),
        "unexpected error: {error:#}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn rejects_missing_manifest_path_from_programmatic_entrypoint() {
    let built_fixture = build_fixture_cdylib("callbacks");
    let package_dir = temp_dir_path("missing-manifest-path-package");
    let missing_manifest_path = built_fixture.workspace_dir.join("missing").join("Cargo.toml");

    let error = generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(missing_manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: Some(format!("{}-package", built_fixture.namespace)),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect_err("missing manifest path should be rejected by the v2 entrypoint");

    assert!(
        error
            .to_string()
            .contains(&format!("manifest path '{}' does not exist", missing_manifest_path)),
        "unexpected error: {error:#}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn installs_fixture_package_npm_dependencies_in_a_temp_directory() {
    let generated = generate_fixture_package("basic");
    let package_dir = &generated.package_dir;

    install_fixture_package_dependencies(package_dir);

    let installed_koffi_manifest = package_dir
        .join("node_modules")
        .join("koffi")
        .join("package.json");
    assert!(
        installed_koffi_manifest.is_file(),
        "expected installed koffi package manifest at {}",
        installed_koffi_manifest
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

#[test]
fn generates_callback_fixture_package_with_expected_files_and_local_koffi_fixture() {
    let generated = generate_fixture_package("callbacks");
    let package_dir = &generated.package_dir;
    let spec = fixture_spec("callbacks");
    let staged_library_relative_path = generated.staged_library_package_relative_path.to_string();

    for relative_path in spec
        .generated_package_relative_paths()
        .into_iter()
        .chain(std::iter::once(staged_library_relative_path))
    {
        let path = package_dir.join(&relative_path);
        assert!(path.is_file(), "expected generated package file at {path}");
    }

    install_fixture_package_dependencies(package_dir);

    let installed_koffi_manifest = package_dir
        .join("node_modules")
        .join("koffi")
        .join("package.json");
    assert!(
        installed_koffi_manifest.is_file(),
        "expected installed koffi package manifest at {}",
        installed_koffi_manifest
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

#[test]
fn generates_bundled_basic_fixture_package_with_only_a_host_prebuild() {
    let generated = generate_fixture_package_with_options(
        "basic",
        FixturePackageOptions {
            bundled_prebuilds: true,
            ..FixturePackageOptions::default()
        },
    );
    let package_dir = &generated.package_dir;
    let namespace = &generated.built_fixture.namespace;
    let bundled_target = generated
        .bundled_prebuild_target
        .as_deref()
        .expect("bundled-mode fixture package should record the staged target");
    let bundled_library_path = generated
        .bundled_prebuild_path
        .as_ref()
        .expect("bundled-mode fixture package should record the staged prebuild path");
    let expected_library_filename = generated
        .built_fixture
        .library_path
        .file_name()
        .expect("fixture library path should have a filename");
    let bundled_library_relative_path = generated.staged_library_package_relative_path.to_string();
    let root_library_path = package_dir.join(expected_library_filename);

    assert!(
        generated.sibling_library_path.is_none(),
        "bundled-mode helper should not stage a sibling library at the package root"
    );
    assert_eq!(
        bundled_library_relative_path,
        format!("prebuilds/{bundled_target}/{expected_library_filename}"),
        "bundled-mode fixture package should report the staged host prebuild through ffi metadata"
    );

    for relative_path in [
        "package.json",
        "index.js",
        "index.d.ts",
        &format!("{namespace}.js"),
        &format!("{namespace}.d.ts"),
        &format!("{namespace}-ffi.js"),
        &format!("{namespace}-ffi.d.ts"),
        "runtime/errors.js",
        "runtime/ffi-types.js",
        "runtime/ffi-converters.js",
        "runtime/rust-call.js",
        "runtime/async-rust-call.js",
        "runtime/handle-map.js",
        "runtime/callbacks.js",
        "runtime/objects.js",
        &bundled_library_relative_path,
    ] {
        let path = package_dir.join(relative_path);
        assert!(path.is_file(), "expected generated package file at {path}");
    }

    assert!(
        !root_library_path.exists(),
        "bundled-mode package should not stage a root-level sibling library at {root_library_path}"
    );
    assert_eq!(
        bundled_library_path,
        &package_dir.join(&bundled_library_relative_path),
        "generator should stage the host library at the expected bundled-prebuild path"
    );

    let mut expected_paths = fixture_spec("basic").generated_package_relative_paths();
    expected_paths.push(bundled_library_relative_path.clone());
    expected_paths.sort();
    assert_eq!(
        read_package_file_tree(package_dir)
            .into_keys()
            .collect::<Vec<_>>(),
        expected_paths,
        "unexpected bundled package file layout"
    );

    install_fixture_package_dependencies(package_dir);

    let installed_koffi_manifest = package_dir
        .join("node_modules")
        .join("koffi")
        .join("package.json");
    assert!(
        installed_koffi_manifest.is_file(),
        "expected installed koffi package manifest at {}",
        installed_koffi_manifest
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}
