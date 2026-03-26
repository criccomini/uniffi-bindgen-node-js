use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};
use heck::ToUpperCamelCase;
use uniffi_bindgen::interface::{
    AsType, CallbackInterface, ComponentInterface, Constructor, Enum, Field, Function, Method,
    Object, Type, Variant,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComponentModel {
    pub functions: Vec<FunctionModel>,
    pub records: Vec<RecordModel>,
    pub flat_enums: Vec<EnumModel>,
    pub tagged_enums: Vec<EnumModel>,
    pub errors: Vec<ErrorModel>,
    pub callback_interfaces: Vec<CallbackInterfaceModel>,
    pub objects: Vec<ObjectModel>,
    pub ffi_rustbuffer_free_identifier: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedComponentApi {
    pub js: String,
    pub dts: String,
}

impl ComponentModel {
    pub(crate) fn from_ci(ci: &ComponentInterface) -> Result<Self> {
        validate_supported_features(ci)?;

        let mut flat_enums = Vec::new();
        let mut tagged_enums = Vec::new();
        let mut errors = Vec::new();

        for enum_def in ci.enum_definitions() {
            if ci.is_name_used_as_error(enum_def.name()) {
                errors.push(ErrorModel::from_enum(enum_def));
            } else if enum_def.is_flat() {
                flat_enums.push(EnumModel::from_enum(enum_def));
            } else {
                tagged_enums.push(EnumModel::from_enum(enum_def));
            }
        }

        let model = Self {
            functions: ci
                .function_definitions()
                .iter()
                .map(FunctionModel::from_function)
                .collect(),
            records: ci
                .record_definitions()
                .map(RecordModel::from_record)
                .collect(),
            flat_enums,
            tagged_enums,
            errors,
            callback_interfaces: ci
                .callback_interface_definitions()
                .iter()
                .map(CallbackInterfaceModel::from_callback_interface)
                .collect(),
            objects: ci
                .object_definitions()
                .iter()
                .map(ObjectModel::from_object)
                .collect(),
            ffi_rustbuffer_free_identifier: ffi_symbol_identifier(ci.ffi_rustbuffer_free().name()),
        };
        model.validate_renderable_types()?;
        Ok(model)
    }

    pub(crate) fn render_public_api(&self) -> Result<RenderedComponentApi> {
        let mut js_sections = Vec::new();
        let mut dts_sections = Vec::new();

        if !self.functions.is_empty() || !self.objects.is_empty() {
            js_sections.push(
                "function uniffiNotImplemented(member) {\n  throw new Error(`${member} is not implemented yet. Koffi-backed bindings are still pending.`);\n}"
                    .to_string(),
            );
        }

        if !self.objects.is_empty() {
            js_sections.push(render_js_object_runtime_helpers(
                &self.ffi_rustbuffer_free_identifier,
            ));
        }

        if !self.records.is_empty() {
            dts_sections.push(
                self.records
                    .iter()
                    .map(render_dts_record)
                    .collect::<Result<Vec<_>>>()?
                    .join("\n\n"),
            );
        }

        if !self.flat_enums.is_empty() {
            let flat_enum_js = self
                .flat_enums
                .iter()
                .map(render_js_flat_enum)
                .collect::<Result<Vec<_>>>()?
                .join("\n\n");
            let flat_enum_dts = self
                .flat_enums
                .iter()
                .map(render_dts_flat_enum)
                .collect::<Result<Vec<_>>>()?
                .join("\n\n");
            js_sections.push(flat_enum_js);
            dts_sections.push(flat_enum_dts);
        }

        if !self.tagged_enums.is_empty() {
            let tagged_enum_js = self
                .tagged_enums
                .iter()
                .map(render_js_tagged_enum)
                .collect::<Result<Vec<_>>>()?
                .join("\n\n");
            let tagged_enum_dts = self
                .tagged_enums
                .iter()
                .map(render_dts_tagged_enum)
                .collect::<Result<Vec<_>>>()?
                .join("\n\n");
            js_sections.push(tagged_enum_js);
            dts_sections.push(tagged_enum_dts);
        }

        if !self.errors.is_empty() {
            let error_js = self
                .errors
                .iter()
                .map(render_js_error)
                .collect::<Result<Vec<_>>>()?
                .join("\n\n");
            let error_dts = self
                .errors
                .iter()
                .map(render_dts_error)
                .collect::<Result<Vec<_>>>()?
                .join("\n\n");
            js_sections.push(error_js);
            dts_sections.push(error_dts);
        }

        if !self.callback_interfaces.is_empty() {
            dts_sections.push(
                self.callback_interfaces
                    .iter()
                    .map(render_dts_callback_interface)
                    .collect::<Result<Vec<_>>>()?
                    .join("\n\n"),
            );
        }

        if !self.functions.is_empty() {
            let functions_js = self
                .functions
                .iter()
                .map(render_js_function)
                .collect::<Result<Vec<_>>>()?
                .join("\n\n");
            let functions_dts = self
                .functions
                .iter()
                .map(render_dts_function)
                .collect::<Result<Vec<_>>>()?
                .join("\n\n");
            js_sections.push(functions_js);
            dts_sections.push(functions_dts);
        }

        if !self.objects.is_empty() {
            let objects_js = self
                .objects
                .iter()
                .map(render_js_object)
                .collect::<Result<Vec<_>>>()?
                .join("\n\n");
            let objects_dts = self
                .objects
                .iter()
                .map(render_dts_object)
                .collect::<Result<Vec<_>>>()?
                .join("\n\n");
            js_sections.push(objects_js);
            dts_sections.push(objects_dts);
        }

        Ok(RenderedComponentApi {
            js: js_sections.join("\n\n"),
            dts: dts_sections.join("\n\n"),
        })
    }

    fn validate_renderable_types(&self) -> Result<()> {
        for function in &self.functions {
            validate_arguments_renderable(
                &function.arguments,
                &format!("function {}", function.name),
            )?;
            validate_optional_type_renderable(
                function.return_type.as_ref(),
                &format!("function {} return type", function.name),
            )?;
            validate_optional_type_renderable(
                function.throws_type.as_ref(),
                &format!("function {} error type", function.name),
            )?;
        }

        for record in &self.records {
            for field in &record.fields {
                validate_type_renderable(
                    &field.type_,
                    &format!("record {} field {}", record.name, field.name),
                )?;
            }
        }

        for enum_def in self.flat_enums.iter().chain(&self.tagged_enums) {
            for variant in &enum_def.variants {
                for field in &variant.fields {
                    validate_type_renderable(
                        &field.type_,
                        &format!(
                            "enum {} variant {} field {}",
                            enum_def.name, variant.name, field.name
                        ),
                    )?;
                }
            }
        }

        for error in &self.errors {
            for variant in &error.variants {
                for field in &variant.fields {
                    validate_type_renderable(
                        &field.type_,
                        &format!(
                            "error {} variant {} field {}",
                            error.name, variant.name, field.name
                        ),
                    )?;
                }
            }
        }

        for callback_interface in &self.callback_interfaces {
            for method in &callback_interface.methods {
                validate_arguments_renderable(
                    &method.arguments,
                    &format!(
                        "callback interface {}.{}",
                        callback_interface.name, method.name
                    ),
                )?;
                validate_optional_type_renderable(
                    method.return_type.as_ref(),
                    &format!(
                        "callback interface {}.{} return type",
                        callback_interface.name, method.name
                    ),
                )?;
                validate_optional_type_renderable(
                    method.throws_type.as_ref(),
                    &format!(
                        "callback interface {}.{} error type",
                        callback_interface.name, method.name
                    ),
                )?;
            }
        }

        for object in &self.objects {
            for constructor in &object.constructors {
                validate_arguments_renderable(
                    &constructor.arguments,
                    &format!("constructor {}.{}", object.name, constructor.name),
                )?;
                validate_optional_type_renderable(
                    constructor.throws_type.as_ref(),
                    &format!(
                        "constructor {}.{} error type",
                        object.name, constructor.name
                    ),
                )?;
            }

            for method in &object.methods {
                validate_arguments_renderable(
                    &method.arguments,
                    &format!("method {}.{}", object.name, method.name),
                )?;
                validate_optional_type_renderable(
                    method.return_type.as_ref(),
                    &format!("method {}.{} return type", object.name, method.name),
                )?;
                validate_optional_type_renderable(
                    method.throws_type.as_ref(),
                    &format!("method {}.{} error type", object.name, method.name),
                )?;
            }
        }

        Ok(())
    }
}

pub(crate) fn render_public_type(type_: &Type) -> Result<String> {
    match type_ {
        Type::UInt8
        | Type::Int8
        | Type::UInt16
        | Type::Int16
        | Type::UInt32
        | Type::Int32
        | Type::Float32
        | Type::Float64 => Ok("number".to_string()),
        Type::UInt64 | Type::Int64 => Ok("bigint".to_string()),
        Type::Boolean => Ok("boolean".to_string()),
        Type::String => Ok("string".to_string()),
        Type::Bytes => Ok("Uint8Array".to_string()),
        Type::Object { name, .. }
        | Type::Record { name, .. }
        | Type::Enum { name, .. }
        | Type::CallbackInterface { name, .. } => Ok(name.clone()),
        Type::Optional { inner_type } => {
            Ok(format!("{} | undefined", render_public_type(inner_type)?))
        }
        Type::Sequence { inner_type } => Ok(format!("Array<{}>", render_public_type(inner_type)?)),
        Type::Map {
            key_type,
            value_type,
        } => Ok(format!(
            "Map<{}, {}>",
            render_public_type(key_type)?,
            render_public_type(value_type)?
        )),
        Type::Timestamp => bail!("timestamps are not supported in the public Node API yet"),
        Type::Duration => bail!("durations are not supported in the public Node API yet"),
        Type::Custom { name, .. } => {
            bail!("custom type '{name}' is not supported in the public Node API yet")
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FunctionModel {
    pub name: String,
    pub is_async: bool,
    pub arguments: Vec<ArgumentModel>,
    pub return_type: Option<Type>,
    pub throws_type: Option<Type>,
}

impl FunctionModel {
    fn from_function(function: &Function) -> Self {
        Self {
            name: function.name().to_string(),
            is_async: function.is_async(),
            arguments: function
                .full_arguments()
                .into_iter()
                .map(ArgumentModel::from_argument)
                .collect(),
            return_type: function.return_type().cloned(),
            throws_type: function.throws_type().cloned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordModel {
    pub name: String,
    pub fields: Vec<FieldModel>,
}

impl RecordModel {
    fn from_record(record: &uniffi_bindgen::interface::Record) -> Self {
        Self {
            name: record.name().to_string(),
            fields: record.fields().iter().map(FieldModel::from_field).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnumModel {
    pub name: String,
    pub variants: Vec<VariantModel>,
}

impl EnumModel {
    fn from_enum(enum_def: &Enum) -> Self {
        Self {
            name: enum_def.name().to_string(),
            variants: enum_def
                .variants()
                .iter()
                .map(VariantModel::from_variant)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ErrorModel {
    pub name: String,
    pub is_flat: bool,
    pub variants: Vec<VariantModel>,
}

impl ErrorModel {
    fn from_enum(enum_def: &Enum) -> Self {
        Self {
            name: enum_def.name().to_string(),
            is_flat: enum_def.is_flat(),
            variants: enum_def
                .variants()
                .iter()
                .map(VariantModel::from_variant)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CallbackInterfaceModel {
    pub name: String,
    pub methods: Vec<MethodModel>,
}

impl CallbackInterfaceModel {
    fn from_callback_interface(callback_interface: &CallbackInterface) -> Self {
        Self {
            name: callback_interface.name().to_string(),
            methods: callback_interface
                .methods()
                .into_iter()
                .map(MethodModel::from_method)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectModel {
    pub name: String,
    pub constructors: Vec<ConstructorModel>,
    pub methods: Vec<MethodModel>,
    pub ffi_object_clone_identifier: String,
    pub ffi_object_free_identifier: String,
}

impl ObjectModel {
    fn from_object(object: &Object) -> Self {
        Self {
            name: object.name().to_string(),
            constructors: object
                .constructors()
                .into_iter()
                .map(ConstructorModel::from_constructor)
                .collect(),
            methods: object
                .methods()
                .into_iter()
                .map(MethodModel::from_method)
                .collect(),
            ffi_object_clone_identifier: ffi_symbol_identifier(object.ffi_object_clone().name()),
            ffi_object_free_identifier: ffi_symbol_identifier(object.ffi_object_free().name()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConstructorModel {
    pub name: String,
    pub is_primary: bool,
    pub is_async: bool,
    pub arguments: Vec<ArgumentModel>,
    pub throws_type: Option<Type>,
}

impl ConstructorModel {
    fn from_constructor(constructor: &Constructor) -> Self {
        Self {
            name: constructor.name().to_string(),
            is_primary: constructor.is_primary_constructor(),
            is_async: constructor.is_async(),
            arguments: constructor
                .full_arguments()
                .into_iter()
                .map(ArgumentModel::from_argument)
                .collect(),
            throws_type: constructor.throws_type().cloned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MethodModel {
    pub name: String,
    pub is_async: bool,
    pub arguments: Vec<ArgumentModel>,
    pub return_type: Option<Type>,
    pub throws_type: Option<Type>,
}

impl MethodModel {
    fn from_method(method: &Method) -> Self {
        Self {
            name: method.name().to_string(),
            is_async: method.is_async(),
            arguments: method
                .arguments()
                .into_iter()
                .cloned()
                .map(ArgumentModel::from_argument)
                .collect(),
            return_type: method.return_type().cloned(),
            throws_type: method.throws_type().cloned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VariantModel {
    pub name: String,
    pub fields: Vec<FieldModel>,
}

impl VariantModel {
    fn from_variant(variant: &Variant) -> Self {
        Self {
            name: variant.name().to_string(),
            fields: variant
                .fields()
                .iter()
                .map(FieldModel::from_field)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldModel {
    pub name: String,
    pub type_: Type,
}

impl FieldModel {
    fn from_field(field: &Field) -> Self {
        Self {
            name: field.name().to_string(),
            type_: field.as_type(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArgumentModel {
    pub name: String,
    pub type_: Type,
}

impl ArgumentModel {
    fn from_argument(argument: uniffi_bindgen::interface::Argument) -> Self {
        Self {
            name: argument.name().to_string(),
            type_: argument.as_type(),
        }
    }
}

fn validate_supported_features(ci: &ComponentInterface) -> Result<()> {
    let mut unsupported = Vec::new();

    let external_types = ci
        .iter_external_types()
        .map(describe_type)
        .collect::<BTreeSet<_>>();
    if !external_types.is_empty() {
        unsupported.push(format!(
            "external types are not supported in v1: {}",
            external_types.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    let custom_types = ci
        .iter_local_types()
        .chain(ci.iter_external_types())
        .filter_map(|type_| match type_ {
            Type::Custom { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if !custom_types.is_empty() {
        unsupported.push(format!(
            "custom types are not supported in v1: {}",
            custom_types.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    let async_callback_interfaces = ci
        .callback_interface_definitions()
        .iter()
        .filter(|callback| callback.has_async_method())
        .map(|callback| callback.name().to_string())
        .collect::<BTreeSet<_>>();
    if !async_callback_interfaces.is_empty() {
        unsupported.push(format!(
            "async callback-interface methods are not supported in v1: {}",
            async_callback_interfaces
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    if unsupported.is_empty() {
        return Ok(());
    }

    bail!(
        "unsupported UniFFI features for Node bindings v1:\n- {}",
        unsupported.join("\n- ")
    );
}

fn describe_type(type_: &Type) -> String {
    match type_ {
        Type::Object { name, .. }
        | Type::Record { name, .. }
        | Type::Enum { name, .. }
        | Type::CallbackInterface { name, .. }
        | Type::Custom { name, .. } => name.clone(),
        Type::Optional { inner_type } => format!("Optional<{}>", describe_type(inner_type)),
        Type::Sequence { inner_type } => format!("Vec<{}>", describe_type(inner_type)),
        Type::Map {
            key_type,
            value_type,
        } => format!(
            "HashMap<{}, {}>",
            describe_type(key_type),
            describe_type(value_type)
        ),
        Type::UInt8 => "u8".to_string(),
        Type::Int8 => "i8".to_string(),
        Type::UInt16 => "u16".to_string(),
        Type::Int16 => "i16".to_string(),
        Type::UInt32 => "u32".to_string(),
        Type::Int32 => "i32".to_string(),
        Type::UInt64 => "u64".to_string(),
        Type::Int64 => "i64".to_string(),
        Type::Float32 => "f32".to_string(),
        Type::Float64 => "f64".to_string(),
        Type::Boolean => "bool".to_string(),
        Type::String => "string".to_string(),
        Type::Bytes => "bytes".to_string(),
        Type::Timestamp => "timestamp".to_string(),
        Type::Duration => "duration".to_string(),
    }
}

fn render_js_function(function: &FunctionModel) -> Result<String> {
    Ok(format!(
        "export {}function {}({}) {{\n  return uniffiNotImplemented({});\n}}",
        if function.is_async { "async " } else { "" },
        js_identifier(&function.name),
        render_js_params(&function.arguments),
        json_string_literal(&function.name)?
    ))
}

fn render_dts_function(function: &FunctionModel) -> Result<String> {
    Ok(format!(
        "export declare function {}({}): {};",
        js_identifier(&function.name),
        render_dts_params(&function.arguments)?,
        render_return_type(function.return_type.as_ref(), function.is_async)?
    ))
}

fn render_dts_record(record: &RecordModel) -> Result<String> {
    let mut lines = vec![format!("export interface {} {{", record.name)];
    for field in &record.fields {
        lines.push(format!(
            "  {}: {};",
            quoted_property_name(&field.name)?,
            render_public_type(&field.type_)?
        ));
    }
    lines.push("}".to_string());
    Ok(lines.join("\n"))
}

fn render_dts_callback_interface(callback_interface: &CallbackInterfaceModel) -> Result<String> {
    let mut lines = vec![format!("export interface {} {{", callback_interface.name)];

    for method in &callback_interface.methods {
        lines.push(format!(
            "  {}({}): {};",
            js_member_identifier(&method.name),
            render_dts_params(&method.arguments)?,
            render_return_type(method.return_type.as_ref(), method.is_async)?
        ));
    }

    lines.push("}".to_string());
    Ok(lines.join("\n"))
}

fn render_js_flat_enum(enum_def: &EnumModel) -> Result<String> {
    let mut lines = vec![format!("export const {} = Object.freeze({{", enum_def.name)];
    for variant in &enum_def.variants {
        let variant_name = json_string_literal(&variant.name)?;
        lines.push(format!("  {}: {},", variant_name, variant_name));
    }
    lines.push("});".to_string());
    Ok(lines.join("\n"))
}

fn render_dts_flat_enum(enum_def: &EnumModel) -> Result<String> {
    let mut lines = vec![format!(
        "export declare const {}: Readonly<{{",
        enum_def.name
    )];
    for variant in &enum_def.variants {
        lines.push(format!(
            "  {}: {};",
            quoted_property_name(&variant.name)?,
            json_string_literal(&variant.name)?
        ));
    }
    lines.push("}>;".to_string());
    lines.push(format!(
        "export type {} = (typeof {})[keyof typeof {}];",
        enum_def.name, enum_def.name, enum_def.name
    ));
    Ok(lines.join("\n"))
}

fn render_js_tagged_enum(enum_def: &EnumModel) -> Result<String> {
    let mut lines = vec![format!("export const {} = Object.freeze({{", enum_def.name)];
    for variant in &enum_def.variants {
        lines.push(format!(
            "  {}({}) {{",
            js_member_identifier(&variant.name),
            render_js_fields_as_params(&variant.fields)
        ));
        lines.push("    return Object.freeze({".to_string());
        lines.push(format!(
            "      tag: {},",
            json_string_literal(&variant.name)?
        ));
        for field in &variant.fields {
            lines.push(format!(
                "      {}: {},",
                quoted_property_name(&field.name)?,
                js_identifier(&field.name)
            ));
        }
        lines.push("    });".to_string());
        lines.push("  },".to_string());
    }
    lines.push("});".to_string());
    Ok(lines.join("\n"))
}

fn render_dts_tagged_enum(enum_def: &EnumModel) -> Result<String> {
    let mut lines = Vec::new();
    let mut variant_types = Vec::new();

    for variant in &enum_def.variants {
        let variant_type_name = variant_type_name(&enum_def.name, &variant.name);
        variant_types.push(variant_type_name.clone());
        lines.push(format!("export interface {} {{", variant_type_name));
        lines.push(format!("  tag: {};", json_string_literal(&variant.name)?));
        for field in &variant.fields {
            lines.push(format!(
                "  {}: {};",
                quoted_property_name(&field.name)?,
                render_public_type(&field.type_)?
            ));
        }
        lines.push("}".to_string());
        lines.push(String::new());
    }

    lines.push(format!(
        "export type {} = {};",
        enum_def.name,
        variant_types.join(" | ")
    ));
    lines.push(format!(
        "export declare const {}: Readonly<{{",
        enum_def.name
    ));
    for variant in &enum_def.variants {
        lines.push(format!(
            "  {}({}): {};",
            js_member_identifier(&variant.name),
            render_dts_fields_as_params(&variant.fields)?,
            variant_type_name(&enum_def.name, &variant.name)
        ));
    }
    lines.push("}>;".to_string());

    Ok(lines.join("\n"))
}

fn render_js_error(error: &ErrorModel) -> Result<String> {
    let mut lines = vec![
        format!("export class {} extends Error {{", error.name),
        "  constructor(tag) {".to_string(),
        "    super(tag);".to_string(),
        format!("    this.name = {};", json_string_literal(&error.name)?),
        "    this.tag = tag;".to_string(),
        "  }".to_string(),
        "}".to_string(),
    ];

    for variant in &error.variants {
        lines.push(String::new());
        let variant_class_name = variant_type_name(&error.name, &variant.name);
        lines.push(format!(
            "export class {} extends {} {{",
            variant_class_name, error.name
        ));
        lines.push(format!(
            "  constructor({}) {{",
            render_js_fields_as_params(&variant.fields)
        ));
        lines.push(format!(
            "    super({});",
            json_string_literal(&variant.name)?
        ));
        lines.push(format!(
            "    this.name = {};",
            json_string_literal(&variant_class_name)?
        ));
        for field in &variant.fields {
            lines.push(format!(
                "    this[{}] = {};",
                json_string_literal(&field.name)?,
                js_identifier(&field.name)
            ));
        }
        lines.push("  }".to_string());
        lines.push("}".to_string());
    }

    Ok(lines.join("\n"))
}

fn render_dts_error(error: &ErrorModel) -> Result<String> {
    let mut lines = vec![
        format!("export declare class {} extends Error {{", error.name),
        "  readonly tag: string;".to_string(),
        "  protected constructor(tag: string);".to_string(),
        "}".to_string(),
    ];

    for variant in &error.variants {
        lines.push(String::new());
        let variant_class_name = variant_type_name(&error.name, &variant.name);
        lines.push(format!(
            "export declare class {} extends {} {{",
            variant_class_name, error.name
        ));
        lines.push(format!(
            "  readonly tag: {};",
            json_string_literal(&variant.name)?
        ));
        for field in &variant.fields {
            lines.push(format!(
                "  readonly {}: {};",
                quoted_property_name(&field.name)?,
                render_public_type(&field.type_)?
            ));
        }
        lines.push(format!(
            "  constructor({});",
            render_dts_fields_as_params(&variant.fields)?
        ));
        lines.push("}".to_string());
    }

    Ok(lines.join("\n"))
}

fn render_js_object(object: &ObjectModel) -> Result<String> {
    let factory_name = object_factory_name(&object.name);
    let mut lines = vec![format!(
        "export class {} extends UniffiObjectBase {{",
        object.name
    )];

    if let Some(primary_constructor) = object
        .constructors
        .iter()
        .find(|constructor| constructor.is_primary && !constructor.is_async)
    {
        lines.push(format!(
            "  constructor({}) {{",
            render_js_params(&primary_constructor.arguments)
        ));
        lines.push("    super();".to_string());
    } else {
        lines.push("  constructor() {".to_string());
        lines.push("    super();".to_string());
    }
    lines.push(format!(
        "    return uniffiNotImplemented({});",
        json_string_literal(&format!("{}.constructor", object.name))?
    ));
    lines.push("  }".to_string());

    for constructor in &object.constructors {
        if constructor.is_primary && !constructor.is_async {
            continue;
        }
        lines.push(String::new());
        lines.push(format!(
            "  static {}{}({}) {{",
            if constructor.is_async { "async " } else { "" },
            js_member_identifier(&constructor.name),
            render_js_params(&constructor.arguments)
        ));
        lines.push(format!(
            "    return uniffiNotImplemented({});",
            json_string_literal(&format!("{}.{}", object.name, constructor.name))?
        ));
        lines.push("  }".to_string());
    }

    for method in &object.methods {
        lines.push(String::new());
        lines.push(format!(
            "  {}{}({}) {{",
            if method.is_async { "async " } else { "" },
            js_member_identifier(&method.name),
            render_js_params(&method.arguments)
        ));
        lines.push(format!(
            "    return uniffiNotImplemented({});",
            json_string_literal(&format!("{}.{}", object.name, method.name))?
        ));
        lines.push("  }".to_string());
    }

    lines.push("}".to_string());
    lines.push(String::new());
    lines.push(format!("const {} = createObjectFactory({{", factory_name));
    lines.push(format!(
        "  typeName: {},",
        json_string_literal(&object.name)?
    ));
    lines.push(format!(
        "  createInstance: () => Object.create({}.prototype),",
        object.name
    ));
    lines.push("  cloneHandle(handle) {".to_string());
    lines.push("    return defaultRustCaller.rustCall(".to_string());
    lines.push(format!(
        "      (status) => ffiFunctions.{}(handle, status),",
        object.ffi_object_clone_identifier
    ));
    lines.push("      uniffiRustCallOptions(),".to_string());
    lines.push("    );".to_string());
    lines.push("  },".to_string());
    lines.push("  freeHandle(handle) {".to_string());
    lines.push("    defaultRustCaller.rustCall(".to_string());
    lines.push(format!(
        "      (status) => ffiFunctions.{}(handle, status),",
        object.ffi_object_free_identifier
    ));
    lines.push("      uniffiRustCallOptions(),".to_string());
    lines.push("    );".to_string());
    lines.push("  },".to_string());
    lines.push("});".to_string());
    Ok(lines.join("\n"))
}

fn render_dts_object(object: &ObjectModel) -> Result<String> {
    let mut lines = vec![format!(
        "export declare class {} extends UniffiObjectBase {{",
        object.name
    )];

    if let Some(primary_constructor) = object
        .constructors
        .iter()
        .find(|constructor| constructor.is_primary && !constructor.is_async)
    {
        lines.push(format!(
            "  constructor({});",
            render_dts_params(&primary_constructor.arguments)?
        ));
    } else {
        lines.push("  protected constructor();".to_string());
    }

    for constructor in &object.constructors {
        if constructor.is_primary && !constructor.is_async {
            continue;
        }
        lines.push(format!(
            "  static {}({}): {};",
            js_member_identifier(&constructor.name),
            render_dts_params(&constructor.arguments)?,
            render_named_return_type(&object.name, constructor.is_async)
        ));
    }

    for method in &object.methods {
        lines.push(format!(
            "  {}({}): {};",
            js_member_identifier(&method.name),
            render_dts_params(&method.arguments)?,
            render_return_type(method.return_type.as_ref(), method.is_async)?
        ));
    }

    lines.push("}".to_string());
    Ok(lines.join("\n"))
}

fn render_js_params(arguments: &[ArgumentModel]) -> String {
    arguments
        .iter()
        .map(|argument| js_identifier(&argument.name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_js_object_runtime_helpers(ffi_rustbuffer_free_identifier: &str) -> String {
    format!(
        "function uniffiFreeRustBuffer(buffer) {{\n  return defaultRustCaller.rustCall(\n    (status) => ffiFunctions.{ffi_rustbuffer_free_identifier}(buffer, status),\n    {{ liftString: FfiConverterString.lift }},\n  );\n}}\n\nfunction uniffiRustCallOptions() {{\n  return {{\n    freeRustBuffer: uniffiFreeRustBuffer,\n    liftString: FfiConverterString.lift,\n  }};\n}}"
    )
}

fn render_dts_params(arguments: &[ArgumentModel]) -> Result<String> {
    arguments
        .iter()
        .map(|argument| {
            Ok(format!(
                "{}: {}",
                js_identifier(&argument.name),
                render_public_type(&argument.type_)?
            ))
        })
        .collect::<Result<Vec<_>>>()
        .map(|params| params.join(", "))
}

fn render_js_fields_as_params(fields: &[FieldModel]) -> String {
    fields
        .iter()
        .map(|field| js_identifier(&field.name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_dts_fields_as_params(fields: &[FieldModel]) -> Result<String> {
    fields
        .iter()
        .map(|field| {
            Ok(format!(
                "{}: {}",
                js_identifier(&field.name),
                render_public_type(&field.type_)?
            ))
        })
        .collect::<Result<Vec<_>>>()
        .map(|params| params.join(", "))
}

fn render_return_type(return_type: Option<&Type>, is_async: bool) -> Result<String> {
    let type_name = match return_type {
        Some(return_type) => render_public_type(return_type)?,
        None => "void".to_string(),
    };
    if is_async {
        Ok(format!("Promise<{type_name}>"))
    } else {
        Ok(type_name)
    }
}

fn render_named_return_type(type_name: &str, is_async: bool) -> String {
    if is_async {
        format!("Promise<{type_name}>")
    } else {
        type_name.to_string()
    }
}

fn variant_type_name(type_name: &str, variant_name: &str) -> String {
    format!("{type_name}{}", variant_name.to_upper_camel_case())
}

fn object_factory_name(type_name: &str) -> String {
    format!("uniffi{}ObjectFactory", type_name.to_upper_camel_case())
}

fn quoted_property_name(name: &str) -> Result<String> {
    json_string_literal(name)
}

fn json_string_literal(value: &str) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

fn js_identifier(name: &str) -> String {
    sanitize_identifier(name, false)
}

fn js_member_identifier(name: &str) -> String {
    sanitize_identifier(name, true)
}

fn ffi_symbol_identifier(name: &str) -> String {
    let mut identifier = String::new();
    for (index, character) in name.chars().enumerate() {
        let valid = if index == 0 {
            is_identifier_start(character)
        } else {
            is_identifier_continue(character)
        };
        if valid {
            identifier.push(character);
        } else {
            identifier.push('_');
        }
    }

    if identifier.is_empty() {
        "_".to_string()
    } else if !identifier.chars().next().is_some_and(is_identifier_start) {
        format!("_{identifier}")
    } else {
        identifier
    }
}

fn sanitize_identifier(name: &str, allow_reserved: bool) -> String {
    let mut identifier = String::new();
    for (index, character) in name.chars().enumerate() {
        let valid = if index == 0 {
            is_identifier_start(character)
        } else {
            is_identifier_continue(character)
        };
        if valid {
            identifier.push(character);
        } else {
            identifier.push('_');
        }
    }

    if identifier.is_empty() {
        identifier.push('_');
    }

    if !identifier.chars().next().is_some_and(is_identifier_start) {
        identifier.insert(0, '_');
    }

    if !allow_reserved && is_reserved_identifier(&identifier) {
        identifier.push('_');
    }

    identifier
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character == '$' || character.is_ascii_alphabetic()
}

fn is_identifier_continue(character: char) -> bool {
    is_identifier_start(character) || character.is_ascii_digit()
}

fn is_reserved_identifier(identifier: &str) -> bool {
    matches!(
        identifier,
        "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "function"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "new"
            | "null"
            | "return"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
    )
}

fn validate_arguments_renderable(arguments: &[ArgumentModel], context: &str) -> Result<()> {
    for argument in arguments {
        validate_type_renderable(
            &argument.type_,
            &format!("{context} argument {}", argument.name),
        )?;
    }
    Ok(())
}

fn validate_optional_type_renderable(type_: Option<&Type>, context: &str) -> Result<()> {
    if let Some(type_) = type_ {
        validate_type_renderable(type_, context)?;
    }
    Ok(())
}

fn validate_type_renderable(type_: &Type, context: &str) -> Result<()> {
    render_public_type(type_)
        .with_context(|| format!("unsupported public Node API type for {context}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uniffi_bindgen::interface::AsType;

    #[test]
    fn component_model_collects_objects_records_enums_errors_and_functions() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {
                string greet(string name);
                void init_logging(Logger? callback);
            };

            dictionary Profile {
                string name;
                u32 age;
            };

            enum Flavor {
                "vanilla",
                "chocolate"
            };

            [Enum]
            interface Outcome {
                Success(string value);
                Missing();
            };

            [Error]
            interface StoreError {
                NotFound();
                Conflict(string message);
            };

            callback interface Logger {
                void write(string message);
            };

            interface Store {
                constructor();
                [Async] Profile fetch(string key);
                void put(string key, Profile profile);
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");

        let model = ComponentModel::from_ci(&ci).expect("component model should build");

        assert_eq!(model.functions.len(), 2);
        assert_eq!(model.functions[0].name, "greet");
        assert_eq!(model.records.len(), 1);
        assert_eq!(model.records[0].name, "Profile");
        assert_eq!(model.flat_enums.len(), 1);
        assert_eq!(model.flat_enums[0].name, "Flavor");
        assert_eq!(model.tagged_enums.len(), 1);
        assert_eq!(model.tagged_enums[0].name, "Outcome");
        assert_eq!(model.errors.len(), 1);
        assert_eq!(model.errors[0].name, "StoreError");
        assert_eq!(model.callback_interfaces.len(), 1);
        assert_eq!(model.callback_interfaces[0].name, "Logger");
        assert_eq!(model.callback_interfaces[0].methods.len(), 1);
        assert_eq!(model.objects.len(), 1);
        assert_eq!(model.objects[0].name, "Store");
        assert_eq!(model.objects[0].constructors.len(), 1);
        assert_eq!(model.objects[0].methods.len(), 2);
        assert!(model.objects[0].methods[0].is_async);
    }

    #[test]
    fn component_model_rejects_custom_types() {
        let ci = ComponentInterface::from_webidl(
            r#"
            [Custom]
            typedef string Url;

            namespace example {
                Url parse(string value);
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");

        let error = ComponentModel::from_ci(&ci).expect_err("custom types should be rejected");

        assert!(
            error.to_string().contains("custom types are not supported"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn component_model_rejects_external_types() {
        let ci = ComponentInterface::from_webidl(
            r#"
            [External="other-crate"]
            typedef enum ExternalThing;

            namespace example {
                ExternalThing get_thing();
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");

        let error = ComponentModel::from_ci(&ci).expect_err("external types should be rejected");

        assert!(
            error
                .to_string()
                .contains("external types are not supported"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn component_model_collects_synchronous_callback_interfaces() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {
                void init_logging(Logger? callback);
            };

            callback interface Logger {
                void write(string message);
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");

        let model = ComponentModel::from_ci(&ci).expect("callback interfaces should build");

        assert_eq!(model.callback_interfaces.len(), 1);
        assert_eq!(model.callback_interfaces[0].name, "Logger");
        assert_eq!(model.callback_interfaces[0].methods[0].name, "write");
        assert_eq!(
            model.functions[0].arguments[0].type_,
            Type::Optional {
                inner_type: Box::new(Type::CallbackInterface {
                    name: "Logger".to_string(),
                    module_path: "fixture_crate".to_string(),
                }),
            }
        );
    }

    #[test]
    fn component_model_rejects_async_callback_interfaces() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {};

            callback interface Logger {
                [Async] void write(string message);
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");

        let error =
            ComponentModel::from_ci(&ci).expect_err("async callback interfaces should fail");

        assert!(
            error
                .to_string()
                .contains("async callback-interface methods are not supported"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn field_and_argument_models_clone_their_uniffi_types() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {
                void store(sequence<u8>? value);
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");

        let model = ComponentModel::from_ci(&ci).expect("component model should build");

        assert_eq!(
            model.functions[0].arguments[0].type_,
            Type::Optional {
                inner_type: Box::new(Type::Sequence {
                    inner_type: Box::new(Type::UInt8),
                }),
            }
        );
        assert_eq!(model.functions[0].arguments[0].type_.as_type().name(), None);
    }

    #[test]
    fn render_public_type_maps_slatedb_primitives_and_collections() {
        assert_eq!(render_public_type(&Type::Bytes).unwrap(), "Uint8Array");
        assert_eq!(render_public_type(&Type::UInt64).unwrap(), "bigint");
        assert_eq!(render_public_type(&Type::Int64).unwrap(), "bigint");
        assert_eq!(
            render_public_type(&Type::Optional {
                inner_type: Box::new(Type::Bytes),
            })
            .unwrap(),
            "Uint8Array | undefined"
        );
        assert_eq!(
            render_public_type(&Type::Sequence {
                inner_type: Box::new(Type::Optional {
                    inner_type: Box::new(Type::UInt64),
                }),
            })
            .unwrap(),
            "Array<bigint | undefined>"
        );
        assert_eq!(
            render_public_type(&Type::Map {
                key_type: Box::new(Type::String),
                value_type: Box::new(Type::Sequence {
                    inner_type: Box::new(Type::Bytes),
                }),
            })
            .unwrap(),
            "Map<string, Array<Uint8Array>>"
        );
        assert_eq!(
            render_public_type(&Type::CallbackInterface {
                name: "LogCallback".to_string(),
                module_path: "crate::logging".to_string(),
            })
            .unwrap(),
            "LogCallback"
        );
    }

    #[test]
    fn render_public_type_handles_nested_slatedb_combinations() {
        assert_eq!(
            render_public_type(&Type::Optional {
                inner_type: Box::new(Type::Sequence {
                    inner_type: Box::new(Type::Sequence {
                        inner_type: Box::new(Type::Bytes),
                    }),
                }),
            })
            .unwrap(),
            "Array<Array<Uint8Array>> | undefined"
        );
        assert_eq!(
            render_public_type(&Type::Map {
                key_type: Box::new(Type::String),
                value_type: Box::new(Type::Optional {
                    inner_type: Box::new(Type::Int64),
                }),
            })
            .unwrap(),
            "Map<string, bigint | undefined>"
        );
    }

    #[test]
    fn render_public_type_rejects_timestamp_until_runtime_exists() {
        let error = render_public_type(&Type::Timestamp)
            .expect_err("timestamps should be rejected until the runtime exists");
        assert!(
            error.to_string().contains("timestamps are not supported"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn render_public_api_emits_js_and_dts_skeletons() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {
                string greet(string name);
            };

            dictionary Profile {
                string name;
                bytes bytes;
            };

            enum Flavor {
                "vanilla",
                "chocolate"
            };

            [Enum]
            interface Outcome {
                Success(string value);
                Missing();
            };

            [Error]
            interface StoreError {
                NotFound();
                Conflict(string message);
            };

            interface Store {
                constructor();
                [Async] Profile fetch(string key);
                void put(string key, Profile profile);
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");

        let rendered = ComponentModel::from_ci(&ci)
            .expect("component model should build")
            .render_public_api()
            .expect("public API should render");

        assert!(
            rendered.js.contains("export function greet(name)"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("export const Flavor = Object.freeze({"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("export class StoreErrorConflict extends StoreError"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("export class Store extends UniffiObjectBase {"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("extends UniffiObjectBase"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("const uniffiStoreObjectFactory = createObjectFactory({"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.dts.contains(
                "export interface Profile {\n  \"name\": string;\n  \"bytes\": Uint8Array;\n}"
            ),
            "unexpected DTS output: {}",
            rendered.dts
        );
        assert!(
            rendered
                .dts
                .contains("export declare class Store extends UniffiObjectBase {"),
            "unexpected DTS output: {}",
            rendered.dts
        );
        assert!(
            rendered
                .dts
                .contains("export type Outcome = OutcomeSuccess | OutcomeMissing;"),
            "unexpected DTS output: {}",
            rendered.dts
        );
        assert!(
            rendered
                .dts
                .contains("fetch(key: string): Promise<Profile>;"),
            "unexpected DTS output: {}",
            rendered.dts
        );
    }

    #[test]
    fn render_public_api_keeps_async_primary_constructor_as_static_new() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {};

            interface AsyncStore {
                [Async] constructor(string path);
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");

        let rendered = ComponentModel::from_ci(&ci)
            .expect("component model should build")
            .render_public_api()
            .expect("public API should render");

        assert!(
            rendered.js.contains("static async new(path)"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .dts
                .contains("static new(path: string): Promise<AsyncStore>;"),
            "unexpected DTS output: {}",
            rendered.dts
        );
    }

    #[test]
    fn render_public_api_emits_callback_interface_types() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {
                void init_logging(LogCallback? callback);
            };

            callback interface LogCallback {
                void log(string message);
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");

        let rendered = ComponentModel::from_ci(&ci)
            .expect("component model should build")
            .render_public_api()
            .expect("public API should render");

        assert!(
            rendered
                .dts
                .contains("export interface LogCallback {\n  log(message: string): void;\n}"),
            "unexpected DTS output: {}",
            rendered.dts
        );
        assert!(
            rendered.dts.contains(
                "export declare function init_logging(callback: LogCallback | undefined): void;"
            ),
            "unexpected DTS output: {}",
            rendered.dts
        );
    }
}
