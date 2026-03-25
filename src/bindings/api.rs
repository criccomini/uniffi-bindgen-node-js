use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};
use uniffi_bindgen::interface::{
    AsType, ComponentInterface, Constructor, Enum, Field, Function, Method, Object, Type, Variant,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComponentModel {
    pub functions: Vec<FunctionModel>,
    pub records: Vec<RecordModel>,
    pub flat_enums: Vec<EnumModel>,
    pub tagged_enums: Vec<EnumModel>,
    pub errors: Vec<ErrorModel>,
    pub objects: Vec<ObjectModel>,
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
            objects: ci
                .object_definitions()
                .iter()
                .map(ObjectModel::from_object)
                .collect(),
        };
        model.validate_renderable_types()?;
        Ok(model)
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
        Type::Object { name, .. } | Type::Record { name, .. } | Type::Enum { name, .. } => {
            Ok(name.clone())
        }
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
        Type::CallbackInterface { name, .. } => {
            bail!("callback interface '{name}' is not supported in the public Node API yet")
        }
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
pub(crate) struct ObjectModel {
    pub name: String,
    pub constructors: Vec<ConstructorModel>,
    pub methods: Vec<MethodModel>,
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

    let callback_interfaces = ci
        .callback_interface_definitions()
        .iter()
        .map(|callback| callback.name().to_string())
        .collect::<BTreeSet<_>>();
    if !callback_interfaces.is_empty() {
        unsupported.push(format!(
            "callback interfaces are not supported yet: {}",
            callback_interfaces
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ")
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

        assert_eq!(model.functions.len(), 1);
        assert_eq!(model.functions[0].name, "greet");
        assert_eq!(model.records.len(), 1);
        assert_eq!(model.records[0].name, "Profile");
        assert_eq!(model.flat_enums.len(), 1);
        assert_eq!(model.flat_enums[0].name, "Flavor");
        assert_eq!(model.tagged_enums.len(), 1);
        assert_eq!(model.tagged_enums[0].name, "Outcome");
        assert_eq!(model.errors.len(), 1);
        assert_eq!(model.errors[0].name, "StoreError");
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
    fn component_model_rejects_callback_interfaces() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {};

            callback interface Logger {
                void write(string message);
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");

        let error = ComponentModel::from_ci(&ci).expect_err("callback interfaces should fail");

        assert!(
            error
                .to_string()
                .contains("callback interfaces are not supported yet"),
            "unexpected error: {error}"
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
}
