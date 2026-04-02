use anyhow::Result;
use askama::Template;
use serde::Serialize;
use uniffi_bindgen::interface::ComponentInterface;

use crate::bindings::ffi_ir::{
    CallbackFunctionModel, ChecksumModel, ContractVersionModel, FunctionModel, StructModel,
    build_ffi_ir,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedComponentFfi {
    pub js: String,
    pub dts: String,
}

pub(crate) fn render_component_ffi(
    ci: &ComponentInterface,
    library_name: &str,
    staged_library_file_name: &str,
    staged_library_package_relative_path: &str,
    bundled_prebuilds: bool,
    manual_load: bool,
) -> Result<RenderedComponentFfi> {
    let model = build_ffi_ir(ci);
    let template_context = ComponentFfiTemplateContext {
        namespace_json: json_string(ci.namespace())?,
        library_name_json: json_string(library_name)?,
        staged_library_file_name_json: json_string(staged_library_file_name)?,
        staged_library_package_relative_path_json: json_string(
            staged_library_package_relative_path,
        )?,
        bundled_prebuilds,
        manual_load,
        requires_runtime_hooks: component_requires_runtime_hooks(ci),
        contract_version: model.contract_version,
        checksums: model.checksums,
        pre_struct_callbacks: model.pre_struct_callbacks,
        post_struct_callbacks: model.post_struct_callbacks,
        structs: model.structs,
        functions: model.functions,
    };

    Ok(RenderedComponentFfi {
        js: ComponentFfiJsTemplate {
            context: template_context.clone(),
        }
        .render()?,
        dts: ComponentFfiDtsTemplate {
            context: template_context,
        }
        .render()?,
    })
}

fn component_requires_runtime_hooks(ci: &ComponentInterface) -> bool {
    ci.has_callback_definitions()
        || ci
            .function_definitions()
            .iter()
            .any(|function| function.is_async())
        || ci.object_definitions().iter().any(|object| {
            object
                .constructors()
                .iter()
                .any(|constructor| constructor.is_async())
                || object.methods().iter().any(|method| method.is_async())
        })
}

#[derive(Debug, Clone, Serialize)]
struct ComponentFfiTemplateContext {
    namespace_json: String,
    library_name_json: String,
    staged_library_file_name_json: String,
    staged_library_package_relative_path_json: String,
    bundled_prebuilds: bool,
    manual_load: bool,
    requires_runtime_hooks: bool,
    contract_version: ContractVersionModel,
    checksums: Vec<ChecksumModel>,
    pre_struct_callbacks: Vec<CallbackFunctionModel>,
    post_struct_callbacks: Vec<CallbackFunctionModel>,
    structs: Vec<StructModel>,
    functions: Vec<FunctionModel>,
}

fn json_string(value: &str) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

#[derive(Template)]
#[template(path = "component/component-ffi.js.j2", escape = "none")]
struct ComponentFfiJsTemplate {
    context: ComponentFfiTemplateContext,
}

#[derive(Template)]
#[template(path = "component/component-ffi.d.ts.j2", escape = "none")]
struct ComponentFfiDtsTemplate {
    context: ComponentFfiTemplateContext,
}
