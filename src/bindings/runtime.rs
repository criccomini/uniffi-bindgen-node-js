use std::collections::BTreeSet;

use anyhow::Result;
use askama::Template;
use camino::Utf8PathBuf;

use super::layout::GeneratedPackageLayout;
use super::templates::write_files;

type TemplateRenderResult = std::result::Result<String, askama::Error>;
type RuntimeTemplateRenderer = fn() -> TemplateRenderResult;

struct RuntimeModuleTemplateSet {
    stem: &'static str,
    dependencies: &'static [&'static str],
    js: RuntimeTemplateRenderer,
    dts: RuntimeTemplateRenderer,
}

pub(crate) fn emit_runtime_files(
    layout: &GeneratedPackageLayout,
    direct_modules: &BTreeSet<&'static str>,
) -> Result<()> {
    write_files(render_runtime_files(layout, direct_modules)?)
}

fn render_runtime_files(
    layout: &GeneratedPackageLayout,
    direct_modules: &BTreeSet<&'static str>,
) -> Result<Vec<(Utf8PathBuf, String)>> {
    runtime_file_contents(direct_modules)?
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

fn runtime_file_contents(direct_modules: &BTreeSet<&'static str>) -> Result<Vec<(String, String)>> {
    let selected_modules = selected_runtime_modules(direct_modules);
    let mut files = Vec::with_capacity(selected_modules.len() * 2);
    for templates in selected_modules {
        files.extend(render_runtime_module_files(templates)?);
    }
    Ok(files)
}

fn selected_runtime_modules(
    direct_modules: &BTreeSet<&'static str>,
) -> Vec<&'static RuntimeModuleTemplateSet> {
    let mut selected_stems = BTreeSet::new();
    for stem in direct_modules {
        visit_runtime_module(stem, &mut selected_stems);
    }

    RUNTIME_MODULE_TEMPLATES
        .iter()
        .filter(|templates| selected_stems.contains(templates.stem))
        .collect()
}

fn visit_runtime_module(stem: &str, selected_stems: &mut BTreeSet<&'static str>) {
    let Some(templates) = runtime_module_template(stem) else {
        return;
    };

    if !selected_stems.insert(templates.stem) {
        return;
    }

    for dependency in templates.dependencies {
        visit_runtime_module(dependency, selected_stems);
    }
}

fn runtime_module_template(stem: &str) -> Option<&'static RuntimeModuleTemplateSet> {
    RUNTIME_MODULE_TEMPLATES
        .iter()
        .find(|templates| templates.stem == stem)
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
        dependencies: &[],
        js: render_runtime_errors_js,
        dts: render_runtime_errors_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "ffi-types",
        dependencies: &["errors"],
        js: render_runtime_ffi_types_js,
        dts: render_runtime_ffi_types_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "ffi-converters",
        dependencies: &["errors", "ffi-types"],
        js: render_runtime_ffi_converters_js,
        dts: render_runtime_ffi_converters_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "rust-call",
        dependencies: &["errors", "ffi-converters", "ffi-types"],
        js: render_runtime_rust_call_js,
        dts: render_runtime_rust_call_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "async-rust-call",
        dependencies: &[
            "errors",
            "ffi-types",
            "ffi-converters",
            "handle-map",
            "rust-call",
        ],
        js: render_runtime_async_rust_call_js,
        dts: render_runtime_async_rust_call_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "handle-map",
        dependencies: &["errors", "ffi-types"],
        js: render_runtime_handle_map_js,
        dts: render_runtime_handle_map_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "callbacks",
        dependencies: &["errors", "ffi-types", "handle-map", "rust-call"],
        js: render_runtime_callbacks_js,
        dts: render_runtime_callbacks_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "objects",
        dependencies: &["errors", "ffi-types", "ffi-converters"],
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
