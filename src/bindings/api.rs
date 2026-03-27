use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};
use heck::ToUpperCamelCase;
use uniffi_bindgen::interface::{
    AsType, Callable, CallbackInterface, ComponentInterface, Constructor, Enum, Field, Function,
    Method, Object, Type, Variant,
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
    pub ffi_rustbuffer_from_bytes_identifier: String,
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

        let callback_interfaces = ci
            .callback_interface_definitions()
            .iter()
            .map(|callback_interface| {
                CallbackInterfaceModel::from_callback_interface(callback_interface, ci)
            })
            .chain(
                ci.object_definitions()
                    .iter()
                    .filter(|object| object.has_callback_interface())
                    .map(|object| CallbackInterfaceModel::from_object(object, ci)),
            )
            .collect::<Vec<_>>();
        let callback_interface_names = callback_interfaces
            .iter()
            .map(|callback_interface| callback_interface.name.clone())
            .collect::<BTreeSet<_>>();

        let model = Self {
            functions: ci
                .function_definitions()
                .iter()
                .map(|function| FunctionModel::from_function(function, ci))
                .collect(),
            records: ci
                .record_definitions()
                .map(RecordModel::from_record)
                .collect(),
            flat_enums,
            tagged_enums,
            errors,
            callback_interfaces,
            objects: ci
                .object_definitions()
                .iter()
                .filter(|object| {
                    !callback_interface_names.contains(object.name())
                        && !object.has_callback_interface()
                })
                .map(|object| ObjectModel::from_object(object, ci))
                .collect(),
            ffi_rustbuffer_from_bytes_identifier: ffi_symbol_identifier(
                ci.ffi_rustbuffer_from_bytes().name(),
            ),
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

        if !self.objects.is_empty() || self.has_placeholder_converters() {
            js_sections.push(render_js_runtime_helpers(
                &self.ffi_rustbuffer_from_bytes_identifier,
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

        if self.has_placeholder_converters() {
            js_sections.push(self.render_js_placeholder_converters()?);
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

    fn has_placeholder_converters(&self) -> bool {
        !self.records.is_empty()
            || !self.flat_enums.is_empty()
            || !self.tagged_enums.is_empty()
            || !self.errors.is_empty()
            || !self.callback_interfaces.is_empty()
    }

    fn render_js_placeholder_converters(&self) -> Result<String> {
        let mut lines = Vec::new();

        for record in &self.records {
            lines.push(render_js_record_converter(record)?);
        }
        for enum_def in &self.flat_enums {
            lines.push(render_js_flat_enum_converter(enum_def)?);
        }
        for enum_def in &self.tagged_enums {
            lines.push(render_js_tagged_enum_converter(enum_def)?);
        }
        for error in &self.errors {
            lines.push(render_js_error_converter(error)?);
        }
        for callback_interface in &self.callback_interfaces {
            lines.push(render_js_callback_interface_converter(callback_interface)?);
        }

        if !self.callback_interfaces.is_empty() {
            lines.push(render_js_callback_runtime_hooks(&self.callback_interfaces)?);
        }

        Ok(lines.join("\n"))
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
        Type::UInt64 | Type::Int64 => Ok("bigint | number".to_string()),
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
    pub ffi_func_identifier: String,
    pub async_ffi: Option<AsyncScaffoldingModel>,
}

impl FunctionModel {
    fn from_function(function: &Function, ci: &ComponentInterface) -> Self {
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
            ffi_func_identifier: ffi_symbol_identifier(function.ffi_func().name()),
            async_ffi: AsyncScaffoldingModel::from_callable(function, ci),
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
    pub ffi_init_callback_identifier: String,
    pub ffi_object_clone_identifier: String,
    pub ffi_object_free_identifier: String,
}

impl CallbackInterfaceModel {
    fn from_callback_interface(
        callback_interface: &CallbackInterface,
        ci: &ComponentInterface,
    ) -> Self {
        Self {
            name: callback_interface.name().to_string(),
            methods: callback_interface
                .methods()
                .into_iter()
                .enumerate()
                .map(|(index, method)| {
                    MethodModel::from_callback_method(method, ci, callback_interface.name(), index)
                })
                .collect(),
            ffi_init_callback_identifier: ffi_symbol_identifier(
                callback_interface.ffi_init_callback().name(),
            ),
            ffi_object_clone_identifier: ffi_symbol_identifier(&ffi_clone_symbol_name(
                callback_interface.module_path(),
                callback_interface.name(),
            )),
            ffi_object_free_identifier: ffi_symbol_identifier(&ffi_free_symbol_name(
                callback_interface.module_path(),
                callback_interface.name(),
            )),
        }
    }

    fn from_object(object: &Object, ci: &ComponentInterface) -> Self {
        Self {
            name: object.name().to_string(),
            methods: object
                .methods()
                .into_iter()
                .enumerate()
                .map(|(index, method)| {
                    MethodModel::from_callback_method(method, ci, object.name(), index)
                })
                .collect(),
            ffi_init_callback_identifier: ffi_symbol_identifier(object.ffi_init_callback().name()),
            ffi_object_clone_identifier: ffi_symbol_identifier(object.ffi_object_clone().name()),
            ffi_object_free_identifier: ffi_symbol_identifier(object.ffi_object_free().name()),
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
    fn from_object(object: &Object, ci: &ComponentInterface) -> Self {
        Self {
            name: object.name().to_string(),
            constructors: object
                .constructors()
                .into_iter()
                .map(|constructor| ConstructorModel::from_constructor(constructor, ci))
                .collect(),
            methods: object
                .methods()
                .into_iter()
                .map(|method| MethodModel::from_method(method, ci))
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
    pub ffi_func_identifier: String,
    pub async_ffi: Option<AsyncScaffoldingModel>,
}

impl ConstructorModel {
    fn from_constructor(constructor: &Constructor, ci: &ComponentInterface) -> Self {
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
            ffi_func_identifier: ffi_symbol_identifier(constructor.ffi_func().name()),
            async_ffi: AsyncScaffoldingModel::from_callable(constructor, ci),
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
    pub ffi_func_identifier: String,
    pub ffi_callback_identifier: Option<String>,
    pub async_ffi: Option<AsyncScaffoldingModel>,
}

impl MethodModel {
    fn from_method(method: &Method, ci: &ComponentInterface) -> Self {
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
            ffi_func_identifier: ffi_symbol_identifier(method.ffi_func().name()),
            ffi_callback_identifier: None,
            async_ffi: AsyncScaffoldingModel::from_callable(method, ci),
        }
    }

    fn from_callback_method(
        method: &Method,
        ci: &ComponentInterface,
        callback_interface_name: &str,
        index: usize,
    ) -> Self {
        let mut model = Self::from_method(method, ci);
        model.ffi_callback_identifier = Some(ffi_symbol_identifier(&callback_method_ffi_name(
            callback_interface_name,
            index,
        )));
        model
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsyncScaffoldingModel {
    pub poll_identifier: String,
    pub cancel_identifier: String,
    pub complete_identifier: String,
    pub free_identifier: String,
}

impl AsyncScaffoldingModel {
    fn from_callable<T: Callable>(callable: &T, ci: &ComponentInterface) -> Option<Self> {
        callable.is_async().then(|| Self {
            poll_identifier: ffi_symbol_identifier(&callable.ffi_rust_future_poll(ci)),
            cancel_identifier: ffi_symbol_identifier(&callable.ffi_rust_future_cancel(ci)),
            complete_identifier: ffi_symbol_identifier(&callable.ffi_rust_future_complete(ci)),
            free_identifier: ffi_symbol_identifier(&callable.ffi_rust_future_free(ci)),
        })
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
    let mut lines = vec![format!(
        "export {}function {}({}) {{",
        if function.is_async { "async " } else { "" },
        js_identifier(&function.name),
        render_js_params(&function.arguments)
    )];

    if function.is_async {
        lines.extend(render_js_async_function_body(function)?);
    } else {
        lines.extend(render_js_sync_function_body(function)?);
    }
    lines.push("}".to_string());

    Ok(lines.join("\n"))
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

fn render_js_record_converter(record: &RecordModel) -> Result<String> {
    let converter_name = type_converter_name(&record.name);
    let record_type_name = json_string_literal(&record.name)?;
    let mut lines = vec![format!(
        "const {} = new (class extends AbstractFfiConverterByteArray {{",
        converter_name
    )];
    lines.push("  allocationSize(value) {".to_string());
    lines.push(format!(
        "    const recordValue = uniffiRequireRecordObject({}, value);",
        record_type_name
    ));
    lines.push(format!(
        "    return {};",
        render_js_record_allocation_size_expression(record)?
    ));
    lines.push("  }".to_string());
    lines.push(String::new());
    lines.push("  write(value, writer) {".to_string());
    lines.push(format!(
        "    const recordValue = uniffiRequireRecordObject({}, value);",
        record_type_name
    ));
    for field in &record.fields {
        lines.push(format!(
            "    {}.write({}, writer);",
            render_js_type_converter_expression(&field.type_)?,
            render_js_property_access("recordValue", &field.name)?
        ));
    }
    lines.push("  }".to_string());
    lines.push(String::new());
    lines.push("  read(reader) {".to_string());
    lines.push("    return {".to_string());
    for field in &record.fields {
        lines.push(format!(
            "      {}: {}.read(reader),",
            quoted_property_name(&field.name)?,
            render_js_type_converter_expression(&field.type_)?
        ));
    }
    lines.push("    };".to_string());
    lines.push("  }".to_string());
    lines.push("})();".to_string());
    Ok(lines.join("\n"))
}

fn render_js_callback_interface_converter(
    callback_interface: &CallbackInterfaceModel,
) -> Result<String> {
    let proxy_class_name = callback_interface_proxy_class_name(&callback_interface.name);
    let factory_name = callback_interface_factory_name(&callback_interface.name);
    let registry_name = callback_interface_registry_name(&callback_interface.name);
    let validator_name = callback_interface_validator_name(&callback_interface.name);
    let register_name = callback_interface_register_name(&callback_interface.name);
    let converter_name = type_converter_name(&callback_interface.name);
    let interface_name = json_string_literal(&callback_interface.name)?;
    let mut lines = vec![format!(
        "class {} extends UniffiObjectBase {{",
        proxy_class_name
    )];

    for method in &callback_interface.methods {
        lines.push(String::new());
        lines.push(format!(
            "  {}({}) {{",
            js_member_identifier(&method.name),
            render_js_params(&method.arguments)
        ));
        lines.extend(render_js_sync_method_body(
            method,
            &factory_name,
            &callback_interface.name,
        )?);
        lines.push("  }".to_string());
    }
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push(format!("const {} = createObjectFactory({{", factory_name));
    lines.push(format!("  typeName: {},", interface_name));
    lines.push(format!(
        "  createInstance: () => Object.create({}.prototype),",
        proxy_class_name
    ));
    lines.push("  cloneHandle(handle) {".to_string());
    lines.push("    return defaultRustCaller.rustCall(".to_string());
    lines.push(format!(
        "      (status) => ffiFunctions.{}(handle, status),",
        callback_interface.ffi_object_clone_identifier
    ));
    lines.push("      uniffiRustCallOptions(),".to_string());
    lines.push("    );".to_string());
    lines.push("  },".to_string());
    lines.push("  freeHandle(handle) {".to_string());
    lines.push("    defaultRustCaller.rustCall(".to_string());
    lines.push(format!(
        "      (status) => ffiFunctions.{}(handle, status),",
        callback_interface.ffi_object_free_identifier
    ));
    lines.push("      uniffiRustCallOptions(),".to_string());
    lines.push("    );".to_string());
    lines.push("  },".to_string());
    lines.push("});".to_string());
    lines.push(String::new());
    lines.push(format!("function {}(implementation) {{", validator_name));
    lines.push(format!(
        "  if ((typeof implementation !== \"object\" && typeof implementation !== \"function\") || implementation == null) {{"
    ));
    lines.push(format!(
        "    throw new TypeError(`${{{}}} implementations must be objects with callable methods.`);",
        interface_name
    ));
    lines.push("  }".to_string());
    for method in &callback_interface.methods {
        lines.push(format!(
            "  if (typeof implementation.{} !== \"function\") {{",
            js_member_identifier(&method.name)
        ));
        lines.push(format!(
            "    throw new TypeError(`${{{}}} is missing required method {}().`);",
            interface_name,
            js_member_identifier(&method.name)
        ));
        lines.push("  }".to_string());
    }
    lines.push("  return implementation;".to_string());
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push(format!(
        "const {} = createCallbackRegistry({{",
        registry_name
    ));
    lines.push(format!("  interfaceName: {},", interface_name));
    lines.push(format!("  validate: {},", validator_name));
    lines.push("});".to_string());
    lines.push(String::new());
    lines.push(format!(
        "function {}(bindings, registrations) {{",
        register_name
    ));

    for method in &callback_interface.methods {
        let callback_identifier = method.ffi_callback_identifier.as_deref().with_context(|| {
            format!(
                "callback interface {}.{} is missing an FFI callback identifier",
                callback_interface.name, method.name
            )
        })?;
        lines.push(format!(
            "  const {}Callback = koffi.register(",
            js_member_identifier(&method.name)
        ));
        lines.push(
            "    (uniffiHandle, ".to_string()
                + &method
                    .arguments
                    .iter()
                    .map(|argument| js_identifier(&argument.name))
                    .chain(["uniffiOutReturn".to_string(), "callStatus".to_string()].into_iter())
                    .collect::<Vec<_>>()
                    .join(", ")
                + ") => {",
        );
        lines.push(
            "      const uniffiStatus = callStatus == null
        ? createRustCallStatus()
        : koffi.decode(callStatus, bindings.ffiTypes.RustCallStatus);"
                .to_string(),
        );
        lines.push("      const uniffiResult = invokeCallbackMethod({".to_string());
        lines.push(format!("        registry: {},", registry_name));
        lines.push("        handle: uniffiHandle,".to_string());
        lines.push(format!(
            "        methodName: {},",
            json_string_literal(&method.name)?
        ));
        lines.push("        args: [".to_string());
        for argument in &method.arguments {
            lines.push(format!(
                "          {},",
                render_js_lift_expression(&argument.type_, &js_identifier(&argument.name))?
            ));
        }
        lines.push("        ],".to_string());
        if let Some(throws_type) = method.throws_type.as_ref() {
            lines.push(format!(
                "        lowerError: (error) => error instanceof {} ? uniffiLowerIntoRustBuffer({}, error) : null,",
                render_public_type(throws_type)?,
                render_js_type_converter_expression(throws_type)?
            ));
        }
        lines.push(
            "        lowerString: (value) => uniffiLowerIntoRustBuffer(FfiConverterString, value),"
                .to_string(),
        );
        lines.push("        status: uniffiStatus,".to_string());
        lines.push("      });".to_string());
        lines.push("      if (callStatus != null) {".to_string());
        lines.push(
            "        koffi.encode(callStatus, bindings.ffiTypes.RustCallStatus, uniffiStatus);"
                .to_string(),
        );
        lines.push("      }".to_string());
        lines.push("      if (uniffiStatus.code !== CALL_SUCCESS) {".to_string());
        lines.push("        return;".to_string());
        lines.push("      }".to_string());
        if let Some(return_type) = method.return_type.as_ref() {
            lines.push(format!(
                "      const loweredReturn = {};",
                render_js_lower_expression(return_type, "uniffiResult")?
            ));
            lines.push(format!(
                "      koffi.encode(uniffiOutReturn, {}, loweredReturn);",
                render_js_koffi_type_expression(return_type, "bindings")?
            ));
        }
        lines.push("    },".to_string());
        lines.push(format!(
            "    koffi.pointer(bindings.ffiCallbacks.{}),",
            callback_identifier
        ));
        lines.push("  );".to_string());
        lines.push(format!(
            "  registrations.push({}Callback);",
            js_member_identifier(&method.name)
        ));
    }

    lines.push("  const uniffiFree = koffi.register(".to_string());
    lines.push(format!(
        "    (uniffiHandle) => {}.remove(uniffiHandle),",
        registry_name
    ));
    lines.push("    koffi.pointer(bindings.ffiCallbacks.CallbackInterfaceFree),".to_string());
    lines.push("  );".to_string());
    lines.push("  registrations.push(uniffiFree);".to_string());
    lines.push(format!(
        "  bindings.ffiFunctions.{}({{",
        callback_interface.ffi_init_callback_identifier
    ));
    for method in &callback_interface.methods {
        lines.push(format!(
            "    {}: {}Callback,",
            quoted_property_name(&method.name)?,
            js_member_identifier(&method.name)
        ));
    }
    lines.push("    uniffi_free: uniffiFree,".to_string());
    lines.push("  });".to_string());
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push(format!("const {} = Object.freeze({{", converter_name));
    lines.push("  lower(value) {".to_string());
    lines.push(format!("    if ({}.isInstance(value)) {{", factory_name));
    lines.push(format!("      return {}.cloneHandle(value);", factory_name));
    lines.push("    }".to_string());
    lines.push(format!("    return {}.register(value);", registry_name));
    lines.push("  },".to_string());
    lines.push("  lift(handle) {".to_string());
    lines.push(format!("    return {}.create(handle);", factory_name));
    lines.push("  },".to_string());
    lines.push("  write(value, writer) {".to_string());
    lines.push("    writer.writeUInt64(this.lower(value));".to_string());
    lines.push("  },".to_string());
    lines.push("  read(reader) {".to_string());
    lines.push("    return this.lift(reader.readUInt64());".to_string());
    lines.push("  },".to_string());
    lines.push("  allocationSize() {".to_string());
    lines.push("    return UNIFFI_OBJECT_HANDLE_SIZE;".to_string());
    lines.push("  },".to_string());
    lines.push("});".to_string());

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

fn render_js_flat_enum_converter(enum_def: &EnumModel) -> Result<String> {
    let converter_name = type_converter_name(&enum_def.name);
    let enum_type_name = json_string_literal(&enum_def.name)?;
    let mut lines = vec![format!(
        "const {} = new (class extends AbstractFfiConverterByteArray {{",
        converter_name
    )];
    lines.push("  allocationSize(value) {".to_string());
    lines.push(format!(
        "    uniffiRequireFlatEnumValue({}, {}, value);",
        enum_def.name, enum_type_name
    ));
    lines.push("    return 4;".to_string());
    lines.push("  }".to_string());
    lines.push(String::new());
    lines.push("  write(value, writer) {".to_string());
    lines.push(format!(
        "    const enumValue = uniffiRequireFlatEnumValue({}, {}, value);",
        enum_def.name, enum_type_name
    ));
    lines.push("    switch (enumValue) {".to_string());
    for (index, variant) in enum_def.variants.iter().enumerate() {
        lines.push(format!(
            "      case {}:",
            render_js_property_access(&enum_def.name, &variant.name)?
        ));
        lines.push(format!("        writer.writeInt32({});", index + 1));
        lines.push("        return;".to_string());
    }
    lines.push("      default:".to_string());
    lines.push(format!(
        "        throw new UnexpectedEnumCase(`Unexpected {} case ${{String(enumValue)}}.`);",
        enum_def.name
    ));
    lines.push("    }".to_string());
    lines.push("  }".to_string());
    lines.push(String::new());
    lines.push("  read(reader) {".to_string());
    lines.push("    const enumTag = reader.readInt32();".to_string());
    lines.push("    switch (enumTag) {".to_string());
    for (index, variant) in enum_def.variants.iter().enumerate() {
        lines.push(format!("      case {}:", index + 1));
        lines.push(format!(
            "        return {};",
            render_js_property_access(&enum_def.name, &variant.name)?
        ));
    }
    lines.push("      default:".to_string());
    lines.push(format!(
        "        throw new UnexpectedEnumCase(`Unexpected {} case ${{String(enumTag)}}.`);",
        enum_def.name
    ));
    lines.push("    }".to_string());
    lines.push("  }".to_string());
    lines.push("})();".to_string());
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

fn render_js_tagged_enum_converter(enum_def: &EnumModel) -> Result<String> {
    let converter_name = type_converter_name(&enum_def.name);
    let enum_type_name = json_string_literal(&enum_def.name)?;
    let mut lines = vec![format!(
        "const {} = new (class extends AbstractFfiConverterByteArray {{",
        converter_name
    )];
    lines.push("  allocationSize(value) {".to_string());
    lines.push(format!(
        "    const enumValue = uniffiRequireTaggedEnumValue({}, value);",
        enum_type_name
    ));
    lines.push("    switch (enumValue.tag) {".to_string());
    for (index, variant) in enum_def.variants.iter().enumerate() {
        let field_terms = variant
            .fields
            .iter()
            .map(|field| {
                Ok(format!(
                    "{}.allocationSize({})",
                    render_js_type_converter_expression(&field.type_)?,
                    render_js_property_access("enumValue", &field.name)?
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let allocation_size = if field_terms.is_empty() {
            "4".to_string()
        } else {
            format!("4 + {}", field_terms.join(" + "))
        };
        lines.push(format!(
            "      case {}:",
            json_string_literal(&variant.name)?
        ));
        lines.push(format!("        return {};", allocation_size));
        if index + 1 == enum_def.variants.len() {
            lines.push("      default:".to_string());
            lines.push(format!(
                "        throw new UnexpectedEnumCase(`Unexpected {} case ${{String(enumValue.tag)}}.`);",
                enum_def.name
            ));
        }
    }
    lines.push("    }".to_string());
    lines.push("  }".to_string());
    lines.push(String::new());
    lines.push("  write(value, writer) {".to_string());
    lines.push(format!(
        "    const enumValue = uniffiRequireTaggedEnumValue({}, value);",
        enum_type_name
    ));
    lines.push("    switch (enumValue.tag) {".to_string());
    for (index, variant) in enum_def.variants.iter().enumerate() {
        lines.push(format!(
            "      case {}:",
            json_string_literal(&variant.name)?
        ));
        lines.push(format!("        writer.writeInt32({});", index + 1));
        for field in &variant.fields {
            lines.push(format!(
                "        {}.write({}, writer);",
                render_js_type_converter_expression(&field.type_)?,
                render_js_property_access("enumValue", &field.name)?
            ));
        }
        lines.push("        return;".to_string());
    }
    lines.push("      default:".to_string());
    lines.push(format!(
        "        throw new UnexpectedEnumCase(`Unexpected {} case ${{String(enumValue.tag)}}.`);",
        enum_def.name
    ));
    lines.push("    }".to_string());
    lines.push("  }".to_string());
    lines.push(String::new());
    lines.push("  read(reader) {".to_string());
    lines.push("    const enumTag = reader.readInt32();".to_string());
    lines.push("    switch (enumTag) {".to_string());
    for (index, variant) in enum_def.variants.iter().enumerate() {
        lines.push(format!("      case {}:", index + 1));
        let field_values = variant
            .fields
            .iter()
            .map(|field| {
                Ok(format!(
                    "{}.read(reader)",
                    render_js_type_converter_expression(&field.type_)?
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        lines.push(format!(
            "        return {}.{}({});",
            enum_def.name,
            js_member_identifier(&variant.name),
            field_values.join(", ")
        ));
    }
    lines.push("      default:".to_string());
    lines.push(format!(
        "        throw new UnexpectedEnumCase(`Unexpected {} case ${{String(enumTag)}}.`);",
        enum_def.name
    ));
    lines.push("    }".to_string());
    lines.push("  }".to_string());
    lines.push("})();".to_string());
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
        format!("export class {} extends globalThis.Error {{", error.name),
        "  constructor(tag, message = tag) {".to_string(),
        "    super(message);".to_string(),
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
        if error.is_flat {
            lines.push("  constructor(message = undefined) {".to_string());
            lines.push(format!(
                "    super({}, message ?? {});",
                json_string_literal(&variant.name)?,
                json_string_literal(&variant.name)?
            ));
        } else {
            lines.push(format!(
                "  constructor({}) {{",
                render_js_fields_as_params(&variant.fields)
            ));
            lines.push(format!(
                "    super({});",
                json_string_literal(&variant.name)?
            ));
        }
        lines.push(format!(
            "    this.name = {};",
            json_string_literal(&variant_class_name)?
        ));
        if error.is_flat {
            lines.push(format!(
                "    this.message = message ?? {};",
                json_string_literal(&variant.name)?
            ));
        } else {
            for field in &variant.fields {
                lines.push(format!(
                    "    this[{}] = {};",
                    json_string_literal(&field.name)?,
                    js_identifier(&field.name)
                ));
            }
        }
        lines.push("  }".to_string());
        lines.push("}".to_string());
    }

    Ok(lines.join("\n"))
}

fn render_js_error_converter(error: &ErrorModel) -> Result<String> {
    let converter_name = type_converter_name(&error.name);
    let allowed_variants = error
        .variants
        .iter()
        .map(|variant| variant_type_name(&error.name, &variant.name))
        .collect::<Vec<_>>()
        .join(", ");
    let mut lines = vec![format!(
        "const {} = new (class extends AbstractFfiConverterByteArray {{",
        converter_name
    )];
    lines.push("  allocationSize(value) {".to_string());
    for variant in &error.variants {
        let variant_class_name = variant_type_name(&error.name, &variant.name);
        let allocation_size = if error.is_flat || variant.fields.is_empty() {
            "4".to_string()
        } else {
            let field_terms = variant
                .fields
                .iter()
                .map(|field| {
                    Ok(format!(
                        "{}.allocationSize({})",
                        render_js_type_converter_expression(&field.type_)?,
                        render_js_property_access("value", &field.name)?
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            format!("4 + {}", field_terms.join(" + "))
        };
        lines.push(format!(
            "    if (value instanceof {}) {{",
            variant_class_name
        ));
        lines.push(format!("      return {};", allocation_size));
        lines.push("    }".to_string());
    }
    lines.push(format!(
        "    throw new TypeError({});",
        json_string_literal(&format!(
            "{} values must be instances of {}.",
            error.name, allowed_variants
        ))?
    ));
    lines.push("  }".to_string());
    lines.push(String::new());
    lines.push("  write(value, writer) {".to_string());
    for (index, variant) in error.variants.iter().enumerate() {
        let variant_class_name = variant_type_name(&error.name, &variant.name);
        lines.push(format!(
            "    if (value instanceof {}) {{",
            variant_class_name
        ));
        lines.push(format!("      writer.writeInt32({});", index + 1));
        if !error.is_flat {
            for field in &variant.fields {
                lines.push(format!(
                    "      {}.write({}, writer);",
                    render_js_type_converter_expression(&field.type_)?,
                    render_js_property_access("value", &field.name)?
                ));
            }
        }
        lines.push("      return;".to_string());
        lines.push("    }".to_string());
    }
    lines.push(format!(
        "    throw new TypeError({});",
        json_string_literal(&format!(
            "{} values must be instances of {}.",
            error.name, allowed_variants
        ))?
    ));
    lines.push("  }".to_string());
    lines.push(String::new());
    lines.push("  read(reader) {".to_string());
    lines.push("    const enumTag = reader.readInt32();".to_string());
    lines.push("    switch (enumTag) {".to_string());
    for (index, variant) in error.variants.iter().enumerate() {
        let variant_class_name = variant_type_name(&error.name, &variant.name);
        lines.push(format!("      case {}:", index + 1));
        if error.is_flat {
            lines.push(format!(
                "        return new {}({}.read(reader));",
                variant_class_name,
                render_js_type_converter_expression(&Type::String)?
            ));
        } else {
            let field_values = variant
                .fields
                .iter()
                .map(|field| {
                    Ok(format!(
                        "{}.read(reader)",
                        render_js_type_converter_expression(&field.type_)?
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            lines.push(format!(
                "        return new {}({});",
                variant_class_name,
                field_values.join(", ")
            ));
        }
    }
    lines.push("      default:".to_string());
    lines.push(format!(
        "        throw new UnexpectedEnumCase(`Unexpected {} case ${{String(enumTag)}}.`);",
        error.name
    ));
    lines.push("    }".to_string());
    lines.push("  }".to_string());
    lines.push("})();".to_string());
    Ok(lines.join("\n"))
}

fn render_dts_error(error: &ErrorModel) -> Result<String> {
    let mut lines = vec![
        format!("export declare class {} extends globalThis.Error {{", error.name),
        "  readonly tag: string;".to_string(),
        "  protected constructor(tag: string, message?: string);".to_string(),
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
        if error.is_flat {
            lines.push("  constructor(message?: string);".to_string());
        } else {
            lines.push(format!(
                "  constructor({});",
                render_dts_fields_as_params(&variant.fields)?
            ));
        }
        lines.push("}".to_string());
    }

    Ok(lines.join("\n"))
}

fn render_js_object(object: &ObjectModel) -> Result<String> {
    let factory_name = object_factory_name(&object.name);
    let converter_name = object_converter_name(&object.name);
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
        lines.extend(render_js_sync_constructor_body(
            primary_constructor,
            Some("this"),
            &factory_name,
        )?);
    } else {
        lines.push("  constructor() {".to_string());
        lines.push("    super();".to_string());
        lines.push(format!(
            "    return uniffiNotImplemented({});",
            json_string_literal(&format!("{}.constructor", object.name))?
        ));
    }
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
        lines.extend(render_js_constructor_body(
            constructor,
            &factory_name,
            &object.name,
        )?);
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
        if method.is_async {
            lines.extend(render_js_async_method_body(
                method,
                &factory_name,
                &object.name,
            )?);
        } else {
            lines.extend(render_js_sync_method_body(
                method,
                &factory_name,
                &object.name,
            )?);
        }
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
    lines.push(format!(
        "  handleType: () => getFfiBindings().ffiTypes.{},",
        ffi_opaque_identifier(&object.name)
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
    lines.push(format!(
        "const {} = createObjectConverter({});",
        converter_name, factory_name
    ));
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

fn render_js_runtime_helpers(
    ffi_rustbuffer_from_bytes_identifier: &str,
    ffi_rustbuffer_free_identifier: &str,
) -> String {
    format!(
        "function uniffiFreeRustBuffer(buffer) {{\n  return defaultRustCaller.rustCall(\n    (status) => ffiFunctions.{ffi_rustbuffer_free_identifier}(buffer, status),\n    {{ liftString: FfiConverterString.lift }},\n  );\n}}\n\nfunction uniffiRustCallOptions(errorConverter = undefined) {{\n  const options = {{\n    freeRustBuffer: uniffiFreeRustBuffer,\n    liftString: FfiConverterString.lift,\n  }};\n  if (errorConverter != null) {{\n    options.errorHandler = (errorBytes) => errorConverter.lift(errorBytes);\n  }}\n  return options;\n}}\n\nfunction uniffiLowerIntoRustBuffer(converter, value) {{\n  return defaultRustCaller.rustCall(\n    (status) => ffiFunctions.{ffi_rustbuffer_from_bytes_identifier}(createForeignBytes(converter.lower(value)), status),\n    uniffiRustCallOptions(),\n  );\n}}\n\nfunction uniffiLiftFromRustBuffer(converter, value) {{\n  return converter.lift(new RustBufferValue(value).consumeIntoUint8Array(uniffiFreeRustBuffer));\n}}\n\nfunction uniffiRequireRecordObject(typeName, value) {{\n  if (typeof value !== \"object\" || value == null) {{\n    throw new TypeError(`${{typeName}} values must be non-null objects.`);\n  }}\n  return value;\n}}\n\nfunction uniffiRequireFlatEnumValue(enumValues, typeName, value) {{\n  for (const enumValue of Object.values(enumValues)) {{\n    if (enumValue === value) {{\n      return enumValue;\n    }}\n  }}\n  throw new TypeError(`${{typeName}} values must be one of ${{Object.values(enumValues).map((item) => JSON.stringify(item)).join(\", \")}}.`);\n}}\n\nfunction uniffiRequireTaggedEnumValue(typeName, value) {{\n  const enumValue = uniffiRequireRecordObject(typeName, value);\n  if (typeof enumValue.tag !== \"string\") {{\n    throw new TypeError(`${{typeName}} values must be tagged objects with a string tag field.`);\n  }}\n  return enumValue;\n}}\n\nfunction uniffiNotImplementedConverter(typeName) {{\n  const fail = (member) => {{\n    throw new Error(`${{typeName}} converter ${{member}} is not implemented yet.`);\n  }};\n  return Object.freeze({{\n    lower() {{\n      return fail(\"lower\");\n    }},\n    lift() {{\n      return fail(\"lift\");\n    }},\n    write() {{\n      return fail(\"write\");\n    }},\n    read() {{\n      return fail(\"read\");\n    }},\n    allocationSize() {{\n      return fail(\"allocationSize\");\n    }},\n  }});\n}}"
    )
}

fn render_js_constructor_body(
    constructor: &ConstructorModel,
    factory_name: &str,
    object_name: &str,
) -> Result<Vec<String>> {
    if constructor.is_async {
        render_js_async_constructor_body(constructor, factory_name, object_name)
    } else {
        render_js_sync_constructor_body(constructor, None, factory_name)
    }
}

fn render_js_sync_constructor_body(
    constructor: &ConstructorModel,
    attach_target: Option<&str>,
    factory_name: &str,
) -> Result<Vec<String>> {
    let mut lines = render_js_argument_lowering(&constructor.arguments)?;
    let call_args = render_js_ffi_call_args(&constructor.arguments, Some("status"));
    lines.push("    const pointer = defaultRustCaller.rustCall(".to_string());
    lines.push(format!(
        "      (status) => ffiFunctions.{}({}),",
        constructor.ffi_func_identifier, call_args
    ));
    lines.push(format!(
        "      {},",
        render_js_rust_call_options_expression(constructor.throws_type.as_ref())?
    ));
    lines.push("    );".to_string());
    lines.push(match attach_target {
        Some(target) => format!("    return {}.attach({}, pointer);", factory_name, target),
        None => format!("    return {}.create(pointer);", factory_name),
    });
    Ok(lines)
}

fn render_js_async_constructor_body(
    constructor: &ConstructorModel,
    factory_name: &str,
    object_name: &str,
) -> Result<Vec<String>> {
    let async_ffi = constructor.async_ffi.as_ref().with_context(|| {
        format!(
            "async constructor {}.{} is missing future scaffolding identifiers",
            object_name, constructor.name
        )
    })?;
    let mut lines = render_js_argument_lowering(&constructor.arguments)?;
    let start_args = render_js_ffi_call_args(&constructor.arguments, None);
    lines.push("    return rustCallAsync({".to_string());
    lines.push(format!(
        "      rustFutureFunc: () => ffiFunctions.{}({}),",
        constructor.ffi_func_identifier, start_args
    ));
    lines.push(format!(
        "      pollFunc: (rustFuture, continuationCallback, continuationHandle) => ffiFunctions.{}(rustFuture, continuationCallback, continuationHandle),",
        async_ffi.poll_identifier
    ));
    lines.push(format!(
        "      cancelFunc: (rustFuture) => ffiFunctions.{}(rustFuture),",
        async_ffi.cancel_identifier
    ));
    lines.push(format!(
        "      completeFunc: (rustFuture, status) => ffiFunctions.{}(rustFuture, status),",
        async_ffi.complete_identifier
    ));
    lines.push(format!(
        "      freeFunc: (rustFuture) => ffiFunctions.{}(rustFuture),",
        async_ffi.free_identifier
    ));
    lines.push(format!(
        "      liftFunc: (pointer) => {}.create(pointer),",
        factory_name
    ));
    lines.push(format!(
        "      ...{},",
        render_js_rust_call_options_expression(constructor.throws_type.as_ref())?
    ));
    lines.push("    });".to_string());
    Ok(lines)
}

fn render_js_argument_lowering(arguments: &[ArgumentModel]) -> Result<Vec<String>> {
    arguments
        .iter()
        .map(|argument| {
            Ok(format!(
                "    const {} = {};",
                lowered_argument_name(&argument.name),
                render_js_lower_expression(&argument.type_, &js_identifier(&argument.name))?
            ))
        })
        .collect()
}

fn render_js_ffi_call_args(arguments: &[ArgumentModel], trailing: Option<&str>) -> String {
    let mut args = arguments
        .iter()
        .map(|argument| lowered_argument_name(&argument.name))
        .collect::<Vec<_>>();
    if let Some(trailing) = trailing {
        args.push(trailing.to_string());
    }
    args.join(", ")
}

fn render_js_ffi_call_args_with_leading(
    leading: &[String],
    arguments: &[ArgumentModel],
    trailing: Option<&str>,
) -> String {
    let mut args = leading.to_vec();
    args.extend(
        arguments
            .iter()
            .map(|argument| lowered_argument_name(&argument.name)),
    );
    if let Some(trailing) = trailing {
        args.push(trailing.to_string());
    }
    args.join(", ")
}

fn render_js_sync_method_body(
    method: &MethodModel,
    factory_name: &str,
    _object_name: &str,
) -> Result<Vec<String>> {
    let mut lines = vec![format!(
        "    const loweredSelf = {}.cloneHandle(this);",
        factory_name
    )];
    lines.extend(render_js_argument_lowering(&method.arguments)?);
    let call_args = render_js_ffi_call_args_with_leading(
        &[String::from("loweredSelf")],
        &method.arguments,
        Some("status"),
    );

    if let Some(return_type) = method.return_type.as_ref() {
        lines.push("    const uniffiResult = defaultRustCaller.rustCall(".to_string());
        lines.push(format!(
            "      (status) => ffiFunctions.{}({}),",
            method.ffi_func_identifier, call_args
        ));
        lines.push(format!(
            "      {},",
            render_js_rust_call_options_expression(method.throws_type.as_ref())?
        ));
        lines.push("    );".to_string());
        lines.push(format!(
            "    return {};",
            render_js_lift_expression(return_type, "uniffiResult")?
        ));
    } else {
        lines.push("    defaultRustCaller.rustCall(".to_string());
        lines.push(format!(
            "      (status) => ffiFunctions.{}({}),",
            method.ffi_func_identifier, call_args
        ));
        lines.push(format!(
            "      {},",
            render_js_rust_call_options_expression(method.throws_type.as_ref())?
        ));
        lines.push("    );".to_string());
    }

    Ok(lines)
}

fn render_js_async_method_body(
    method: &MethodModel,
    factory_name: &str,
    object_name: &str,
) -> Result<Vec<String>> {
    let async_ffi = method.async_ffi.as_ref().with_context(|| {
        format!(
            "async method {}.{} is missing future scaffolding identifiers",
            object_name, method.name
        )
    })?;
    let mut lines = vec![format!(
        "    const loweredSelf = {}.cloneHandle(this);",
        factory_name
    )];
    lines.extend(render_js_argument_lowering(&method.arguments)?);
    let start_args = render_js_ffi_call_args_with_leading(
        &[String::from("loweredSelf")],
        &method.arguments,
        None,
    );

    lines.push("    return rustCallAsync({".to_string());
    lines.push(format!(
        "      rustFutureFunc: () => ffiFunctions.{}({}),",
        method.ffi_func_identifier, start_args
    ));
    lines.push(format!(
        "      pollFunc: (rustFuture, continuationCallback, continuationHandle) => ffiFunctions.{}(rustFuture, continuationCallback, continuationHandle),",
        async_ffi.poll_identifier
    ));
    lines.push(format!(
        "      cancelFunc: (rustFuture) => ffiFunctions.{}(rustFuture),",
        async_ffi.cancel_identifier
    ));
    lines.push(format!(
        "      completeFunc: (rustFuture, status) => ffiFunctions.{}(rustFuture, status),",
        async_ffi.complete_identifier
    ));
    lines.push(format!(
        "      freeFunc: (rustFuture) => ffiFunctions.{}(rustFuture),",
        async_ffi.free_identifier
    ));
    lines.push(format!(
        "      liftFunc: {},",
        render_js_async_lift_closure(method.return_type.as_ref())?
    ));
    lines.push(format!(
        "      ...{},",
        render_js_rust_call_options_expression(method.throws_type.as_ref())?
    ));
    lines.push("    });".to_string());

    Ok(lines)
}

fn render_js_lower_expression(type_: &Type, value_expr: &str) -> Result<String> {
    match type_ {
        Type::UInt8 => Ok(format!("FfiConverterUInt8.lower({value_expr})")),
        Type::Int8 => Ok(format!("FfiConverterInt8.lower({value_expr})")),
        Type::UInt16 => Ok(format!("FfiConverterUInt16.lower({value_expr})")),
        Type::Int16 => Ok(format!("FfiConverterInt16.lower({value_expr})")),
        Type::UInt32 => Ok(format!("FfiConverterUInt32.lower({value_expr})")),
        Type::Int32 => Ok(format!("FfiConverterInt32.lower({value_expr})")),
        Type::UInt64 => Ok(format!("FfiConverterUInt64.lower({value_expr})")),
        Type::Int64 => Ok(format!("FfiConverterInt64.lower({value_expr})")),
        Type::Float32 => Ok(format!("FfiConverterFloat32.lower({value_expr})")),
        Type::Float64 => Ok(format!("FfiConverterFloat64.lower({value_expr})")),
        Type::Boolean => Ok(format!("FfiConverterBool.lower({value_expr})")),
        Type::String
        | Type::Bytes
        | Type::Record { .. }
        | Type::Enum { .. }
        | Type::Optional { .. }
        | Type::Sequence { .. }
        | Type::Map { .. }
        | Type::Timestamp
        | Type::Duration => Ok(format!(
            "uniffiLowerIntoRustBuffer({}, {value_expr})",
            render_js_type_converter_expression(type_)?
        )),
        Type::Object { name, .. } => Ok(format!(
            "{}.cloneHandle({value_expr})",
            object_factory_name(name)
        )),
        Type::CallbackInterface { .. } => Ok(format!(
            "{}.lower({value_expr})",
            render_js_type_converter_expression(type_)?
        )),
        Type::Custom { name, .. } => bail!("custom type '{name}' is not supported"),
    }
}

fn render_js_lift_expression(type_: &Type, value_expr: &str) -> Result<String> {
    match type_ {
        Type::UInt8 => Ok(format!("FfiConverterUInt8.lift({value_expr})")),
        Type::Int8 => Ok(format!("FfiConverterInt8.lift({value_expr})")),
        Type::UInt16 => Ok(format!("FfiConverterUInt16.lift({value_expr})")),
        Type::Int16 => Ok(format!("FfiConverterInt16.lift({value_expr})")),
        Type::UInt32 => Ok(format!("FfiConverterUInt32.lift({value_expr})")),
        Type::Int32 => Ok(format!("FfiConverterInt32.lift({value_expr})")),
        Type::UInt64 => Ok(format!("FfiConverterUInt64.lift({value_expr})")),
        Type::Int64 => Ok(format!("FfiConverterInt64.lift({value_expr})")),
        Type::Float32 => Ok(format!("FfiConverterFloat32.lift({value_expr})")),
        Type::Float64 => Ok(format!("FfiConverterFloat64.lift({value_expr})")),
        Type::Boolean => Ok(format!("FfiConverterBool.lift({value_expr})")),
        Type::String
        | Type::Bytes
        | Type::Record { .. }
        | Type::Enum { .. }
        | Type::Optional { .. }
        | Type::Sequence { .. }
        | Type::Map { .. }
        | Type::Timestamp
        | Type::Duration => Ok(format!(
            "uniffiLiftFromRustBuffer({}, {value_expr})",
            render_js_type_converter_expression(type_)?
        )),
        Type::Object { name, .. } => Ok(format!(
            "{}.create({value_expr})",
            object_factory_name(name)
        )),
        Type::CallbackInterface { .. } => Ok(format!(
            "{}.lift({value_expr})",
            render_js_type_converter_expression(type_)?
        )),
        Type::Custom { name, .. } => bail!("custom type '{name}' is not supported"),
    }
}

fn render_js_async_lift_closure(return_type: Option<&Type>) -> Result<String> {
    match return_type {
        Some(return_type) => Ok(format!(
            "(uniffiResult) => {}",
            render_js_lift_expression(return_type, "uniffiResult")?
        )),
        None => Ok("(_uniffiResult) => undefined".to_string()),
    }
}

fn render_js_type_converter_expression(type_: &Type) -> Result<String> {
    match type_ {
        Type::UInt8 => Ok("FfiConverterUInt8".to_string()),
        Type::Int8 => Ok("FfiConverterInt8".to_string()),
        Type::UInt16 => Ok("FfiConverterUInt16".to_string()),
        Type::Int16 => Ok("FfiConverterInt16".to_string()),
        Type::UInt32 => Ok("FfiConverterUInt32".to_string()),
        Type::Int32 => Ok("FfiConverterInt32".to_string()),
        Type::UInt64 => Ok("FfiConverterUInt64".to_string()),
        Type::Int64 => Ok("FfiConverterInt64".to_string()),
        Type::Float32 => Ok("FfiConverterFloat32".to_string()),
        Type::Float64 => Ok("FfiConverterFloat64".to_string()),
        Type::Boolean => Ok("FfiConverterBool".to_string()),
        Type::String => Ok("FfiConverterString".to_string()),
        Type::Bytes => Ok("FfiConverterBytes".to_string()),
        Type::Optional { inner_type } => Ok(format!(
            "new FfiConverterOptional({})",
            render_js_type_converter_expression(inner_type)?
        )),
        Type::Sequence { inner_type } => Ok(format!(
            "new FfiConverterArray({})",
            render_js_type_converter_expression(inner_type)?
        )),
        Type::Map {
            key_type,
            value_type,
        } => Ok(format!(
            "new FfiConverterMap({}, {})",
            render_js_type_converter_expression(key_type)?,
            render_js_type_converter_expression(value_type)?
        )),
        Type::Object { name, .. }
        | Type::Record { name, .. }
        | Type::Enum { name, .. }
        | Type::CallbackInterface { name, .. } => Ok(type_converter_name(name)),
        Type::Timestamp => Ok("FfiConverterTimestamp".to_string()),
        Type::Duration => Ok("FfiConverterDuration".to_string()),
        Type::Custom { name, .. } => bail!("custom type '{name}' is not supported"),
    }
}

fn render_js_callback_runtime_hooks(
    callback_interfaces: &[CallbackInterfaceModel],
) -> Result<String> {
    let mut lines = vec![
        "const uniffiRegisteredCallbackPointers = [];".to_string(),
        String::new(),
        "function uniffiRegisterCallbackVtables(bindings) {".to_string(),
    ];

    for callback_interface in callback_interfaces {
        lines.push(format!(
            "  {}(bindings, uniffiRegisteredCallbackPointers);",
            callback_interface_register_name(&callback_interface.name)
        ));
    }
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push("function uniffiUnregisterCallbackVtables() {".to_string());
    for callback_interface in callback_interfaces {
        lines.push(format!(
            "  {}.clear();",
            callback_interface_registry_name(&callback_interface.name)
        ));
    }
    lines.push("  while (uniffiRegisteredCallbackPointers.length > 0) {".to_string());
    lines.push("    koffi.unregister(uniffiRegisteredCallbackPointers.pop());".to_string());
    lines.push("  }".to_string());
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push("configureRuntimeHooks({".to_string());
    lines.push("  onLoad(bindings) {".to_string());
    lines.push("    uniffiRegisterCallbackVtables(bindings);".to_string());
    lines.push("  },".to_string());
    lines.push("  onUnload() {".to_string());
    lines.push("    uniffiUnregisterCallbackVtables();".to_string());
    lines.push("  },".to_string());
    lines.push("});".to_string());

    Ok(lines.join("\n"))
}

fn render_js_koffi_type_expression(type_: &Type, ffi_bindings_expr: &str) -> Result<String> {
    match type_ {
        Type::UInt8 => Ok("\"uint8_t\"".to_string()),
        Type::Int8 => Ok("\"int8_t\"".to_string()),
        Type::UInt16 => Ok("\"uint16_t\"".to_string()),
        Type::Int16 => Ok("\"int16_t\"".to_string()),
        Type::UInt32 => Ok("\"uint32_t\"".to_string()),
        Type::Int32 => Ok("\"int32_t\"".to_string()),
        Type::UInt64 | Type::CallbackInterface { .. } => Ok("\"uint64_t\"".to_string()),
        Type::Int64 => Ok("\"int64_t\"".to_string()),
        Type::Float32 => Ok("\"float\"".to_string()),
        Type::Float64 => Ok("\"double\"".to_string()),
        Type::Boolean => Ok("\"int8_t\"".to_string()),
        Type::String
        | Type::Bytes
        | Type::Record { .. }
        | Type::Enum { .. }
        | Type::Optional { .. }
        | Type::Sequence { .. }
        | Type::Map { .. }
        | Type::Timestamp
        | Type::Duration => Ok(format!("{ffi_bindings_expr}.ffiTypes.RustBuffer")),
        Type::Object { name, .. } => Ok(format!(
            "{ffi_bindings_expr}.ffiTypes.{}",
            ffi_opaque_identifier(name)
        )),
        Type::Custom { name, .. } => bail!("custom type '{name}' is not supported"),
    }
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

fn render_js_rust_call_options_expression(throws_type: Option<&Type>) -> Result<String> {
    match throws_type {
        Some(throws_type) => Ok(format!(
            "uniffiRustCallOptions({})",
            render_js_type_converter_expression(throws_type)?
        )),
        None => Ok("uniffiRustCallOptions()".to_string()),
    }
}

fn render_js_record_allocation_size_expression(record: &RecordModel) -> Result<String> {
    if record.fields.is_empty() {
        return Ok("0".to_string());
    }

    let terms = record
        .fields
        .iter()
        .map(|field| {
            Ok(format!(
                "{}.allocationSize({})",
                render_js_type_converter_expression(&field.type_)?,
                render_js_property_access("recordValue", &field.name)?
            ))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(terms.join(" + "))
}

fn variant_type_name(type_name: &str, variant_name: &str) -> String {
    format!("{type_name}{}", variant_name.to_upper_camel_case())
}

fn callback_interface_proxy_class_name(type_name: &str) -> String {
    format!("Uniffi{}Proxy", type_name.to_upper_camel_case())
}

fn callback_interface_factory_name(type_name: &str) -> String {
    format!("uniffi{}Factory", type_name.to_upper_camel_case())
}

fn callback_interface_registry_name(type_name: &str) -> String {
    format!("uniffi{}Registry", type_name.to_upper_camel_case())
}

fn callback_interface_validator_name(type_name: &str) -> String {
    format!(
        "uniffiValidate{}Implementation",
        type_name.to_upper_camel_case()
    )
}

fn callback_interface_register_name(type_name: &str) -> String {
    format!("uniffiRegister{}Vtable", type_name.to_upper_camel_case())
}

fn object_factory_name(type_name: &str) -> String {
    format!("uniffi{}ObjectFactory", type_name.to_upper_camel_case())
}

fn object_converter_name(type_name: &str) -> String {
    type_converter_name(type_name)
}

fn type_converter_name(type_name: &str) -> String {
    format!("FfiConverter{}", sanitize_identifier(type_name, false))
}

fn lowered_argument_name(argument_name: &str) -> String {
    format!("lowered{}", argument_name.to_upper_camel_case())
}

fn quoted_property_name(name: &str) -> Result<String> {
    json_string_literal(name)
}

fn render_js_property_access(value_expr: &str, property_name: &str) -> Result<String> {
    Ok(format!(
        "{}[{}]",
        value_expr,
        json_string_literal(property_name)?
    ))
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

fn ffi_clone_symbol_name(namespace: &str, object_name: &str) -> String {
    let namespace = namespace.replace("::", "__");
    let object_name = object_name.to_ascii_lowercase();
    format!("uniffi_{namespace}_fn_clone_{object_name}")
}

fn ffi_free_symbol_name(namespace: &str, object_name: &str) -> String {
    let namespace = namespace.replace("::", "__");
    let object_name = object_name.to_ascii_lowercase();
    format!("uniffi_{namespace}_fn_free_{object_name}")
}

fn callback_method_ffi_name(callback_interface_name: &str, index: usize) -> String {
    format!("CallbackInterface{callback_interface_name}Method{index}")
}

fn ffi_opaque_identifier(name: &str) -> String {
    ffi_symbol_identifier(&format!("RustArcPtr{name}"))
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

fn render_js_sync_function_body(function: &FunctionModel) -> Result<Vec<String>> {
    let mut lines = render_js_argument_lowering(&function.arguments)?;
    let call_args = render_js_ffi_call_args(&function.arguments, Some("status"));

    if let Some(return_type) = function.return_type.as_ref() {
        lines.push("  const uniffiResult = defaultRustCaller.rustCall(".to_string());
        lines.push(format!(
            "    (status) => ffiFunctions.{}({}),",
            function.ffi_func_identifier, call_args
        ));
        lines.push(format!(
            "    {},",
            render_js_rust_call_options_expression(function.throws_type.as_ref())?
        ));
        lines.push("  );".to_string());
        lines.push(format!(
            "  return {};",
            render_js_lift_expression(return_type, "uniffiResult")?
        ));
    } else {
        lines.push("  defaultRustCaller.rustCall(".to_string());
        lines.push(format!(
            "    (status) => ffiFunctions.{}({}),",
            function.ffi_func_identifier, call_args
        ));
        lines.push(format!(
            "    {},",
            render_js_rust_call_options_expression(function.throws_type.as_ref())?
        ));
        lines.push("  );".to_string());
    }

    Ok(lines)
}

fn render_js_async_function_body(function: &FunctionModel) -> Result<Vec<String>> {
    let async_ffi = function.async_ffi.as_ref().with_context(|| {
        format!(
            "async function {} is missing future scaffolding identifiers",
            function.name
        )
    })?;
    let mut lines = render_js_argument_lowering(&function.arguments)?;
    let start_args = render_js_ffi_call_args(&function.arguments, None);

    lines.push("  return rustCallAsync({".to_string());
    lines.push(format!(
        "    rustFutureFunc: () => ffiFunctions.{}({}),",
        function.ffi_func_identifier, start_args
    ));
    lines.push(format!(
        "    pollFunc: (rustFuture, continuationCallback, continuationHandle) => ffiFunctions.{}(rustFuture, continuationCallback, continuationHandle),",
        async_ffi.poll_identifier
    ));
    lines.push(format!(
        "    cancelFunc: (rustFuture) => ffiFunctions.{}(rustFuture),",
        async_ffi.cancel_identifier
    ));
    lines.push(format!(
        "    completeFunc: (rustFuture, status) => ffiFunctions.{}(rustFuture, status),",
        async_ffi.complete_identifier
    ));
    lines.push(format!(
        "    freeFunc: (rustFuture) => ffiFunctions.{}(rustFuture),",
        async_ffi.free_identifier
    ));
    lines.push(format!(
        "    liftFunc: {},",
        render_js_async_lift_closure(function.return_type.as_ref())?
    ));
    lines.push(format!(
        "    ...{},",
        render_js_rust_call_options_expression(function.throws_type.as_ref())?
    ));
    lines.push("  });".to_string());

    Ok(lines)
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
    fn component_model_accepts_async_callback_interfaces() {
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

        let model = ComponentModel::from_ci(&ci).expect("async callback interfaces should build");

        assert_eq!(model.callback_interfaces.len(), 1);
        assert!(model.callback_interfaces[0].methods[0].is_async);
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
        assert_eq!(
            render_public_type(&Type::UInt64).unwrap(),
            "bigint | number"
        );
        assert_eq!(render_public_type(&Type::Int64).unwrap(), "bigint | number");
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
            "Array<bigint | number | undefined>"
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
            "Map<string, bigint | number | undefined>"
        );
    }

    #[test]
    fn render_public_type_allows_number_inputs_for_i64_map_values() {
        assert_eq!(
            render_public_type(&Type::Map {
                key_type: Box::new(Type::String),
                value_type: Box::new(Type::Int64),
            })
            .unwrap(),
            "Map<string, bigint | number>"
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
            rendered
                .js
                .contains("const pointer = defaultRustCaller.rustCall("),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("return uniffiStoreObjectFactory.attach(this, pointer);"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("const loweredSelf = uniffiStoreObjectFactory.cloneHandle(this);"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "const loweredProfile = uniffiLowerIntoRustBuffer(FfiConverterProfile, profile);"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "liftFunc: (uniffiResult) => uniffiLiftFromRustBuffer(FfiConverterProfile, uniffiResult),"
            ),
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
        assert!(
            rendered.js.contains(
                "const FfiConverterProfile = new (class extends AbstractFfiConverterByteArray {"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("FfiConverterString.write(recordValue[\"name\"], writer);"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("FfiConverterBytes.write(recordValue[\"bytes\"], writer);"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            !rendered.js.contains(
                "const FfiConverterProfile = uniffiNotImplementedConverter(\"Profile\");"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "const FfiConverterFlavor = new (class extends AbstractFfiConverterByteArray {"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("writer.writeInt32(1);"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("return Flavor[\"vanilla\"];"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            !rendered
                .js
                .contains("const FfiConverterFlavor = uniffiNotImplementedConverter(\"Flavor\");"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "const FfiConverterOutcome = new (class extends AbstractFfiConverterByteArray {"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("const enumValue = uniffiRequireTaggedEnumValue(\"Outcome\", value);"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("return Outcome.Success(FfiConverterString.read(reader));"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            !rendered.js.contains(
                "const FfiConverterOutcome = uniffiNotImplementedConverter(\"Outcome\");"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "const FfiConverterStoreError = new (class extends AbstractFfiConverterByteArray {"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("return new StoreErrorConflict(FfiConverterString.read(reader));"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            !rendered.js.contains(
                "const FfiConverterStoreError = uniffiNotImplementedConverter(\"StoreError\");"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
    }

    #[test]
    fn render_public_api_wires_typed_error_lifting_for_methods_and_constructors() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {};

            [Error]
            interface StoreError {
                NotFound();
                Conflict(string message);
            };

            interface Store {
                [Throws=StoreError]
                constructor();
                [Throws=StoreError]
                void put(string key);
                [Async, Throws=StoreError]
                string get(string key);
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
                .js
                .contains("uniffiRustCallOptions(FfiConverterStoreError)"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            !rendered
                .js
                .contains("throws a typed error, but error lifting is still pending"),
            "unexpected JS output: {}",
            rendered.js
        );
    }

    #[test]
    fn render_public_api_uses_global_error_base_for_errors_named_error() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {
                [Throws=Error]
                void fail();
            };

            [Error]
            interface Error {
                Invalid(string message);
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
                .js
                .contains("export class Error extends globalThis.Error {"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .dts
                .contains("export declare class Error extends globalThis.Error {"),
            "unexpected DTS output: {}",
            rendered.dts
        );
        assert!(
            rendered.js.contains("export class ErrorInvalid extends Error {"),
            "unexpected JS output: {}",
            rendered.js
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
            rendered.js.contains("return rustCallAsync({"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("liftFunc: (pointer) => uniffiAsyncStoreObjectFactory.create(pointer),"),
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
    fn render_public_api_emits_sync_method_return_lifting() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {};

            interface Store {
                constructor();
                string name();
                Store clone();
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
                .js
                .contains("return uniffiLiftFromRustBuffer(FfiConverterString, uniffiResult);"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("return uniffiStoreObjectFactory.create(uniffiResult);"),
            "unexpected JS output: {}",
            rendered.js
        );
    }

    #[test]
    fn render_public_api_emits_async_method_object_lifting() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {};

            interface Store {
                constructor();
                [Async] Store clone_async();
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
            rendered.js.contains(
                "pollFunc: (rustFuture, continuationCallback, continuationHandle) => ffiFunctions."
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "liftFunc: (uniffiResult) => uniffiStoreObjectFactory.create(uniffiResult),"
            ),
            "unexpected JS output: {}",
            rendered.js
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
        assert!(
            rendered
                .js
                .contains("const uniffiLogCallbackRegistry = createCallbackRegistry({"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("function uniffiRegisterLogCallbackVtable(bindings, registrations) {"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("configureRuntimeHooks({"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("defaultRustCaller.rustCall("),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            !rendered.js.contains("init_logging is not implemented yet"),
            "unexpected JS output: {}",
            rendered.js
        );
    }

    #[test]
    fn render_public_api_emits_nested_byte_sequence_converters() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {
                sequence<bytes> round_trip_chunks(sequence<bytes> chunks);
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
            rendered.js.contains(
                "const loweredChunks = uniffiLowerIntoRustBuffer(new FfiConverterArray(FfiConverterBytes), chunks);"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "return uniffiLiftFromRustBuffer(new FfiConverterArray(FfiConverterBytes), uniffiResult);"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .dts
                .contains("export declare function round_trip_chunks(chunks: Array<Uint8Array>): Array<Uint8Array>;"),
            "unexpected DTS output: {}",
            rendered.dts
        );
    }

    #[test]
    fn render_public_api_emits_optional_byte_array_converters() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {
                bytes? round_trip_bytes(bytes? value);
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
            rendered.js.contains(
                "const loweredValue = uniffiLowerIntoRustBuffer(new FfiConverterOptional(FfiConverterBytes), value);"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "return uniffiLiftFromRustBuffer(new FfiConverterOptional(FfiConverterBytes), uniffiResult);"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .dts
                .contains("export declare function round_trip_bytes(value: Uint8Array | undefined): Uint8Array | undefined;"),
            "unexpected DTS output: {}",
            rendered.dts
        );
    }

    #[test]
    fn render_public_api_emits_optional_object_converters() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {};

            interface Store {
                constructor();
                Store? maybe_clone(Store? value);
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
            rendered.js.contains(
                "const loweredValue = uniffiLowerIntoRustBuffer(new FfiConverterOptional(FfiConverterStore), value);"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "return uniffiLiftFromRustBuffer(new FfiConverterOptional(FfiConverterStore), uniffiResult);"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .dts
                .contains("maybe_clone(value: Store | undefined): Store | undefined;"),
            "unexpected DTS output: {}",
            rendered.dts
        );
    }
}
