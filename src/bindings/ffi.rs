use std::collections::BTreeSet;

use anyhow::Result;
use askama::Template;
use serde::Serialize;
use uniffi_bindgen::interface::{
    ComponentInterface, FfiArgument, FfiCallbackFunction, FfiDefinition, FfiFunction, FfiStruct,
    FfiType,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedComponentFfi {
    pub js: String,
    pub dts: String,
}

pub(crate) fn render_component_ffi(
    ci: &ComponentInterface,
    cdylib_name: &str,
    lib_path_literal: Option<&str>,
    manual_load: bool,
) -> Result<RenderedComponentFfi> {
    let model = ComponentFfiModel::from_ci(ci);
    let template_context = ComponentFfiTemplateContext {
        namespace_json: json_string(ci.namespace())?,
        cdylib_name_json: json_string(cdylib_name)?,
        lib_path_literal_json: json_optional_string(lib_path_literal)?,
        manual_load,
        opaque_types: model.opaque_types,
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

#[derive(Debug, Clone)]
struct ComponentFfiModel {
    opaque_types: Vec<OpaqueTypeModel>,
    pre_struct_callbacks: Vec<CallbackFunctionModel>,
    post_struct_callbacks: Vec<CallbackFunctionModel>,
    structs: Vec<StructModel>,
    functions: Vec<FunctionModel>,
}

impl ComponentFfiModel {
    fn from_ci(ci: &ComponentInterface) -> Self {
        let mut opaque_names = BTreeSet::new();
        let mut pre_struct_callbacks = Vec::new();
        let mut post_struct_callbacks = Vec::new();
        let mut structs = Vec::new();
        let mut functions = Vec::new();

        for definition in ci.ffi_definitions() {
            collect_opaque_types_from_definition(&definition, &mut opaque_names);

            match definition {
                FfiDefinition::CallbackFunction(callback) => {
                    let model = CallbackFunctionModel::from_callback(&callback);
                    if model.depends_on_structs {
                        post_struct_callbacks.push(model);
                    } else {
                        pre_struct_callbacks.push(model);
                    }
                }
                FfiDefinition::Struct(struct_) => {
                    structs.push(StructModel::from_struct(&struct_));
                }
                FfiDefinition::Function(function) => {
                    functions.push(FunctionModel::from_function(&function));
                }
            }
        }

        let opaque_types = opaque_names
            .into_iter()
            .map(|name| OpaqueTypeModel {
                identifier: opaque_identifier(&name),
                name_json: json_string(&opaque_type_name(&name))
                    .expect("opaque type names should serialize"),
            })
            .collect();

        Self {
            opaque_types,
            pre_struct_callbacks,
            post_struct_callbacks,
            structs,
            functions,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ComponentFfiTemplateContext {
    namespace_json: String,
    cdylib_name_json: String,
    lib_path_literal_json: String,
    manual_load: bool,
    opaque_types: Vec<OpaqueTypeModel>,
    pre_struct_callbacks: Vec<CallbackFunctionModel>,
    post_struct_callbacks: Vec<CallbackFunctionModel>,
    structs: Vec<StructModel>,
    functions: Vec<FunctionModel>,
}

#[derive(Debug, Clone, Serialize)]
struct OpaqueTypeModel {
    identifier: String,
    name_json: String,
}

#[derive(Debug, Clone, Serialize)]
struct CallbackFunctionModel {
    identifier: String,
    name_json: String,
    return_type_expr: String,
    argument_type_exprs: Vec<String>,
    depends_on_structs: bool,
}

impl CallbackFunctionModel {
    fn from_callback(callback: &FfiCallbackFunction) -> Self {
        let depends_on_structs = callback
            .arguments()
            .into_iter()
            .any(|argument| type_depends_on_structs(&argument.type_()))
            || callback.return_type().is_some_and(type_depends_on_structs);
        let mut argument_type_exprs = callback
            .arguments()
            .into_iter()
            .map(FfiArgument::type_)
            .map(render_type_expr)
            .collect::<Vec<_>>();
        if callback.has_rust_call_status_arg() {
            argument_type_exprs.push("koffi.pointer(ffiTypes.RustCallStatus)".to_string());
        }

        Self {
            identifier: js_identifier(callback.name()),
            name_json: json_string(callback.name()).expect("FFI callback names should serialize"),
            return_type_expr: render_optional_type_expr(callback.return_type()),
            argument_type_exprs,
            depends_on_structs,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct StructModel {
    identifier: String,
    name_json: String,
    fields: Vec<StructFieldModel>,
}

impl StructModel {
    fn from_struct(struct_: &FfiStruct) -> Self {
        Self {
            identifier: js_identifier(struct_.name()),
            name_json: json_string(struct_.name()).expect("FFI struct names should serialize"),
            fields: struct_
                .fields()
                .iter()
                .map(|field| StructFieldModel {
                    name_json: json_string(field.name())
                        .expect("FFI struct field names should serialize"),
                    type_expr: render_type_expr(field.type_()),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct StructFieldModel {
    name_json: String,
    type_expr: String,
}

#[derive(Debug, Clone, Serialize)]
struct FunctionModel {
    identifier: String,
    name_json: String,
    return_type_expr: String,
    argument_type_exprs: Vec<String>,
}

impl FunctionModel {
    fn from_function(function: &FfiFunction) -> Self {
        let mut argument_type_exprs = function
            .arguments()
            .into_iter()
            .map(FfiArgument::type_)
            .map(render_type_expr)
            .collect::<Vec<_>>();
        if function.has_rust_call_status_arg() {
            argument_type_exprs.push("koffi.pointer(ffiTypes.RustCallStatus)".to_string());
        }

        Self {
            identifier: js_identifier(function.name()),
            name_json: json_string(function.name()).expect("FFI function names should serialize"),
            return_type_expr: render_optional_type_expr(function.return_type()),
            argument_type_exprs,
        }
    }
}

fn collect_opaque_types_from_definition(definition: &FfiDefinition, names: &mut BTreeSet<String>) {
    match definition {
        FfiDefinition::Function(function) => {
            function
                .arguments()
                .into_iter()
                .for_each(|argument| collect_opaque_types_from_type(&argument.type_(), names));
            if let Some(return_type) = function.return_type() {
                collect_opaque_types_from_type(return_type, names);
            }
        }
        FfiDefinition::CallbackFunction(callback) => {
            callback
                .arguments()
                .into_iter()
                .for_each(|argument| collect_opaque_types_from_type(&argument.type_(), names));
            if let Some(return_type) = callback.return_type() {
                collect_opaque_types_from_type(return_type, names);
            }
        }
        FfiDefinition::Struct(struct_) => {
            struct_
                .fields()
                .iter()
                .for_each(|field| collect_opaque_types_from_type(&field.type_(), names));
        }
    }
}

fn collect_opaque_types_from_type(type_: &FfiType, names: &mut BTreeSet<String>) {
    match type_ {
        FfiType::RustArcPtr(name) => {
            names.insert(name.clone());
        }
        FfiType::Reference(inner) | FfiType::MutReference(inner) => {
            collect_opaque_types_from_type(inner, names)
        }
        _ => {}
    }
}

fn type_depends_on_structs(type_: &FfiType) -> bool {
    match type_ {
        FfiType::Struct(_) => true,
        FfiType::Reference(inner) | FfiType::MutReference(inner) => type_depends_on_structs(inner),
        _ => false,
    }
}

fn render_optional_type_expr(type_: Option<&FfiType>) -> String {
    type_
        .map(render_type_expr)
        .unwrap_or_else(|| "\"void\"".to_string())
}

fn render_type_expr(type_: FfiType) -> String {
    match type_ {
        FfiType::UInt8 => "\"uint8_t\"".to_string(),
        FfiType::Int8 => "\"int8_t\"".to_string(),
        FfiType::UInt16 => "\"uint16_t\"".to_string(),
        FfiType::Int16 => "\"int16_t\"".to_string(),
        FfiType::UInt32 => "\"uint32_t\"".to_string(),
        FfiType::Int32 => "\"int32_t\"".to_string(),
        FfiType::UInt64 => "\"uint64_t\"".to_string(),
        FfiType::Int64 => "\"int64_t\"".to_string(),
        FfiType::Float32 => "\"float\"".to_string(),
        FfiType::Float64 => "\"double\"".to_string(),
        FfiType::RustArcPtr(name) => format!("ffiTypes.{}", opaque_identifier(&name)),
        FfiType::RustBuffer(_) => "ffiTypes.RustBuffer".to_string(),
        FfiType::ForeignBytes => "ffiTypes.ForeignBytes".to_string(),
        FfiType::Callback(name) => format!("ffiCallbacks.{}", js_identifier(&name)),
        FfiType::Struct(name) => format!("ffiStructs.{}", js_identifier(&name)),
        FfiType::Handle => "ffiTypes.UniffiHandle".to_string(),
        FfiType::RustCallStatus => "ffiTypes.RustCallStatus".to_string(),
        FfiType::Reference(inner) | FfiType::MutReference(inner) => {
            format!("koffi.pointer({})", render_type_expr(*inner))
        }
        FfiType::VoidPointer => "ffiTypes.VoidPointer".to_string(),
    }
}

fn opaque_type_name(name: &str) -> String {
    format!("RustArcPtr{name}")
}

fn opaque_identifier(name: &str) -> String {
    js_identifier(&opaque_type_name(name))
}

fn js_identifier(name: &str) -> String {
    let mut output = String::with_capacity(name.len());

    for (index, ch) in name.chars().enumerate() {
        let is_valid = if index == 0 {
            ch.is_ascii_alphabetic() || ch == '_' || ch == '$'
        } else {
            ch.is_ascii_alphanumeric() || ch == '_' || ch == '$'
        };

        if is_valid {
            output.push(ch);
        } else {
            output.push('_');
        }
    }

    if output.is_empty() {
        "_".to_string()
    } else {
        output
    }
}

fn json_string(value: &str) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

fn json_optional_string(value: Option<&str>) -> Result<String> {
    Ok(serde_json::to_string(&value)?)
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
