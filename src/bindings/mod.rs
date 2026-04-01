use crate::node_v2::package_layout::GeneratedPackageLayout;
use anyhow::Result;
use askama::Template;
use camino::Utf8PathBuf;

mod api;
mod ffi;
mod ffi_ir;
mod package_writer;
mod templates;

pub(crate) use self::package_writer::{ComponentJsImports, write_generated_package};

type TemplateRenderResult = std::result::Result<String, askama::Error>;
type RuntimeTemplateRenderer = fn() -> TemplateRenderResult;

struct RuntimeModuleTemplateSet {
    stem: &'static str,
    js: RuntimeTemplateRenderer,
    dts: RuntimeTemplateRenderer,
}

fn runtime_files(layout: &GeneratedPackageLayout) -> Result<Vec<(Utf8PathBuf, String)>> {
    runtime_file_contents()?
        .into_iter()
        .map(|(file_name, contents)| Ok((layout.runtime_path(&file_name), contents)))
        .collect::<Result<Vec<_>>>()
}

fn rendered_runtime_file(
    file_name: String,
    contents: TemplateRenderResult,
) -> Result<(String, String)> {
    Ok((file_name, contents?))
}

fn runtime_file_contents() -> Result<Vec<(String, String)>> {
    let mut files = Vec::with_capacity(RUNTIME_MODULE_TEMPLATES.len() * 2);
    for templates in RUNTIME_MODULE_TEMPLATES {
        files.extend(render_runtime_module_files(templates)?);
    }
    Ok(files)
}

fn render_runtime_module_files(
    templates: &RuntimeModuleTemplateSet,
) -> Result<[(String, String); 2]> {
    Ok([
        rendered_runtime_file(format!("{}.js", templates.stem), (templates.js)())?,
        rendered_runtime_file(format!("{}.d.ts", templates.stem), (templates.dts)())?,
    ])
}

const RUNTIME_MODULE_TEMPLATES: &[RuntimeModuleTemplateSet] = &[
    RuntimeModuleTemplateSet {
        stem: "errors",
        js: render_runtime_errors_js,
        dts: render_runtime_errors_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "ffi-types",
        js: render_runtime_ffi_types_js,
        dts: render_runtime_ffi_types_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "ffi-converters",
        js: render_runtime_ffi_converters_js,
        dts: render_runtime_ffi_converters_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "rust-call",
        js: render_runtime_rust_call_js,
        dts: render_runtime_rust_call_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "async-rust-call",
        js: render_runtime_async_rust_call_js,
        dts: render_runtime_async_rust_call_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "handle-map",
        js: render_runtime_handle_map_js,
        dts: render_runtime_handle_map_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "callbacks",
        js: render_runtime_callbacks_js,
        dts: render_runtime_callbacks_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "objects",
        js: render_runtime_objects_js,
        dts: render_runtime_objects_dts,
    },
];

fn render_runtime_errors_js() -> TemplateRenderResult {
    RuntimeErrorsJsTemplate {}.render()
}

fn render_runtime_errors_dts() -> TemplateRenderResult {
    RuntimeErrorsDtsTemplate {}.render()
}

fn render_runtime_ffi_types_js() -> TemplateRenderResult {
    RuntimeFfiTypesJsTemplate {}.render()
}

fn render_runtime_ffi_types_dts() -> TemplateRenderResult {
    RuntimeFfiTypesDtsTemplate {}.render()
}

fn render_runtime_ffi_converters_js() -> TemplateRenderResult {
    RuntimeFfiConvertersJsTemplate {}.render()
}

fn render_runtime_ffi_converters_dts() -> TemplateRenderResult {
    RuntimeFfiConvertersDtsTemplate {}.render()
}

fn render_runtime_rust_call_js() -> TemplateRenderResult {
    RuntimeRustCallJsTemplate {}.render()
}

fn render_runtime_rust_call_dts() -> TemplateRenderResult {
    RuntimeRustCallDtsTemplate {}.render()
}

fn render_runtime_async_rust_call_js() -> TemplateRenderResult {
    RuntimeAsyncRustCallJsTemplate {}.render()
}

fn render_runtime_async_rust_call_dts() -> TemplateRenderResult {
    RuntimeAsyncRustCallDtsTemplate {}.render()
}

fn render_runtime_handle_map_js() -> TemplateRenderResult {
    RuntimeHandleMapJsTemplate {}.render()
}

fn render_runtime_handle_map_dts() -> TemplateRenderResult {
    RuntimeHandleMapDtsTemplate {}.render()
}

fn render_runtime_callbacks_js() -> TemplateRenderResult {
    RuntimeCallbacksJsTemplate {}.render()
}

fn render_runtime_callbacks_dts() -> TemplateRenderResult {
    RuntimeCallbacksDtsTemplate {}.render()
}

fn render_runtime_objects_js() -> TemplateRenderResult {
    RuntimeObjectsJsTemplate {}.render()
}

fn render_runtime_objects_dts() -> TemplateRenderResult {
    RuntimeObjectsDtsTemplate {}.render()
}

#[derive(Template)]
#[template(path = "runtime/errors.js.j2", escape = "none")]
struct RuntimeErrorsJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/errors.d.ts.j2", escape = "none")]
struct RuntimeErrorsDtsTemplate {}

#[derive(Template)]
#[template(path = "runtime/ffi-types.js.j2", escape = "none")]
struct RuntimeFfiTypesJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/ffi-types.d.ts.j2", escape = "none")]
struct RuntimeFfiTypesDtsTemplate {}

#[derive(Template)]
#[template(path = "runtime/ffi-converters.js.j2", escape = "none")]
struct RuntimeFfiConvertersJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/ffi-converters.d.ts.j2", escape = "none")]
struct RuntimeFfiConvertersDtsTemplate {}

#[derive(Template)]
#[template(path = "runtime/rust-call.js.j2", escape = "none")]
struct RuntimeRustCallJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/rust-call.d.ts.j2", escape = "none")]
struct RuntimeRustCallDtsTemplate {}

#[derive(Template)]
#[template(path = "runtime/async-rust-call.js.j2", escape = "none")]
struct RuntimeAsyncRustCallJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/async-rust-call.d.ts.j2", escape = "none")]
struct RuntimeAsyncRustCallDtsTemplate {}

#[derive(Template)]
#[template(path = "runtime/handle-map.js.j2", escape = "none")]
struct RuntimeHandleMapJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/handle-map.d.ts.j2", escape = "none")]
struct RuntimeHandleMapDtsTemplate {}

#[derive(Template)]
#[template(path = "runtime/callbacks.js.j2", escape = "none")]
struct RuntimeCallbacksJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/callbacks.d.ts.j2", escape = "none")]
struct RuntimeCallbacksDtsTemplate {}

#[derive(Template)]
#[template(path = "runtime/objects.js.j2", escape = "none")]
struct RuntimeObjectsJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/objects.d.ts.j2", escape = "none")]
struct RuntimeObjectsDtsTemplate {}

// GENERATED CODE
#[cfg(test)]
mod tests {
    use super::write_generated_package;
    use std::{
        env, fs, process,
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::Result;
    use camino::{Utf8Path, Utf8PathBuf};
    use uniffi_bindgen::{Component, interface::ComponentInterface};

    use crate::node_v2::config::{
        NodeBindingCliOverrides, NodeBindingGeneratorConfig, parse_node_binding_config,
    };

    fn component_with_namespace(namespace: &str) -> Component<NodeBindingGeneratorConfig> {
        Component {
            ci: ComponentInterface::from_webidl(
                &format!("namespace {namespace} {{}};"),
                "fixture_crate",
            )
            .expect("valid test UDL"),
            config: NodeBindingGeneratorConfig {
                package_name: Some(format!("{namespace}-package")),
                cdylib_name: Some("fixture".to_string()),
                ..NodeBindingGeneratorConfig::default()
            },
        }
    }

    fn component_with_manual_load(namespace: &str) -> Component<NodeBindingGeneratorConfig> {
        let mut component = component_with_namespace(namespace);
        component.config.manual_load = true;
        component
    }

    fn component_from_webidl(source: &str) -> Component<NodeBindingGeneratorConfig> {
        Component {
            ci: ComponentInterface::from_webidl(source, "fixture_crate").expect("valid test UDL"),
            config: NodeBindingGeneratorConfig {
                package_name: Some("fixture-package".to_string()),
                cdylib_name: Some("fixture".to_string()),
                ..NodeBindingGeneratorConfig::default()
            },
        }
    }

    fn temp_dir_path(name: &str) -> Utf8PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        Utf8PathBuf::from_path_buf(env::temp_dir().join(format!(
            "uniffi-bindgen-node-js-{name}-{}-{unique}",
            process::id()
        )))
        .expect("temp dir path should be utf-8")
    }

    fn parse_node_config(source: &str) -> NodeBindingGeneratorConfig {
        let root = toml::from_str::<toml::Value>(source).expect("test TOML should deserialize");
        parse_node_binding_config(&root).expect("node config should deserialize")
    }

    fn write_test_package(
        output_dir: &Utf8Path,
        component: &Component<NodeBindingGeneratorConfig>,
    ) -> Result<()> {
        write_generated_package(output_dir, component)
    }

    fn extract_section(contents: &str, start_marker: &str, end_marker: &str) -> String {
        let start = contents
            .find(start_marker)
            .unwrap_or_else(|| panic!("missing start marker {start_marker:?}"));
        let end = contents[start..]
            .find(end_marker)
            .map(|offset| start + offset)
            .unwrap_or_else(|| panic!("missing end marker {end_marker:?}"));
        contents[start..end].trim().to_string()
    }

    fn normalize_checksum_value(contents: &str) -> String {
        contents
            .lines()
            .map(|line| {
                if line
                    .trim_start()
                    .starts_with("\"uniffi_fixture_crate_checksum_func_current_generation\": ")
                {
                    "    \"uniffi_fixture_crate_checksum_func_current_generation\": <CHECKSUM>,"
                        .to_string()
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn assert_contains_in_order(contents: &str, snippets: &[&str]) {
        let mut search_start = 0;

        for snippet in snippets {
            let relative_offset = contents[search_start..]
                .find(snippet)
                .unwrap_or_else(|| panic!("missing ordered snippet {snippet:?}"));
            search_start += relative_offset + snippet.len();
        }
    }

    #[test]
    fn write_bindings_creates_output_package_directory() {
        let output_dir = temp_dir_path("package-root");
        write_test_package(&output_dir, &component_with_namespace("example"))
            .expect("write_bindings should succeed");

        assert!(output_dir.is_dir(), "expected {output_dir} to be created");

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_emits_package_and_component_files() {
        let output_dir = temp_dir_path("package-files");
        write_test_package(&output_dir, &component_with_namespace("example"))
            .expect("write_bindings should succeed");

        for expected in [
            "package.json",
            "index.js",
            "index.d.ts",
            "example.js",
            "example.d.ts",
            "example-ffi.js",
            "example-ffi.d.ts",
            "runtime/errors.js",
            "runtime/errors.d.ts",
            "runtime/ffi-types.js",
            "runtime/ffi-types.d.ts",
            "runtime/ffi-converters.js",
            "runtime/ffi-converters.d.ts",
            "runtime/rust-call.js",
            "runtime/rust-call.d.ts",
            "runtime/async-rust-call.js",
            "runtime/async-rust-call.d.ts",
            "runtime/handle-map.js",
            "runtime/handle-map.d.ts",
            "runtime/callbacks.js",
            "runtime/callbacks.d.ts",
            "runtime/objects.js",
            "runtime/objects.d.ts",
        ] {
            let path = output_dir.join(expected);
            assert!(path.is_file(), "expected generated file {path}");
        }

        let package_json = fs::read_to_string(output_dir.join("package.json").as_std_path())
            .expect("package.json should be readable");
        assert!(
            package_json.contains("\"name\": \"example-package\""),
            "unexpected package.json contents: {package_json}"
        );
        assert!(
            package_json.contains("\"koffi\": \"^2.0.0\""),
            "unexpected package.json contents: {package_json}"
        );

        let component_js = fs::read_to_string(output_dir.join("example.js").as_std_path())
            .expect("component JS should be readable");
        assert!(
            component_js.contains("componentMetadata"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export { ffiMetadata }"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("loadFfi()"),
            "unexpected component JS contents: {component_js}"
        );

        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        assert!(
            component_ffi_js.contains("import koffi from \"koffi\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("from \"./runtime/ffi-types.js\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("koffi.load"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("uniffi_contract_version"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        let ffi_types_js =
            fs::read_to_string(output_dir.join("runtime/ffi-types.js").as_std_path())
                .expect("runtime FFI types JS should be readable");
        let errors_js = fs::read_to_string(output_dir.join("runtime/errors.js").as_std_path())
            .expect("runtime errors JS should be readable");
        assert!(
            errors_js.contains("export class UniffiError"),
            "unexpected runtime errors JS contents: {errors_js}"
        );
        assert!(
            errors_js.contains("export const UniffiInternalError"),
            "unexpected runtime errors JS contents: {errors_js}"
        );
        assert!(
            ffi_types_js.contains("export const RustBuffer"),
            "unexpected runtime FFI types JS contents: {ffi_types_js}"
        );
        assert!(
            ffi_types_js.contains("export function defineCallbackVtable"),
            "unexpected runtime FFI types JS contents: {ffi_types_js}"
        );
        assert!(
            ffi_types_js.contains("export function normalizeUInt64"),
            "unexpected runtime FFI types JS contents: {ffi_types_js}"
        );
        assert!(
            ffi_types_js.contains("export class RustBufferValue"),
            "unexpected runtime FFI types JS contents: {ffi_types_js}"
        );
        assert!(
            ffi_types_js.contains("export function readRustBufferBytes"),
            "unexpected runtime FFI types JS contents: {ffi_types_js}"
        );
        let ffi_converters_js =
            fs::read_to_string(output_dir.join("runtime/ffi-converters.js").as_std_path())
                .expect("runtime FFI converters JS should be readable");
        assert!(
            ffi_converters_js.contains("export class AbstractFfiConverterByteArray"),
            "unexpected runtime FFI converters JS contents: {ffi_converters_js}"
        );
        assert!(
            ffi_converters_js.contains("export const FfiConverterString"),
            "unexpected runtime FFI converters JS contents: {ffi_converters_js}"
        );
        let rust_call_js =
            fs::read_to_string(output_dir.join("runtime/rust-call.js").as_std_path())
                .expect("runtime rust-call JS should be readable");
        assert!(
            rust_call_js.contains("export function checkRustCallStatus"),
            "unexpected runtime rust-call JS contents: {rust_call_js}"
        );
        assert!(
            rust_call_js.contains("export class UniffiRustCaller"),
            "unexpected runtime rust-call JS contents: {rust_call_js}"
        );
        let handle_map_js =
            fs::read_to_string(output_dir.join("runtime/handle-map.js").as_std_path())
                .expect("runtime handle-map JS should be readable");
        assert!(
            handle_map_js.contains("export class UniffiHandleMap"),
            "unexpected runtime handle-map JS contents: {handle_map_js}"
        );
        assert!(
            handle_map_js.contains("export const FIRST_FOREIGN_HANDLE"),
            "unexpected runtime handle-map JS contents: {handle_map_js}"
        );
        let async_rust_call_js =
            fs::read_to_string(output_dir.join("runtime/async-rust-call.js").as_std_path())
                .expect("runtime async rust-call JS should be readable");
        let callbacks_js =
            fs::read_to_string(output_dir.join("runtime/callbacks.js").as_std_path())
                .expect("runtime callbacks JS should be readable");
        let objects_js = fs::read_to_string(output_dir.join("runtime/objects.js").as_std_path())
            .expect("runtime objects JS should be readable");
        assert!(
            async_rust_call_js.contains("export async function rustCallAsync"),
            "unexpected runtime async rust-call JS contents: {async_rust_call_js}"
        );
        assert!(
            async_rust_call_js.contains("export const rustFutureContinuationCallback"),
            "unexpected runtime async rust-call JS contents: {async_rust_call_js}"
        );
        assert!(
            callbacks_js.contains("export class UniffiCallbackRegistry"),
            "unexpected runtime callbacks JS contents: {callbacks_js}"
        );
        assert!(
            callbacks_js.contains("export function invokeCallbackMethod"),
            "unexpected runtime callbacks JS contents: {callbacks_js}"
        );
        assert!(
            callbacks_js.contains("export function writeCallbackError"),
            "unexpected runtime callbacks JS contents: {callbacks_js}"
        );
        assert!(
            objects_js.contains("export class UniffiObjectFactory"),
            "unexpected runtime objects JS contents: {objects_js}"
        );
        assert!(
            objects_js.contains("export class FfiConverterObject"),
            "unexpected runtime objects JS contents: {objects_js}"
        );
        assert!(
            objects_js.contains("UNIFFI_OBJECT_HANDLE_SIZE"),
            "unexpected runtime objects JS contents: {objects_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_emits_koffi_callback_and_function_declarations() {
        let output_dir = temp_dir_path("ffi-bindings");
        let component = component_from_webidl(
            r#"
            namespace example {
                u64 current_generation();
                void init_logging(LogCallback callback);
            };

            callback interface LogCallback {
                void log(string message);
            };
            "#,
        );

        write_test_package(&output_dir, &component).expect("write_bindings should succeed");

        let component_js = fs::read_to_string(output_dir.join("example.js").as_std_path())
            .expect("component JS should be readable");
        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        assert!(
            component_js.contains("createCallbackRegistry"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("configureRuntimeHooks({"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("loadFfi();"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_ffi_js
                .contains("defineCallbackPrototype(\"CallbackInterfaceLogCallbackMethod0\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("export function configureRuntimeHooks"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            !component_ffi_js.contains("if (!ffiMetadata.manualLoad) {\n  load();\n}"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("defineCallbackPrototype(\"RustFutureContinuationCallback\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js
                .contains("defineCallbackVtable(\"VTableCallbackInterfaceLogCallback\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("defineStructType(\"ForeignFuture\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("defineCallbackPrototype(\"ForeignFutureCompleteVoid\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("current_generation"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("init_callback_vtable_logcallback"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("ffi_fixture_crate_uniffi_contract_version"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("uniffi_fixture_crate_checksum_func_current_generation"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("normalizeUInt64"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_emits_typed_object_handle_round_trip_support() {
        let output_dir = temp_dir_path("object-handle-round-trip");
        let component = component_from_webidl(
            r#"
            namespace example {};

            interface Store {
                constructor();
                Store? maybe_clone(Store? value);
            };
            "#,
        );

        write_test_package(&output_dir, &component).expect("write_bindings should succeed");

        let component_js = fs::read_to_string(output_dir.join("example.js").as_std_path())
            .expect("component JS should be readable");
        let objects_js = fs::read_to_string(output_dir.join("runtime/objects.js").as_std_path())
            .expect("runtime objects JS should be readable");
        let ffi_types_js =
            fs::read_to_string(output_dir.join("runtime/ffi-types.js").as_std_path())
                .expect("runtime FFI types JS should be readable");

        assert!(
            component_js.contains("getFfiBindings"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("handleType: () => getFfiBindings().ffiTypes.RustArcPtrStore,"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            objects_js.contains("import koffi from \"koffi\";"),
            "unexpected runtime objects JS contents: {objects_js}"
        );
        assert!(
            objects_js.contains(
                "return koffi.decode(new BigUint64Array([normalizeUInt64(normalizedHandle)]), handleType);",
            ),
            "unexpected runtime objects JS contents: {objects_js}"
        );
        assert!(
            objects_js.contains("return this.factory.createRetyped(handle);"),
            "unexpected runtime objects JS contents: {objects_js}"
        );
        assert!(
            objects_js.contains("const rawHandle = requireHandle("),
            "unexpected runtime objects JS contents: {objects_js}"
        );
        assert!(
            ffi_types_js.contains("return normalizeUInt64(pointer);"),
            "unexpected runtime FFI types JS contents: {ffi_types_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_emits_slatedb_callback_interface_paths() {
        let output_dir = temp_dir_path("slatedb-callbacks");
        let component = component_from_webidl(
            r#"
            namespace example {
                void init_logging(LogLevel level, LogCallback? callback);
            };

            enum LogLevel {
                "off",
                "info"
            };

            dictionary LogRecord {
                LogLevel level;
                string target;
                string message;
            };

            [Error]
            interface MergeOperatorCallbackError {
                Callback(string message);
            };

            callback interface LogCallback {
                void log(LogRecord record);
            };

            callback interface MergeOperator {
                [Throws=MergeOperatorCallbackError]
                bytes merge(bytes key, bytes? existing_value, bytes operand);
            };

            interface DbBuilder {
                constructor();
                void with_merge_operator(MergeOperator merge_operator);
            };
            "#,
        );

        write_test_package(&output_dir, &component).expect("write_bindings should succeed");

        let component_js = fs::read_to_string(output_dir.join("example.js").as_std_path())
            .expect("component JS should be readable");

        assert!(
            component_js.contains("export function init_logging(level, callback)"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "const loweredCallback = uniffiLowerIntoRustBuffer(uniffiOptionalConverter(FfiConverterLogCallback), callback);"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js
                .contains(
                    "function uniffiRegisterLogCallbackVtable(bindings, registrations, vtableReferences) {"
                ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js
                .contains(
                    "function uniffiRegisterMergeOperatorVtable(bindings, registrations, vtableReferences) {"
                ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("args: [\n          uniffiLiftFromRustBuffer(FfiConverterLogRecord, record),\n        ],"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "const loweredMergeOperator = FfiConverterMergeOperator.lower(merge_operator);"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "args: [\n          uniffiLiftFromRustBuffer(FfiConverterBytes, key),\n          uniffiLiftFromRustBuffer(uniffiOptionalConverter(FfiConverterBytes), existing_value),\n          uniffiLiftFromRustBuffer(FfiConverterBytes, operand),\n        ],"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "lowerError: (error) => error instanceof MergeOperatorCallbackError ? uniffiLowerIntoRustBuffer(FfiConverterMergeOperatorCallbackError, error) : null,"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "const loweredReturn = uniffiLowerIntoRustBuffer(FfiConverterBytes, uniffiResult);"
            ),
            "unexpected component JS contents: {component_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_emits_slatedb_async_api_paths() {
        let output_dir = temp_dir_path("slatedb-async-apis");
        let component = component_from_webidl(
            r#"
            namespace example {};

            enum IsolationLevel {
                "read_committed",
                "serializable"
            };

            dictionary KeyRange {
                bytes start;
                bytes end;
            };

            dictionary KeyValue {
                bytes key;
                bytes value;
            };

            dictionary WriteHandle {
                u64 seq;
            };

            dictionary WalFileMetadata {
                i64 last_modified_seconds;
                u32 last_modified_nanos;
                u64 size_bytes;
                string location;
            };

            dictionary RowEntry {
                bytes key;
                bytes value;
            };

            interface WriteBatch {
                constructor();
            };

            interface DbIterator {
                constructor();
                [Async] KeyValue? next();
                [Async] void seek(bytes key);
            };

            interface DbSnapshot {
                constructor();
                [Async] bytes? get(bytes key);
                [Async] KeyValue? get_key_value(bytes key);
                [Async] DbIterator scan(KeyRange range);
                [Async] DbIterator scan_prefix(bytes prefix);
            };

            interface DbReader {
                constructor();
                [Async] bytes? get(bytes key);
                [Async] DbIterator scan(KeyRange range);
                [Async] DbIterator scan_prefix(bytes prefix);
                [Async] void shutdown();
            };

            interface DbTransaction {
                constructor();
                [Async] void put(bytes key, bytes value);
                [Async] bytes? get(bytes key);
                [Async] KeyValue? get_key_value(bytes key);
                [Async] DbIterator scan(KeyRange range);
                [Async] DbIterator scan_prefix(bytes prefix);
                [Async] WriteHandle? commit();
            };

            interface Db {
                constructor();
                [Async] void shutdown();
                [Async] bytes? get(bytes key);
                [Async] KeyValue? get_key_value(bytes key);
                [Async] DbIterator scan(KeyRange range);
                [Async] DbIterator scan_prefix(bytes prefix);
                [Async] WriteHandle put(bytes key, bytes value);
                [Async] void flush();
                [Async] DbSnapshot snapshot();
                [Async] DbTransaction begin(IsolationLevel isolation_level);
                [Async] void write(WriteBatch batch);
            };

            interface WalFile {
                constructor();
                [Async] WalFileMetadata metadata();
                [Async] WalFileIterator iterator();
            };

            interface WalFileIterator {
                constructor();
                [Async] RowEntry? next();
            };

            interface WalReader {
                constructor();
                WalFile get(u64 id);
                [Async] sequence<WalFile> list(u64? start_id, u64? end_id);
            };
            "#,
        );

        write_test_package(&output_dir, &component).expect("write_bindings should succeed");

        let component_js = fs::read_to_string(output_dir.join("example.js").as_std_path())
            .expect("component JS should be readable");
        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");

        assert!(
            component_js.contains("export class Db extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export class DbReader extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export class DbIterator extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export class DbSnapshot extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export class DbTransaction extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export class WalReader extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export class WalFile extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export class WalFileIterator extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiLiftFromRustBuffer(uniffiOptionalConverter(FfiConverterBytes), uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiLiftFromRustBuffer(uniffiOptionalConverter(FfiConverterKeyValue), uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiDbIteratorObjectFactory.createRawExternal(uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiDbSnapshotObjectFactory.createRawExternal(uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiDbTransactionObjectFactory.createRawExternal(uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiWalFileIteratorObjectFactory.createRawExternal(uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiLiftFromRustBuffer(uniffiOptionalConverter(FfiConverterWriteHandle), uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiLiftFromRustBuffer(FfiConverterWalFileMetadata, uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiLiftFromRustBuffer(uniffiOptionalConverter(FfiConverterRowEntry), uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiLiftFromRustBuffer(uniffiArrayConverter(FfiConverterWalFile), uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js
                .contains("const loweredBatch = uniffiWriteBatchObjectFactory.cloneHandle(batch);"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "const loweredIsolationLevel = uniffiLowerIntoRustBuffer(FfiConverterIsolationLevel, isolation_level);"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("rustFutureContinuationCallback"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("let uniffiRustFutureContinuationPointer = null;"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("const library = bindings.library;"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("let libraryCache = uniffiLibraryFunctionCache.get(library);"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("uniffiLibraryFunctionCache.set(library, libraryCache);"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("function uniffiGetRustFutureContinuationPointer() {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "pollFunc: (rustFuture, _continuationCallback, continuationHandle) => ffiFunctions."
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("uniffiGetRustFutureContinuationPointer(), continuationHandle)"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("configureRuntimeHooks({"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("koffi.unregister(uniffiRustFutureContinuationPointer);"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_ffi_js.contains("export function configureRuntimeHooks"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            !component_ffi_js.contains("if (!ffiMetadata.manualLoad) {\n  load();\n}"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_makes_ffi_load_idempotent() {
        let output_dir = temp_dir_path("ffi-idempotent-load");
        write_test_package(&output_dir, &component_with_namespace("example"))
            .expect("write_bindings should succeed");

        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        assert!(
            component_ffi_js.contains("loadedBindings.libraryPath === canonicalLibraryPath"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("return loadedBindings;"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("Call unload() before loading a different library path."),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_validates_contract_version_during_load() {
        let output_dir = temp_dir_path("ffi-contract-version");
        write_test_package(&output_dir, &component_with_namespace("example"))
            .expect("write_bindings should succeed");

        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        assert!(
            component_ffi_js.contains("expectedContractVersion: 29"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("validateContractVersion(bindings);"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("throw new ContractVersionMismatchError(expected, actual);"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("bindings.library.unload();"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        let component_ffi_dts =
            fs::read_to_string(output_dir.join("example-ffi.d.ts").as_std_path())
                .expect("component FFI DTS should be readable");
        assert!(
            component_ffi_dts.contains("export declare function validateContractVersion"),
            "unexpected component FFI DTS contents: {component_ffi_dts}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_validates_checksums_during_load() {
        let output_dir = temp_dir_path("ffi-checksums");
        let component = component_from_webidl(
            r#"
            namespace example {
                u64 current_generation();
            };
            "#,
        );

        write_test_package(&output_dir, &component).expect("write_bindings should succeed");

        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        assert!(
            component_ffi_js.contains("validateChecksums(bindings);"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("checksums: Object.freeze({"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("\"uniffi_fixture_crate_checksum_func_current_generation\":"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains(
                "throw new ChecksumMismatchError(\"uniffi_fixture_crate_checksum_func_current_generation\", expected, actual);"
            ),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        let component_ffi_dts =
            fs::read_to_string(output_dir.join("example-ffi.d.ts").as_std_path())
                .expect("component FFI DTS should be readable");
        assert!(
            component_ffi_dts.contains("export declare function validateChecksums"),
            "unexpected component FFI DTS contents: {component_ffi_dts}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_snapshot_ffi_initialization_contract() {
        let output_dir = temp_dir_path("ffi-initialization");
        let component = component_from_webidl(
            r#"
            namespace example {
                u64 current_generation();
            };
            "#,
        );

        write_test_package(&output_dir, &component).expect("write_bindings should succeed");

        let ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        let ffi_dts = fs::read_to_string(output_dir.join("example-ffi.d.ts").as_std_path())
            .expect("component FFI DTS should be readable");
        let metadata_section = normalize_checksum_value(&extract_section(
            &ffi_js,
            "export const ffiMetadata = Object.freeze({",
            "function createBindingCore(",
        ));
        let lifecycle_section = extract_section(
            &ffi_js,
            "export function load(libraryPath = undefined) {",
            "export const ffiFunctions = Object.freeze({",
        );
        let initialization_contract = extract_section(
            &ffi_dts,
            "export interface FfiMetadata {",
            "export declare const ffiFunctions: Readonly<Record<string, (...args: any[]) => any>>;",
        );

        insta::assert_snapshot!(
            format!(
                "=== metadata ===\n{metadata_section}\n\n=== lifecycle ===\n{lifecycle_section}"
            ),
            @r#"
        === metadata ===
        export const ffiMetadata = Object.freeze({
          namespace: "example",
          cdylibName: "fixture",
          libPathLiteral: null,
          bundledPrebuilds: false,
          manualLoad: false,
        });

        export const ffiIntegrity = Object.freeze({
          contractVersionFunction: "ffi_fixture_crate_uniffi_contract_version",
          expectedContractVersion: 30,
          checksums: Object.freeze({

            "uniffi_fixture_crate_checksum_func_current_generation": <CHECKSUM>,

          }),
        });

        let loadedBindings = null;
        let loadedFfiTypes = null;
        let loadedFfiFunctions = null;
        // Koffi retains native state for repeated lib.func() declarations, so keep a
        // single binding core alive across unload/load cycles and evict stale cores
        // when switching to a different canonical library path.
        let cachedBindingCore = null;
        let cachedLibraryPath = null;
        let runtimeHooks = Object.freeze({});
        const moduleFilename = fileURLToPath(import.meta.url);
        const moduleDirectory = dirname(moduleFilename);
        const libraryNotLoadedMessage =
          "The native library is not loaded. Call load(libraryPath) first.";

        function defaultSiblingLibraryFilename() {
          const extensionByPlatform = {
            darwin: ".dylib",
            linux: ".so",
            win32: ".dll",
          };
          const extension = extensionByPlatform[process.platform] ?? ".so";

          if (process.platform === "win32") {
            return ffiMetadata.cdylibName.endsWith(extension)
              ? ffiMetadata.cdylibName
              : `${ffiMetadata.cdylibName}${extension}`;
          }

          const libraryBaseName = ffiMetadata.cdylibName.startsWith("lib")
            ? ffiMetadata.cdylibName
            : `lib${ffiMetadata.cdylibName}`;
          return libraryBaseName.endsWith(extension)
            ? libraryBaseName
            : `${libraryBaseName}${extension}`;
        }

        function defaultBundledTarget() {
          if (process.platform !== "linux") {
            return `${process.platform}-${process.arch}`;
          }

          const glibcVersionRuntime =
            process.report?.getReport?.().header?.glibcVersionRuntime;
          const linuxLibc = glibcVersionRuntime == null ? "musl" : "gnu";
          return `${process.platform}-${process.arch}-${linuxLibc}`;
        }

        function defaultBundledLibrary() {
          const target = defaultBundledTarget();
          const filename = defaultSiblingLibraryFilename();
          return Object.freeze({
            target,
            packageRelativePath: `prebuilds/${target}/${filename}`,
            libraryPath: join(moduleDirectory, "prebuilds", target, filename),
          });
        }

        function defaultSiblingLibraryPath() {
          return join(moduleDirectory, defaultSiblingLibraryFilename());
        }

        function resolveLibraryPath(libraryPath = undefined) {
          const rawLibraryPath = libraryPath ?? ffiMetadata.libPathLiteral;
          if (rawLibraryPath != null) {
            return Object.freeze({
              libraryPath: isAbsolute(rawLibraryPath)
                ? rawLibraryPath
                : join(moduleDirectory, rawLibraryPath),
              bundledPrebuild: null,
            });
          }

          if (ffiMetadata.bundledPrebuilds) {
            const bundledPrebuild = defaultBundledLibrary();
            return Object.freeze({
              libraryPath: bundledPrebuild.libraryPath,
              bundledPrebuild,
            });
          }

          return Object.freeze({
            libraryPath: defaultSiblingLibraryPath(),
            bundledPrebuild: null,
          });
        }

        function canonicalizeExistingLibraryPath(libraryPath) {
          if (!existsSync(libraryPath)) {
            return libraryPath;
          }

          return typeof realpathSync.native === "function"
            ? realpathSync.native(libraryPath)
            : realpathSync(libraryPath);
        }

        === lifecycle ===
        export function load(libraryPath = undefined) {
          const resolution = resolveLibraryPath(libraryPath);
          const resolvedLibraryPath = resolution.libraryPath;
          const bundledPrebuild = resolution.bundledPrebuild;
          const canonicalLibraryPath = canonicalizeExistingLibraryPath(resolvedLibraryPath);

          if (loadedBindings !== null) {
            if (loadedBindings.libraryPath === canonicalLibraryPath) {
              return loadedBindings;
            }

            throw new Error(
              `The native library is already loaded from ${JSON.stringify(loadedBindings.libraryPath)}. Call unload() before loading a different library path.`,
            );
          }

          if (bundledPrebuild !== null && !existsSync(resolvedLibraryPath)) {
            throw new Error(
              `No bundled UniFFI library was found for target ${JSON.stringify(bundledPrebuild.target)}. Expected ${JSON.stringify(bundledPrebuild.packageRelativePath)} inside the generated package.`,
            );
          }

          let bindingCore =
            cachedLibraryPath === canonicalLibraryPath
              ? cachedBindingCore
              : null;
          if (bindingCore == null && cachedBindingCore != null) {
            cachedBindingCore.library.unload();
            clearBindingCoreCache();
          }

          const bindings = createBindings(canonicalLibraryPath, bindingCore);
          try {
            runtimeHooks.onLoad?.(bindings);
            if (bindingCore == null) {
              validateContractVersion(bindings);
              validateChecksums(bindings);
              bindingCore = cacheBindingCore(canonicalLibraryPath, bindings);
            }
          } catch (error) {
            try {
              runtimeHooks.onUnload?.(bindings);
            } catch {
              // Preserve the original initialization failure.
            }
            if (bindingCore == null) {
              try {
                bindings.library.unload();
              } catch {
                // Preserve the original initialization failure.
              }
            }
            throw error;
          }

          loadedBindings = bindings;
          loadedFfiTypes = bindings.ffiTypes;
          loadedFfiFunctions = bindings.ffiFunctions;
          return loadedBindings;
        }

        export function unload() {
          if (loadedBindings === null) {
            return false;
          }

          let hookError = null;
          try {
            runtimeHooks.onUnload?.(loadedBindings);
          } catch (error) {
            hookError = error;
          }
          loadedBindings = null;
          loadedFfiTypes = null;
          loadedFfiFunctions = null;
          if (hookError != null) {
            throw hookError;
          }
          return true;
        }

        export function isLoaded() {
          return loadedBindings !== null;
        }

        export function configureRuntimeHooks(hooks = undefined) {
          runtimeHooks = Object.freeze(hooks ?? {});
        }


        if (!ffiMetadata.manualLoad) {
          load();
        }


        function throwLibraryNotLoaded() {
          throw new LibraryNotLoadedError(libraryNotLoadedMessage);
        }

        export function getFfiBindings() {
          if (loadedBindings === null) {
            throwLibraryNotLoaded();
          }

          return loadedBindings;
        }

        export function getFfiTypes() {
          if (loadedFfiTypes === null) {
            throwLibraryNotLoaded();
          }

          return loadedFfiTypes;
        }

        function getLoadedFfiFunctions() {
          if (loadedFfiFunctions === null) {
            throwLibraryNotLoaded();
          }

          return loadedFfiFunctions;
        }

        export function getContractVersion(bindings = getFfiBindings()) {
          return bindings.ffiFunctions.ffi_fixture_crate_uniffi_contract_version();
        }

        export function validateContractVersion(bindings = getFfiBindings()) {
          const actual = getContractVersion(bindings);
          const expected = ffiIntegrity.expectedContractVersion;
          if (actual !== expected) {
            throw new ContractVersionMismatchError(expected, actual);
          }
          return actual;
        }

        export function getChecksums(bindings = getFfiBindings()) {
          return Object.freeze({

            "uniffi_fixture_crate_checksum_func_current_generation": bindings.ffiFunctions.uniffi_fixture_crate_checksum_func_current_generation(),

          });
        }

        export function validateChecksums(bindings = getFfiBindings()) {
          const actualChecksums = getChecksums(bindings);

          {
            const expected = ffiIntegrity.checksums["uniffi_fixture_crate_checksum_func_current_generation"];
            const actual = actualChecksums["uniffi_fixture_crate_checksum_func_current_generation"];
            if (actual !== expected) {
              throw new ChecksumMismatchError("uniffi_fixture_crate_checksum_func_current_generation", expected, actual);
            }
          }

          return actualChecksums;
        }
        "#
        );

        insta::assert_snapshot!(
            initialization_contract,
            @r#"
        export interface FfiMetadata {
          namespace: string;
          cdylibName: string;
          libPathLiteral: string | null;
          bundledPrebuilds: boolean;
          manualLoad: boolean;
        }

        export interface FfiBindings {
          libraryPath: string;
          library: unknown;
          ffiTypes: Readonly<Record<string, unknown>>;
          ffiCallbacks: Readonly<Record<string, unknown>>;
          ffiStructs: Readonly<Record<string, unknown>>;
          ffiFunctions: Readonly<Record<string, (...args: any[]) => any>>;
        }

        export interface FfiIntegrity {
          contractVersionFunction: string;
          expectedContractVersion: number;
          checksums: Readonly<Record<string, number>>;
        }

        export interface FfiRuntimeHooks {
          onLoad?(bindings: Readonly<FfiBindings>): void;
          onUnload?(bindings: Readonly<FfiBindings>): void;
        }

        export declare const ffiMetadata: Readonly<FfiMetadata>;
        export declare const ffiIntegrity: Readonly<FfiIntegrity>;
        export declare function configureRuntimeHooks(hooks?: FfiRuntimeHooks | null): void;
        export declare function load(libraryPath?: string | null): Readonly<FfiBindings>;
        export declare function unload(): boolean;
        export declare function isLoaded(): boolean;
        export declare function getFfiBindings(): Readonly<FfiBindings>;
        export declare function getFfiTypes(): Readonly<Record<string, unknown>>;
        export declare function getContractVersion(bindings?: Readonly<FfiBindings>): number;
        export declare function validateContractVersion(bindings?: Readonly<FfiBindings>): number;
        export declare function getChecksums(
          bindings?: Readonly<FfiBindings>,
        ): Readonly<Record<string, number>>;
        export declare function validateChecksums(
          bindings?: Readonly<FfiBindings>,
        ): Readonly<Record<string, number>>;
        "#
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_emits_bundled_resolution_contract() {
        let output_dir = temp_dir_path("ffi-bundled-initialization");
        let mut component = component_from_webidl(
            r#"
            namespace example {
                u64 current_generation();
            };
            "#,
        );
        component.config.bundled_prebuilds = true;

        write_test_package(&output_dir, &component).expect("write_bindings should succeed");

        let component_js = fs::read_to_string(output_dir.join("example.js").as_std_path())
            .expect("component JS should be readable");
        let ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        let component_metadata = extract_section(
            &component_js,
            "export const componentMetadata = Object.freeze({",
            "export { ffiMetadata } from",
        );
        let metadata_and_resolution = extract_section(
            &ffi_js,
            "export const ffiMetadata = Object.freeze({",
            "function createBindingCore(",
        );
        let lifecycle_section = extract_section(
            &ffi_js,
            "export function load(libraryPath = undefined) {",
            "export const ffiFunctions = Object.freeze({",
        );

        assert!(
            component_metadata.contains("bundledPrebuilds: true"),
            "component metadata should expose bundledPrebuilds:\n{component_metadata}"
        );
        assert!(
            metadata_and_resolution.contains("bundledPrebuilds: true"),
            "ffi metadata should expose bundledPrebuilds:\n{metadata_and_resolution}"
        );
        assert!(
            metadata_and_resolution.contains("function defaultBundledTarget()"),
            "bundled target helper should be emitted:\n{metadata_and_resolution}"
        );
        assert!(
            metadata_and_resolution.contains(
                "const glibcVersionRuntime =\n    process.report?.getReport?.().header?.glibcVersionRuntime;"
            ),
            "linux libc detection should use process.report:\n{metadata_and_resolution}"
        );
        assert!(
            metadata_and_resolution
                .contains("const linuxLibc = glibcVersionRuntime == null ? \"musl\" : \"gnu\";"),
            "linux libc suffix should distinguish musl and gnu:\n{metadata_and_resolution}"
        );
        assert!(
            metadata_and_resolution
                .contains("packageRelativePath: `prebuilds/${target}/${filename}`,"),
            "bundled libraries should resolve under prebuilds/<target>/<filename>:\n{metadata_and_resolution}"
        );
        assert_contains_in_order(
            &metadata_and_resolution,
            &[
                "const rawLibraryPath = libraryPath ?? ffiMetadata.libPathLiteral;",
                "if (rawLibraryPath != null) {",
                "if (ffiMetadata.bundledPrebuilds) {",
                "libraryPath: defaultSiblingLibraryPath(),",
            ],
        );
        assert_contains_in_order(
            &lifecycle_section,
            &[
                "if (bundledPrebuild !== null && !existsSync(resolvedLibraryPath)) {",
                "No bundled UniFFI library was found for target ${JSON.stringify(bundledPrebuild.target)}.",
                "Expected ${JSON.stringify(bundledPrebuild.packageRelativePath)} inside the generated package.",
                "let bindingCore =",
                "const bindings = createBindings(canonicalLibraryPath, bindingCore);",
            ],
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_reports_all_unsupported_uniffi_features() {
        let output_dir = temp_dir_path("unsupported-uniffi-features");
        let component = component_from_webidl(
            r#"
            [External="other-crate"]
            typedef enum ExternalThing;

            [Custom]
            typedef string Url;

            namespace example {
                ExternalThing read_external();
                Url parse_url(string value);
            };

            callback interface Logger {
                [Async] void write(string message);
            };
            "#,
        );

        let error = write_test_package(&output_dir, &component)
            .expect_err("unsupported features should be rejected");

        insta::assert_snapshot!(
            error.to_string(),
            @r#"
        unsupported UniFFI features for Node bindings v1:
        - external types are not supported in v1: ExternalThing
        - custom types are not supported in v1: Url
        "#
        );
    }

    #[test]
    fn write_bindings_resolves_sibling_and_literal_library_paths() {
        let output_dir = temp_dir_path("ffi-library-paths");
        write_test_package(&output_dir, &component_with_namespace("example"))
            .expect("write_bindings should succeed");

        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        assert!(
            component_ffi_js.contains("import { existsSync, realpathSync } from \"node:fs\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("import { dirname, isAbsolute, join } from \"node:path\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("const moduleFilename = fileURLToPath(import.meta.url);"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("function defaultSiblingLibraryPath()"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js
                .contains("const rawLibraryPath = libraryPath ?? ffiMetadata.libPathLiteral;"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("function canonicalizeExistingLibraryPath(libraryPath)"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("libraryPath: isAbsolute(rawLibraryPath)"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("bundledPrebuild: null,"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_auto_loads_by_default() {
        let output_dir = temp_dir_path("ffi-auto-load");
        write_test_package(&output_dir, &component_with_namespace("example"))
            .expect("write_bindings should succeed");

        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        assert!(
            component_ffi_js.contains("if (!ffiMetadata.manualLoad) {"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("  load();"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_exports_manual_load_helpers() {
        let output_dir = temp_dir_path("manual-load-exports");
        write_test_package(&output_dir, &component_with_manual_load("example"))
            .expect("write_bindings should succeed");

        let component_js = fs::read_to_string(output_dir.join("example.js").as_std_path())
            .expect("component JS should be readable");
        assert!(
            component_js.contains("export { load, unload } from \"./example-ffi.js\";"),
            "unexpected component JS contents: {component_js}"
        );

        let component_dts = fs::read_to_string(output_dir.join("example.d.ts").as_std_path())
            .expect("component DTS should be readable");
        assert!(
            component_dts.contains("export { load, unload } from \"./example-ffi.js\";"),
            "unexpected component DTS contents: {component_dts}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn config_validation_rejects_commonjs_output() {
        let config = parse_node_config(
            r#"
            [bindings.node]
            module_format = "commonjs"
            "#,
        );

        let error = config
            .validate()
            .expect_err("CommonJS output should be rejected");

        assert!(
            error
                .to_string()
                .contains("CommonJS output is not supported"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn config_override_validation_rejects_commonjs_output() {
        let overrides = NodeBindingCliOverrides::from_parts(
            None,
            None,
            None,
            None,
            false,
            false,
            vec!["commonjs=true".to_string()],
        )
        .expect("override should parse");
        let mut config = NodeBindingGeneratorConfig::default();

        overrides.apply_to(&mut config);
        let error = config
            .validate()
            .expect_err("CommonJS override should be rejected");

        assert!(
            error
                .to_string()
                .contains("CommonJS output is not supported"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn new_config_rejects_removed_legacy_library_path_keys() {
        for key in [
            "lib_path_module",
            "lib_path_modules",
            "out_lib_path_module",
            "out_lib_path_modules",
        ] {
            let source = format!(
                r#"
            [bindings.node]
            {key} = "./native/example.node"
            "#
            );
            let root =
                toml::from_str::<toml::Value>(&source).expect("test TOML should deserialize");
            let error = parse_node_binding_config(&root).unwrap_err();

            assert!(
                error
                    .to_string()
                    .contains(&format!("unknown field `{key}`")),
                "unexpected error for {key}: {error}"
            );
        }
    }

    #[test]
    fn config_override_rejects_removed_legacy_library_path_keys() {
        for key in [
            "lib_path_module",
            "lib_path_modules",
            "out_lib_path_module",
            "out_lib_path_modules",
        ] {
            let error = NodeBindingCliOverrides::from_parts(
                None,
                None,
                None,
                None,
                false,
                false,
                vec![format!("{key}=./native/example.node")],
            )
            .unwrap_err();

            assert!(
                error
                    .to_string()
                    .contains(&format!("unsupported --config-override key '{key}'")),
                "unexpected error for {key}: {error}"
            );
        }
    }

    #[test]
    fn new_config_parses_bindings_node_settings_and_defaults() {
        let explicit = parse_node_config(
            r#"
            [bindings.node]
            package_name = "fixture-package"
            cdylib_name = "fixture_cdylib"
            node_engine = ">=20"
            lib_path_literal = "./native/libfixture.node"
            bundled_prebuilds = false
            manual_load = true
            "#,
        );

        assert_eq!(explicit.package_name.as_deref(), Some("fixture-package"));
        assert_eq!(explicit.cdylib_name.as_deref(), Some("fixture_cdylib"));
        assert_eq!(explicit.node_engine, ">=20");
        assert_eq!(
            explicit.lib_path_literal.as_deref(),
            Some("./native/libfixture.node")
        );
        assert!(!explicit.bundled_prebuilds);
        assert!(explicit.manual_load);

        let defaulted = parse_node_config(
            r#"
            [bindings.node]
            "#,
        );

        assert_eq!(defaulted.package_name, None);
        assert_eq!(defaulted.cdylib_name, None);
        assert_eq!(defaulted.node_engine, ">=16");
        assert_eq!(defaulted.lib_path_literal, None);
        assert!(!defaulted.bundled_prebuilds);
        assert!(!defaulted.manual_load);
    }

    #[test]
    fn new_config_accepts_bundled_prebuilds() {
        let config = parse_node_config(
            r#"
            [bindings.node]
            bundled_prebuilds = true
            "#,
        );

        assert!(config.bundled_prebuilds);
    }

    #[test]
    fn config_validation_rejects_bundled_prebuilds_with_lib_path_literal() {
        let config = parse_node_config(
            r#"
            [bindings.node]
            bundled_prebuilds = true
            lib_path_literal = "./native/libfixture.node"
            "#,
        );

        let error = config
            .validate()
            .expect_err("bundled_prebuilds with lib_path_literal should be rejected");

        assert!(
            error
                .to_string()
                .contains("bundled_prebuilds cannot be enabled together with lib_path_literal"),
            "unexpected error: {error}"
        );
    }
}
