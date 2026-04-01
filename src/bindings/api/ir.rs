use std::collections::BTreeSet;

use anyhow::Result;
use uniffi_bindgen::interface::{
    AsType, Callable, CallbackInterface, ComponentInterface, Constructor, Enum, Field, Function,
    Method, Object, Type, Variant, ffi::FfiType,
};

use super::{
    ffi_symbol_identifier, foreign_future_complete_ffi_name,
    render_js_default_async_callback_return_value_expression, validate_supported_features,
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
/// `derive_ffi_funcs()`, matching the v2 loader pipeline. UniFFI 0.31 only
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
    ) -> Self {
        Self {
            name: callback_interface.name().to_string(),
            docstring: callback_interface.docstring().map(str::to_owned),
            methods: callback_method_models(
                callback_interface.methods(),
                callback_interface
                    .ffi_callbacks()
                    .into_iter()
                    .map(|callback| callback.name().to_string())
                    .collect(),
                ci,
            ),
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
        }
    }

    fn from_object(object: &Object, ci: &ComponentInterface) -> Self {
        Self {
            name: object.name().to_string(),
            docstring: object.docstring().map(str::to_owned),
            methods: callback_method_models(
                object.methods(),
                object
                    .ffi_callbacks()
                    .into_iter()
                    .map(|callback| callback.name().to_string())
                    .collect(),
                ci,
            ),
            ffi_init_callback_identifier: ffi_symbol_identifier(object.ffi_init_callback().name()),
            ffi_object_clone_identifier: ffi_symbol_identifier(object.ffi_object_clone().name()),
            ffi_object_free_identifier: ffi_symbol_identifier(object.ffi_object_free().name()),
        }
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
        ffi_callback_name: &str,
    ) -> Self {
        let mut model = Self::from_method(method, ci);
        model.ffi_callback_identifier = Some(ffi_symbol_identifier(ffi_callback_name));
        model.async_callback_ffi = AsyncCallbackMethodModel::from_method(method);
        model
    }
}

fn callback_method_models(
    methods: Vec<&Method>,
    ffi_callback_names: Vec<String>,
    ci: &ComponentInterface,
) -> Vec<MethodModel> {
    assert_eq!(
        methods.len(),
        ffi_callback_names.len(),
        "UniFFI callback method metadata and callback symbol lists diverged"
    );

    methods
        .into_iter()
        .zip(ffi_callback_names)
        .map(|(method, ffi_callback_name)| {
            MethodModel::from_callback_method(method, ci, &ffi_callback_name)
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsyncCallbackMethodModel {
    pub complete_identifier: String,
    pub result_struct_identifier: String,
    pub result_struct_has_return_value: bool,
    pub default_error_return_value_expression: Option<String>,
}

impl AsyncCallbackMethodModel {
    fn from_method(method: &Method) -> Option<Self> {
        method.is_async().then(|| {
            let return_ffi_type = method.return_type().map(FfiType::from);
            let result_struct = method.foreign_future_ffi_result_struct();
            Self {
                complete_identifier: ffi_symbol_identifier(&foreign_future_complete_ffi_name(
                    return_ffi_type.as_ref(),
                )),
                result_struct_identifier: ffi_symbol_identifier(result_struct.name()),
                result_struct_has_return_value: result_struct
                    .fields()
                    .iter()
                    .any(|field| field.name() == "return_value"),
                default_error_return_value_expression: method
                    .return_type()
                    .map(render_js_default_async_callback_return_value_expression),
            }
        })
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
