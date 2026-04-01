use std::collections::BTreeSet;

use anyhow::Result;
use serde::Serialize;
use uniffi_bindgen::interface::{
    ComponentInterface, FfiArgument, FfiCallbackFunction, FfiDefinition, FfiFunction, FfiStruct,
    FfiType,
};

pub(crate) fn build_ffi_ir(ci: &ComponentInterface) -> ComponentFfiModel {
    ComponentFfiModel::from_ci(ci)
}

#[derive(Debug, Clone)]
pub(crate) struct ComponentFfiModel {
    pub(crate) contract_version: ContractVersionModel,
    pub(crate) checksums: Vec<ChecksumModel>,
    pub(crate) opaque_types: Vec<OpaqueTypeModel>,
    pub(crate) pre_struct_callbacks: Vec<CallbackFunctionModel>,
    pub(crate) post_struct_callbacks: Vec<CallbackFunctionModel>,
    pub(crate) structs: Vec<StructModel>,
    pub(crate) functions: Vec<FunctionModel>,
}

impl ComponentFfiModel {
    fn from_ci(ci: &ComponentInterface) -> Self {
        let contract_version_symbol = ci.ffi_uniffi_contract_version().name().to_string();
        let checksums = ci
            .iter_checksums()
            .map(|(name, expected)| ChecksumModel {
                identifier: js_identifier(&name),
                name_json: json_string(&name).expect("FFI checksum names should serialize"),
                expected,
            })
            .collect();
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
            contract_version: ContractVersionModel {
                identifier: js_identifier(&contract_version_symbol),
                name_json: json_string(&contract_version_symbol)
                    .expect("FFI contract version symbol names should serialize"),
                expected: ci.uniffi_contract_version(),
            },
            checksums,
            opaque_types,
            pre_struct_callbacks,
            post_struct_callbacks,
            structs,
            functions,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ContractVersionModel {
    pub(crate) identifier: String,
    pub(crate) name_json: String,
    pub(crate) expected: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChecksumModel {
    pub(crate) identifier: String,
    pub(crate) name_json: String,
    pub(crate) expected: u16,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct OpaqueTypeModel {
    pub(crate) identifier: String,
    pub(crate) name_json: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct CallbackFunctionModel {
    pub(crate) identifier: String,
    pub(crate) name_json: String,
    pub(crate) return_type_expr: String,
    pub(crate) argument_type_exprs: Vec<String>,
    pub(crate) depends_on_structs: bool,
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
pub(crate) struct StructModel {
    pub(crate) identifier: String,
    pub(crate) name_json: String,
    pub(crate) fields: Vec<StructFieldModel>,
    pub(crate) is_callback_vtable: bool,
}

impl StructModel {
    fn from_struct(struct_: &FfiStruct) -> Self {
        Self {
            identifier: js_identifier(struct_.name()),
            name_json: json_string(struct_.name()).expect("FFI struct names should serialize"),
            is_callback_vtable: struct_.name().starts_with("VTableCallbackInterface"),
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
pub(crate) struct StructFieldModel {
    pub(crate) name_json: String,
    pub(crate) type_expr: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FunctionModel {
    pub(crate) identifier: String,
    pub(crate) generic_identifier: Option<String>,
    pub(crate) name_json: String,
    pub(crate) return_type_expr: String,
    pub(crate) argument_type_exprs: Vec<String>,
    pub(crate) generic_argument_type_exprs: Option<Vec<String>>,
    pub(crate) return_normalizer: Option<String>,
}

impl FunctionModel {
    fn from_function(function: &FfiFunction) -> Self {
        let arguments = function.arguments();
        let mut argument_type_exprs = arguments
            .iter()
            .copied()
            .map(FfiArgument::type_)
            .map(render_type_expr)
            .collect::<Vec<_>>();
        if function.has_rust_call_status_arg() {
            argument_type_exprs.push("koffi.pointer(ffiTypes.RustCallStatus)".to_string());
        }

        let generic_argument_type_exprs = match arguments.first().copied().map(FfiArgument::type_) {
            Some(FfiType::Handle) => {
                let mut generic_argument_type_exprs = arguments
                    .iter()
                    .enumerate()
                    .map(|(index, argument)| {
                        if index == 0 {
                            "ffiTypes.UniffiHandle".to_string()
                        } else {
                            render_type_expr(argument.type_())
                        }
                    })
                    .collect::<Vec<_>>();
                if function.has_rust_call_status_arg() {
                    generic_argument_type_exprs
                        .push("koffi.pointer(ffiTypes.RustCallStatus)".to_string());
                }
                Some(generic_argument_type_exprs)
            }
            _ => None,
        };

        Self {
            identifier: js_identifier(function.name()),
            generic_identifier: generic_argument_type_exprs
                .as_ref()
                .map(|_| format!("{}_generic_abi", js_identifier(function.name()))),
            name_json: json_string(function.name()).expect("FFI function names should serialize"),
            return_type_expr: render_optional_type_expr(function.return_type()),
            argument_type_exprs,
            generic_argument_type_exprs,
            return_normalizer: function
                .return_type()
                .and_then(render_return_normalizer_expr),
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
        .map(|type_| render_type_expr(type_.clone()))
        .unwrap_or_else(|| "\"void\"".to_string())
}

fn render_return_normalizer_expr(type_: &FfiType) -> Option<String> {
    match type_ {
        FfiType::Int64 => Some("normalizeInt64".to_string()),
        FfiType::UInt64 => Some("normalizeUInt64".to_string()),
        FfiType::Handle => Some("normalizeHandle".to_string()),
        FfiType::RustBuffer(_) => Some("normalizeRustBuffer".to_string()),
        FfiType::RustCallStatus => Some("normalizeRustCallStatus".to_string()),
        _ => None,
    }
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
        FfiType::RustBuffer(_) => "ffiTypes.RustBuffer".to_string(),
        FfiType::ForeignBytes => "ffiTypes.ForeignBytes".to_string(),
        FfiType::Callback(name) => format!("koffi.pointer(ffiCallbacks.{})", js_identifier(&name)),
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
