use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};
use uniffi_bindgen::interface::{
    AsType, Callable, CallbackInterface, ComponentInterface, Constructor, Enum, Field, Function,
    Method, Object, Type, Variant,
    ffi::{FfiArgument, FfiCallbackFunction, FfiDefinition, FfiStruct, FfiType},
};

use super::{
    ffi_symbol_identifier, render_js_default_async_callback_return_value_expression,
    validate_supported_features,
};

pub(crate) fn build_public_api_ir(ci: &ComponentInterface) -> Result<ComponentModel> {
    ComponentModel::from_ci(ci)
}

/// Normalized public-API IR derived from a `ComponentInterface`.
///
/// Template-specific decisions live in the renderer layer so this stays a
/// reusable conversion boundary between UniFFI metadata loading and codegen.
///
/// The input `ComponentInterface` is expected to already have passed through
/// `derive_ffi_funcs()`, matching the loader-based pipeline. UniFFI 0.31 only
/// materializes some callback-trait object FFI metadata, such as
/// `ffi_init_callback()`, during that step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComponentModel {
    pub namespace_docstring: Option<String>,
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

impl ComponentModel {
    pub(crate) fn from_ci(ci: &ComponentInterface) -> Result<Self> {
        validate_supported_features(ci)?;

        let (flat_enums, tagged_enums, errors) = classify_enum_models(ci);
        let callback_interfaces = collect_callback_interfaces(ci)?;
        let callback_interface_names = callback_interface_names(&callback_interfaces);

        let model = Self {
            namespace_docstring: ci.namespace_docstring().map(str::to_owned),
            functions: ci
                .function_definitions()
                .iter()
                .map(|function| FunctionModel::from_function(function, ci))
                .collect(),
            records: ci
                .record_definitions()
                .iter()
                .map(RecordModel::from_record)
                .collect(),
            flat_enums,
            tagged_enums,
            errors,
            callback_interfaces,
            objects: collect_object_models(ci, &callback_interface_names),
            ffi_rustbuffer_from_bytes_identifier: ffi_symbol_identifier(
                ci.ffi_rustbuffer_from_bytes().name(),
            ),
            ffi_rustbuffer_free_identifier: ffi_symbol_identifier(ci.ffi_rustbuffer_free().name()),
        };
        model.validate_renderable_types()?;
        Ok(model)
    }
}

fn classify_enum_models(
    ci: &ComponentInterface,
) -> (Vec<EnumModel>, Vec<EnumModel>, Vec<ErrorModel>) {
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

    (flat_enums, tagged_enums, errors)
}

fn collect_callback_interfaces(ci: &ComponentInterface) -> Result<Vec<CallbackInterfaceModel>> {
    ci.callback_interface_definitions()
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
        .collect()
}

fn callback_interface_names(callback_interfaces: &[CallbackInterfaceModel]) -> BTreeSet<String> {
    callback_interfaces
        .iter()
        .map(|callback_interface| callback_interface.name.clone())
        .collect()
}

fn collect_object_models(
    ci: &ComponentInterface,
    callback_interface_names: &BTreeSet<String>,
) -> Vec<ObjectModel> {
    ci.object_definitions()
        .iter()
        .filter(|object| {
            !callback_interface_names.contains(object.name()) && !object.has_callback_interface()
        })
        .map(|object| ObjectModel::from_object(object, ci))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FunctionModel {
    pub name: String,
    pub docstring: Option<String>,
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
            docstring: function.docstring().map(str::to_owned),
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
    pub docstring: Option<String>,
    pub fields: Vec<FieldModel>,
}

impl RecordModel {
    fn from_record(record: &uniffi_bindgen::interface::Record) -> Self {
        Self {
            name: record.name().to_string(),
            docstring: record.docstring().map(str::to_owned),
            fields: record.fields().iter().map(FieldModel::from_field).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EnumModel {
    pub name: String,
    pub docstring: Option<String>,
    pub variants: Vec<VariantModel>,
}

impl EnumModel {
    fn from_enum(enum_def: &Enum) -> Self {
        Self {
            name: enum_def.name().to_string(),
            docstring: enum_def.docstring().map(str::to_owned),
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
    pub docstring: Option<String>,
    pub is_flat: bool,
    pub variants: Vec<VariantModel>,
}

impl ErrorModel {
    fn from_enum(enum_def: &Enum) -> Self {
        Self {
            name: enum_def.name().to_string(),
            docstring: enum_def.docstring().map(str::to_owned),
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
    pub docstring: Option<String>,
    pub methods: Vec<MethodModel>,
    pub ffi_init_callback_identifier: String,
    pub ffi_object_clone_identifier: String,
    pub ffi_object_free_identifier: String,
}

impl CallbackInterfaceModel {
    fn from_callback_interface(
        callback_interface: &CallbackInterface,
        ci: &ComponentInterface,
    ) -> Result<Self> {
        let vtable_methods = callback_interface.vtable_methods();
        validate_callback_vtable_definition(
            &callback_interface.vtable_definition(),
            &vtable_methods,
            callback_interface.name(),
        )?;

        Ok(Self {
            name: callback_interface.name().to_string(),
            docstring: callback_interface.docstring().map(str::to_owned),
            methods: callback_method_models(vtable_methods, ci)?,
            ffi_init_callback_identifier: ffi_symbol_identifier(
                callback_interface.ffi_init_callback().name(),
            ),
            ffi_object_clone_identifier: ffi_symbol_identifier(&uniffi_meta::clone_fn_symbol_name(
                callback_interface.module_path(),
                callback_interface.name(),
            )),
            ffi_object_free_identifier: ffi_symbol_identifier(&uniffi_meta::free_fn_symbol_name(
                callback_interface.module_path(),
                callback_interface.name(),
            )),
        })
    }

    fn from_object(object: &Object, ci: &ComponentInterface) -> Result<Self> {
        let vtable_methods = object.vtable_methods();
        let vtable_definition = object.vtable_definition().with_context(|| {
            format!(
                "callback trait object {} is missing its UniFFI vtable definition",
                object.name()
            )
        })?;
        validate_callback_vtable_definition(&vtable_definition, &vtable_methods, object.name())?;

        Ok(Self {
            name: object.name().to_string(),
            docstring: object.docstring().map(str::to_owned),
            methods: callback_method_models(vtable_methods, ci)?,
            ffi_init_callback_identifier: ffi_symbol_identifier(object.ffi_init_callback().name()),
            ffi_object_clone_identifier: ffi_symbol_identifier(object.ffi_object_clone().name()),
            ffi_object_free_identifier: ffi_symbol_identifier(object.ffi_object_free().name()),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObjectModel {
    pub name: String,
    pub docstring: Option<String>,
    pub constructors: Vec<ConstructorModel>,
    pub methods: Vec<MethodModel>,
    pub ffi_object_clone_identifier: String,
    pub ffi_object_free_identifier: String,
}

impl ObjectModel {
    fn from_object(object: &Object, ci: &ComponentInterface) -> Self {
        Self {
            name: object.name().to_string(),
            docstring: object.docstring().map(str::to_owned),
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
    pub docstring: Option<String>,
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
            docstring: constructor.docstring().map(str::to_owned),
            is_primary: constructor.is_primary_constructor(),
            is_async: constructor.is_async(),
            // UniFFI 0.31 constructors expose their actual FFI call shape through
            // `full_arguments()`; there is no implicit receiver slot to strip.
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
    pub docstring: Option<String>,
    pub is_async: bool,
    pub arguments: Vec<ArgumentModel>,
    pub return_type: Option<Type>,
    pub throws_type: Option<Type>,
    pub ffi_func_identifier: String,
    pub ffi_callback_identifier: Option<String>,
    pub async_callback_ffi: Option<AsyncCallbackMethodModel>,
    pub async_ffi: Option<AsyncScaffoldingModel>,
}

impl MethodModel {
    fn from_method(method: &Method, ci: &ComponentInterface) -> Self {
        Self {
            name: method.name().to_string(),
            docstring: method.docstring().map(str::to_owned),
            is_async: method.is_async(),
            // UniFFI 0.31 `Method::full_arguments()` includes the implicit receiver
            // handle. The Node runtime supplies that from `this`, so the public IR
            // must keep only the explicit user-facing arguments here.
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
            async_callback_ffi: None,
            async_ffi: AsyncScaffoldingModel::from_callable(method, ci),
        }
    }

    fn from_callback_method(
        method: &Method,
        ci: &ComponentInterface,
        ffi_callback: &FfiCallbackFunction,
    ) -> Result<Self> {
        let mut model = Self::from_method(method, ci);
        model.ffi_callback_identifier = Some(ffi_symbol_identifier(ffi_callback.name()));
        model.async_callback_ffi = AsyncCallbackMethodModel::from_method(method, ffi_callback, ci)?;
        Ok(model)
    }
}

fn callback_method_models(
    vtable_methods: Vec<(FfiCallbackFunction, Method)>,
    ci: &ComponentInterface,
) -> Result<Vec<MethodModel>> {
    vtable_methods
        .into_iter()
        .map(|(ffi_callback, method)| MethodModel::from_callback_method(&method, ci, &ffi_callback))
        .collect()
}

fn validate_callback_vtable_definition(
    vtable_definition: &FfiStruct,
    vtable_methods: &[(FfiCallbackFunction, Method)],
    interface_name: &str,
) -> Result<()> {
    let actual_fields = vtable_definition
        .fields()
        .iter()
        .map(|field| field.name().to_string())
        .collect::<Vec<_>>();
    let expected_fields = std::iter::once("uniffi_free".to_string())
        .chain(std::iter::once("uniffi_clone".to_string()))
        .chain(
            vtable_methods
                .iter()
                .map(|(_, method)| method.name().to_string()),
        )
        .collect::<Vec<_>>();

    if actual_fields != expected_fields {
        bail!(
            "callback interface {interface_name} vtable field order changed: expected {:?}, found {:?}",
            expected_fields,
            actual_fields
        );
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsyncCallbackMethodModel {
    pub complete_identifier: String,
    pub result_struct_identifier: String,
    pub result_struct_has_return_value: bool,
    pub dropped_callback_struct_identifier: String,
    pub dropped_callback_identifier: String,
    pub default_error_return_value_expression: Option<String>,
}

impl AsyncCallbackMethodModel {
    fn from_method(
        method: &Method,
        ffi_callback: &FfiCallbackFunction,
        ci: &ComponentInterface,
    ) -> Result<Option<Self>> {
        if !method.is_async() {
            return Ok(None);
        }

        let abi = resolve_async_callback_abi(method, ffi_callback, ci)?;
        Ok(Some(Self {
            complete_identifier: ffi_symbol_identifier(&abi.complete_callback_name),
            result_struct_identifier: ffi_symbol_identifier(&abi.result_struct_name),
            result_struct_has_return_value: abi.result_struct_has_return_value,
            dropped_callback_struct_identifier: ffi_symbol_identifier(
                &abi.dropped_callback_struct_name,
            ),
            dropped_callback_identifier: ffi_symbol_identifier(&abi.dropped_callback_name),
            default_error_return_value_expression: method
                .return_type()
                .map(render_js_default_async_callback_return_value_expression),
        }))
    }
}

struct AsyncCallbackAbi {
    complete_callback_name: String,
    result_struct_name: String,
    result_struct_has_return_value: bool,
    dropped_callback_struct_name: String,
    dropped_callback_name: String,
}

fn resolve_async_callback_abi(
    method: &Method,
    ffi_callback: &FfiCallbackFunction,
    ci: &ComponentInterface,
) -> Result<AsyncCallbackAbi> {
    let [
        complete_callback_arg,
        callback_data_arg,
        dropped_callback_arg,
    ] = async_callback_trailing_arguments(method.name(), ffi_callback)?;
    let complete_callback_name =
        callback_name_from_argument(method.name(), complete_callback_arg, "completion callback")?;
    expect_uint64_argument(method.name(), callback_data_arg, "callback data")?;
    let dropped_callback_struct_name = mut_referenced_struct_name(
        method.name(),
        dropped_callback_arg,
        "dropped-callback pointer",
        "dropped-callback struct",
    )?;

    let result_struct_name =
        lookup_completion_result_struct(ci, &complete_callback_name, method.name())?;
    let result_struct = lookup_ffi_struct(ci, &result_struct_name, method.name())?;
    let dropped_callback_struct =
        lookup_ffi_struct(ci, &dropped_callback_struct_name, method.name())?;
    let dropped_callback_name =
        lookup_dropped_callback_name(&dropped_callback_struct, method.name())?;

    Ok(AsyncCallbackAbi {
        complete_callback_name,
        result_struct_name,
        result_struct_has_return_value: result_struct_has_return_value(&result_struct),
        dropped_callback_struct_name,
        dropped_callback_name,
    })
}

fn async_callback_trailing_arguments<'a>(
    method_name: &str,
    ffi_callback: &'a FfiCallbackFunction,
) -> Result<[&'a FfiArgument; 3]> {
    let arguments = ffi_callback.arguments();
    let trailing = arguments
        .get(arguments.len().saturating_sub(3)..)
        .filter(|trailing| trailing.len() == 3)
        .with_context(|| {
            format!(
                "async callback interface method {method_name} is missing the expected ForeignFuture ABI arguments"
            )
        })?;

    Ok([trailing[0], trailing[1], trailing[2]])
}

fn callback_name_from_argument(
    method_name: &str,
    argument: &FfiArgument,
    argument_label: &str,
) -> Result<String> {
    match argument.type_() {
        FfiType::Callback(name) => Ok(name),
        other => bail!(
            "async callback interface method {method_name} uses an unexpected {argument_label} type: {other:?}"
        ),
    }
}

fn expect_uint64_argument(
    method_name: &str,
    argument: &FfiArgument,
    argument_label: &str,
) -> Result<()> {
    match argument.type_() {
        FfiType::UInt64 => Ok(()),
        other => bail!(
            "async callback interface method {method_name} uses an unexpected {argument_label} type: {other:?}"
        ),
    }
}

fn mut_referenced_struct_name(
    method_name: &str,
    argument: &FfiArgument,
    pointer_label: &str,
    struct_label: &str,
) -> Result<String> {
    match argument.type_() {
        FfiType::MutReference(inner) => match inner.as_ref() {
            FfiType::Struct(name) => Ok(name.clone()),
            other => bail!(
                "async callback interface method {method_name} uses an unexpected {struct_label} type: {other:?}"
            ),
        },
        other => bail!(
            "async callback interface method {method_name} uses an unexpected {pointer_label} type: {other:?}"
        ),
    }
}

fn result_struct_has_return_value(struct_: &FfiStruct) -> bool {
    struct_
        .fields()
        .iter()
        .any(|field| field.name() == "return_value")
}

fn lookup_completion_result_struct(
    ci: &ComponentInterface,
    complete_callback_name: &str,
    method_name: &str,
) -> Result<String> {
    let callback = lookup_ffi_callback(ci, complete_callback_name, method_name)?;
    let callback_arguments = callback.arguments();
    let result_argument = callback_arguments.get(1).with_context(|| {
        format!(
            "async callback interface method {method_name} completion callback {complete_callback_name} is missing its result argument"
        )
    })?;

    match result_argument.type_() {
        FfiType::Struct(name) => Ok(name),
        other => bail!(
            "async callback interface method {method_name} completion callback {complete_callback_name} uses an unexpected result argument type: {other:?}"
        ),
    }
}

fn lookup_ffi_callback(
    ci: &ComponentInterface,
    callback_name: &str,
    method_name: &str,
) -> Result<FfiCallbackFunction> {
    ci.ffi_definitions()
        .find_map(|definition| match definition {
            FfiDefinition::CallbackFunction(callback) if callback.name() == callback_name => {
                Some(callback)
            }
            _ => None,
        })
        .with_context(|| {
            format!(
                "async callback interface method {method_name} could not resolve completion callback {callback_name}"
            )
        })
}

fn lookup_ffi_struct(
    ci: &ComponentInterface,
    struct_name: &str,
    method_name: &str,
) -> Result<FfiStruct> {
    ci.ffi_definitions()
        .find_map(|definition| match definition {
            FfiDefinition::Struct(struct_) if struct_.name() == struct_name => Some(struct_),
            _ => None,
        })
        .with_context(|| {
            format!(
                "async callback interface method {method_name} could not resolve FFI struct {struct_name}"
            )
        })
}

fn lookup_dropped_callback_name(struct_: &FfiStruct, method_name: &str) -> Result<String> {
    named_callback_field(struct_, "free")
        .or_else(|| first_callback_field(struct_))
        .with_context(|| {
            format!(
                "async callback interface method {method_name} could not resolve a dropped-callback function from FFI struct {}",
                struct_.name()
            )
        })
}

fn named_callback_field(struct_: &FfiStruct, field_name: &str) -> Option<String> {
    struct_
        .fields()
        .iter()
        .find_map(|field| match field.type_() {
            FfiType::Callback(name) if field.name() == field_name => Some(name),
            _ => None,
        })
}

fn first_callback_field(struct_: &FfiStruct) -> Option<String> {
    struct_
        .fields()
        .iter()
        .find_map(|field| match field.type_() {
            FfiType::Callback(name) => Some(name),
            _ => None,
        })
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
            // UniFFI 0.30/0.31 owns the rust_future_* helper naming contract,
            // including return-type suffixes, so always ask the callable/CI
            // for the exact exported symbols instead of reconstructing them.
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
    pub docstring: Option<String>,
    pub fields: Vec<FieldModel>,
}

impl VariantModel {
    fn from_variant(variant: &Variant) -> Self {
        Self {
            name: variant.name().to_string(),
            docstring: variant.docstring().map(str::to_owned),
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
    pub docstring: Option<String>,
    pub type_: Type,
}

impl FieldModel {
    fn from_field(field: &Field) -> Self {
        Self {
            name: field.name().to_string(),
            docstring: field.docstring().map(str::to_owned),
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
