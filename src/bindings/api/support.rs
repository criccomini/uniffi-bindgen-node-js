use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};
use heck::ToUpperCamelCase;
use uniffi_bindgen::interface::{ComponentInterface, Type, ffi::FfiType};

use super::{ArgumentModel, FieldModel, RecordModel};

pub(crate) fn validate_supported_features(ci: &ComponentInterface) -> Result<()> {
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

pub(crate) fn render_js_params(arguments: &[ArgumentModel]) -> String {
    arguments
        .iter()
        .map(|argument| js_identifier(&argument.name))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn render_js_argument_lowering(arguments: &[ArgumentModel]) -> Result<Vec<String>> {
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

pub(crate) fn render_js_ffi_call_args(
    arguments: &[ArgumentModel],
    trailing: Option<&str>,
) -> String {
    let mut args = arguments
        .iter()
        .map(|argument| lowered_argument_name(&argument.name))
        .collect::<Vec<_>>();
    if let Some(trailing) = trailing {
        args.push(trailing.to_string());
    }
    args.join(", ")
}

pub(crate) fn render_js_ffi_call_args_with_leading(
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

pub(crate) fn render_js_lower_expression(type_: &Type, value_expr: &str) -> Result<String> {
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
        Type::String => Ok(format!("uniffiLowerString({value_expr})")),
        Type::Bytes
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
        Type::Object { name, imp, .. } => {
            if imp.has_callback_interface() {
                Ok(format!("{}.lower({value_expr})", type_converter_name(name)))
            } else {
                Ok(format!(
                    "{}.cloneHandle({value_expr})",
                    object_factory_name(name)
                ))
            }
        }
        Type::CallbackInterface { .. } => Ok(format!(
            "{}.lower({value_expr})",
            render_js_type_converter_expression(type_)?
        )),
        Type::Custom { name, .. } => bail!("custom type '{name}' is not supported"),
    }
}

pub(crate) fn render_js_lift_expression(type_: &Type, value_expr: &str) -> Result<String> {
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
        Type::String => Ok(format!("uniffiLiftStringFromRustBuffer({value_expr})")),
        Type::Bytes
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
        Type::Object { name, imp, .. } => {
            if imp.has_callback_interface() {
                Ok(format!("{}.lift({value_expr})", type_converter_name(name)))
            } else {
                Ok(format!(
                    "{}.create({value_expr})",
                    object_factory_name(name)
                ))
            }
        }
        Type::CallbackInterface { .. } => Ok(format!(
            "{}.lift({value_expr})",
            render_js_type_converter_expression(type_)?
        )),
        Type::Custom { name, .. } => bail!("custom type '{name}' is not supported"),
    }
}

pub(crate) fn render_js_async_lift_closure(return_type: Option<&Type>) -> Result<String> {
    match return_type {
        Some(Type::Object { name, imp, .. }) if !imp.has_callback_interface() => Ok(format!(
            "(uniffiResult) => {}.createRawExternal(uniffiResult)",
            object_factory_name(name)
        )),
        Some(return_type) => Ok(format!(
            "(uniffiResult) => {}",
            render_js_lift_expression(return_type, "uniffiResult")?
        )),
        None => Ok("(_uniffiResult) => undefined".to_string()),
    }
}

pub(crate) fn render_js_async_complete_setup(
    return_type: Option<&Type>,
    complete_identifier: &str,
    indent: &str,
) -> Result<Vec<String>> {
    match return_type {
        Some(Type::Object { imp, .. }) if !imp.has_callback_interface() => {
            render_js_async_object_complete_setup(complete_identifier, indent)
        }
        _ => Ok(vec![format!(
            "{indent}const completeFunc = (rustFuture, status) => ffiFunctions.{complete_identifier}(rustFuture, status);"
        )]),
    }
}

pub(crate) fn render_js_async_object_complete_setup(
    complete_identifier: &str,
    indent: &str,
) -> Result<Vec<String>> {
    Ok(vec![
        format!("{indent}const bindings = getFfiBindings();"),
        format!("{indent}const completePointer = bindings.library.func("),
        format!("{indent}  {},", json_string_literal(complete_identifier)?),
        format!("{indent}  bindings.ffiTypes.VoidPointer,"),
        format!(
            "{indent}  [bindings.ffiTypes.UniffiHandle, koffi.pointer(bindings.ffiTypes.RustCallStatus)],"
        ),
        format!("{indent});"),
        format!(
            "{indent}const completeFunc = (rustFuture, status) => completePointer(rustFuture, status);"
        ),
    ])
}

pub(crate) fn render_js_type_converter_expression(type_: &Type) -> Result<String> {
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

pub(crate) fn render_js_koffi_type_expression(
    type_: &Type,
    ffi_bindings_expr: &str,
) -> Result<String> {
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

pub(crate) fn render_js_default_async_callback_return_value_expression(type_: &Type) -> String {
    match type_ {
        Type::UInt8
        | Type::Int8
        | Type::UInt16
        | Type::Int16
        | Type::UInt32
        | Type::Int32
        | Type::UInt64
        | Type::Int64
        | Type::Float32
        | Type::Float64
        | Type::Boolean => "0".to_string(),
        Type::String
        | Type::Bytes
        | Type::Record { .. }
        | Type::Enum { .. }
        | Type::Optional { .. }
        | Type::Sequence { .. }
        | Type::Map { .. }
        | Type::Timestamp
        | Type::Duration => "EMPTY_RUST_BUFFER".to_string(),
        Type::Object { .. } | Type::CallbackInterface { .. } => "0n".to_string(),
        Type::Custom { name, .. } => {
            unreachable!("custom type '{name}' should have been rejected before codegen")
        }
    }
}

pub(crate) fn render_dts_params(arguments: &[ArgumentModel]) -> Result<String> {
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

pub(crate) fn render_js_fields_as_params(fields: &[FieldModel]) -> String {
    fields
        .iter()
        .map(|field| js_identifier(&field.name))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn render_dts_fields_as_params(fields: &[FieldModel]) -> Result<String> {
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

pub(crate) fn render_return_type(return_type: Option<&Type>, is_async: bool) -> Result<String> {
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

pub(crate) fn render_named_return_type(type_name: &str, is_async: bool) -> String {
    if is_async {
        format!("Promise<{type_name}>")
    } else {
        type_name.to_string()
    }
}

pub(crate) fn render_js_rust_call_options_expression(throws_type: Option<&Type>) -> Result<String> {
    match throws_type {
        Some(throws_type) => Ok(format!(
            "uniffiRustCallOptions({})",
            render_js_type_converter_expression(throws_type)?
        )),
        None => Ok("uniffiRustCallOptions()".to_string()),
    }
}

pub(crate) fn render_js_record_allocation_size_expression(record: &RecordModel) -> Result<String> {
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

pub(crate) fn variant_type_name(type_name: &str, variant_name: &str) -> String {
    format!("{type_name}{}", variant_name.to_upper_camel_case())
}

pub(crate) fn callback_interface_proxy_class_name(type_name: &str) -> String {
    format!("Uniffi{}Proxy", type_name.to_upper_camel_case())
}

pub(crate) fn callback_interface_factory_name(type_name: &str) -> String {
    format!("uniffi{}Factory", type_name.to_upper_camel_case())
}

pub(crate) fn callback_interface_registry_name(type_name: &str) -> String {
    format!("uniffi{}Registry", type_name.to_upper_camel_case())
}

pub(crate) fn callback_interface_validator_name(type_name: &str) -> String {
    format!(
        "uniffiValidate{}Implementation",
        type_name.to_upper_camel_case()
    )
}

pub(crate) fn callback_interface_register_name(type_name: &str) -> String {
    format!("uniffiRegister{}Vtable", type_name.to_upper_camel_case())
}

pub(crate) fn callback_interface_vtable_struct_name(type_name: &str) -> String {
    format!("VTableCallbackInterface{}", type_name.to_upper_camel_case())
}

pub(crate) fn object_factory_name(type_name: &str) -> String {
    format!("uniffi{}ObjectFactory", type_name.to_upper_camel_case())
}

pub(crate) fn object_converter_name(type_name: &str) -> String {
    type_converter_name(type_name)
}

pub(crate) fn type_converter_name(type_name: &str) -> String {
    format!("FfiConverter{}", sanitize_identifier(type_name, false))
}

pub(crate) fn lowered_argument_name(argument_name: &str) -> String {
    format!("lowered{}", argument_name.to_upper_camel_case())
}

pub(crate) fn quoted_property_name(name: &str) -> Result<String> {
    json_string_literal(name)
}

pub(crate) fn render_js_property_access(value_expr: &str, property_name: &str) -> Result<String> {
    Ok(format!(
        "{}[{}]",
        value_expr,
        json_string_literal(property_name)?
    ))
}

pub(crate) fn json_string_literal(value: &str) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

pub(crate) fn js_identifier(name: &str) -> String {
    sanitize_identifier(name, false)
}

pub(crate) fn js_member_identifier(name: &str) -> String {
    sanitize_identifier(name, true)
}

pub(crate) fn ffi_symbol_identifier(name: &str) -> String {
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

pub(crate) fn ffi_clone_symbol_name(namespace: &str, object_name: &str) -> String {
    let namespace = namespace.replace("::", "__");
    let object_name = object_name.to_ascii_lowercase();
    format!("uniffi_{namespace}_fn_clone_{object_name}")
}

pub(crate) fn ffi_free_symbol_name(namespace: &str, object_name: &str) -> String {
    let namespace = namespace.replace("::", "__");
    let object_name = object_name.to_ascii_lowercase();
    format!("uniffi_{namespace}_fn_free_{object_name}")
}

pub(crate) fn callback_method_ffi_name(callback_interface_name: &str, index: usize) -> String {
    format!("CallbackInterface{callback_interface_name}Method{index}")
}

pub(crate) fn foreign_future_complete_ffi_name(return_ffi_type: Option<&FfiType>) -> String {
    format!(
        "ForeignFutureComplete{}",
        FfiType::return_type_name(return_ffi_type).to_upper_camel_case()
    )
}

pub(crate) fn ffi_opaque_identifier(name: &str) -> String {
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

pub(crate) fn validate_arguments_renderable(
    arguments: &[ArgumentModel],
    context: &str,
) -> Result<()> {
    for argument in arguments {
        validate_type_renderable(
            &argument.type_,
            &format!("{context} argument {}", argument.name),
        )?;
    }
    Ok(())
}

pub(crate) fn validate_optional_type_renderable(type_: Option<&Type>, context: &str) -> Result<()> {
    if let Some(type_) = type_ {
        validate_type_renderable(type_, context)?;
    }
    Ok(())
}

pub(crate) fn validate_type_renderable(type_: &Type, context: &str) -> Result<()> {
    render_public_type(type_)
        .with_context(|| format!("unsupported public Node API type for {context}"))?;
    Ok(())
}
