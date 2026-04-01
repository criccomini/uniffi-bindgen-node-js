use std::collections::BTreeSet;

use anyhow::{Context, Result, bail};
use heck::ToUpperCamelCase;
use textwrap::dedent;
use uniffi_bindgen::interface::{ComponentInterface, Type, UniffiTraitMethods};

use super::{ArgumentModel, FieldModel, RecordModel};

pub(crate) fn validate_supported_features(ci: &ComponentInterface) -> Result<()> {
    let mut unsupported = Vec::new();

    push_unsupported_feature(
        &mut unsupported,
        "external types are not supported in generated Node bindings",
        ci.iter_external_types().map(describe_type).collect(),
    );
    push_unsupported_feature(
        &mut unsupported,
        "custom types are not supported in generated Node bindings",
        ci.iter_local_types()
            .chain(ci.iter_external_types())
            .filter_map(custom_type_name)
            .collect(),
    );
    push_unsupported_feature(
        &mut unsupported,
        "record constructors are not supported in generated Node bindings",
        collect_record_constructor_names(ci),
    );
    push_unsupported_feature(
        &mut unsupported,
        "record methods are not supported in generated Node bindings",
        collect_record_method_names(ci),
    );
    push_unsupported_feature(
        &mut unsupported,
        "enum constructors are not supported in generated Node bindings",
        collect_enum_constructor_names(ci),
    );
    push_unsupported_feature(
        &mut unsupported,
        "enum methods are not supported in generated Node bindings",
        collect_enum_method_names(ci),
    );
    push_unsupported_feature(
        &mut unsupported,
        "record UniFFI trait methods are not supported in generated Node bindings",
        collect_record_trait_method_names(ci),
    );
    push_unsupported_feature(
        &mut unsupported,
        "enum UniFFI trait methods are not supported in generated Node bindings",
        collect_enum_trait_method_names(ci),
    );
    push_unsupported_feature(
        &mut unsupported,
        "non-exhaustive enums are not supported in generated Node bindings",
        collect_non_exhaustive_enum_names(ci),
    );
    push_unsupported_feature(
        &mut unsupported,
        "object UniFFI trait methods are not supported in generated Node bindings",
        collect_object_trait_method_names(ci),
    );

    if unsupported.is_empty() {
        return Ok(());
    }

    bail!(
        "unsupported UniFFI features for generated Node bindings:\n- {}",
        unsupported.join("\n- ")
    );
}

fn push_unsupported_feature(
    unsupported: &mut Vec<String>,
    message_prefix: &str,
    names: BTreeSet<String>,
) {
    if names.is_empty() {
        return;
    }

    unsupported.push(format!(
        "{message_prefix}: {}",
        names.into_iter().collect::<Vec<_>>().join(", ")
    ));
}

fn custom_type_name(type_: &Type) -> Option<String> {
    match type_ {
        Type::Custom { name, .. } => Some(name.clone()),
        _ => None,
    }
}

fn collect_record_method_names(ci: &ComponentInterface) -> BTreeSet<String> {
    ci.record_definitions()
        .iter()
        .flat_map(|record| {
            record
                .methods()
                .iter()
                .map(move |method| describe_member(record.name(), method.name()))
        })
        .collect()
}

fn collect_record_constructor_names(ci: &ComponentInterface) -> BTreeSet<String> {
    ci.record_definitions()
        .iter()
        .flat_map(|record| {
            record
                .constructors()
                .iter()
                .map(move |constructor| describe_member(record.name(), constructor.name()))
        })
        .collect()
}

fn collect_enum_method_names(ci: &ComponentInterface) -> BTreeSet<String> {
    ci.enum_definitions()
        .iter()
        .flat_map(|enum_def| {
            enum_def
                .methods()
                .iter()
                .map(move |method| describe_member(enum_def.name(), method.name()))
        })
        .collect()
}

fn collect_enum_constructor_names(ci: &ComponentInterface) -> BTreeSet<String> {
    ci.enum_definitions()
        .iter()
        .flat_map(|enum_def| {
            enum_def
                .constructors()
                .iter()
                .map(move |constructor| describe_member(enum_def.name(), constructor.name()))
        })
        .collect()
}

fn collect_record_trait_method_names(ci: &ComponentInterface) -> BTreeSet<String> {
    ci.record_definitions()
        .iter()
        .filter_map(|record| {
            describe_uniffi_trait_methods(record.name(), record.uniffi_trait_methods())
        })
        .collect()
}

fn collect_enum_trait_method_names(ci: &ComponentInterface) -> BTreeSet<String> {
    ci.enum_definitions()
        .iter()
        .filter_map(|enum_def| {
            describe_uniffi_trait_methods(enum_def.name(), enum_def.uniffi_trait_methods())
        })
        .collect()
}

fn collect_non_exhaustive_enum_names(ci: &ComponentInterface) -> BTreeSet<String> {
    ci.enum_definitions()
        .iter()
        .filter(|enum_def| enum_def.is_non_exhaustive())
        .map(|enum_def| enum_def.name().to_string())
        .collect()
}

fn collect_object_trait_method_names(ci: &ComponentInterface) -> BTreeSet<String> {
    ci.object_definitions()
        .iter()
        .filter_map(|object| {
            describe_uniffi_trait_methods(object.name(), object.uniffi_trait_methods())
        })
        .collect()
}

fn describe_member(type_name: &str, member_name: &str) -> String {
    format!("{type_name}.{member_name}")
}

fn describe_uniffi_trait_methods(type_name: &str, methods: UniffiTraitMethods) -> Option<String> {
    let mut trait_names = Vec::new();

    if methods.debug_fmt.is_some() {
        trait_names.push("Debug");
    }
    if methods.display_fmt.is_some() {
        trait_names.push("Display");
    }
    if methods.eq_eq.is_some() || methods.eq_ne.is_some() {
        trait_names.push("Eq");
    }
    if methods.hash_hash.is_some() {
        trait_names.push("Hash");
    }
    if methods.ord_cmp.is_some() {
        trait_names.push("Ord");
    }

    if trait_names.is_empty() {
        None
    } else {
        Some(format!("{type_name} ({})", trait_names.join(", ")))
    }
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
        Type::Timestamp => Ok("Date".to_string()),
        Type::Duration => Ok("number".to_string()),
        Type::Custom { name, .. } => {
            bail!("custom type '{name}' is not supported in the public Node API yet")
        }
    }
}

pub(crate) fn render_doc_comment(docstring: Option<&str>, indent: &str) -> String {
    let Some(docstring) = docstring else {
        return String::new();
    };

    let docstring = dedent(docstring);
    let lines = docstring.lines().map(str::trim_end).collect::<Vec<_>>();
    let Some(start) = lines.iter().position(|line| !line.trim().is_empty()) else {
        return String::new();
    };
    let end = lines
        .iter()
        .rposition(|line| !line.trim().is_empty())
        .expect("start guarantees at least one non-empty line")
        + 1;

    let mut rendered = String::new();
    rendered.push_str(indent);
    rendered.push_str("/**\n");
    for line in &lines[start..end] {
        rendered.push_str(indent);
        if line.trim().is_empty() {
            rendered.push_str(" *\n");
            continue;
        }

        rendered.push_str(" * ");
        rendered.push_str(&line.replace("*/", "*\\/"));
        rendered.push('\n');
    }
    rendered.push_str(indent);
    rendered.push_str(" */\n");
    rendered
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
    render_js_ffi_call_args_impl(&[], arguments, trailing)
}

pub(crate) fn render_js_ffi_call_args_with_leading(
    leading: &[String],
    arguments: &[ArgumentModel],
    trailing: Option<&str>,
) -> String {
    render_js_ffi_call_args_impl(leading, arguments, trailing)
}

fn render_js_ffi_call_args_impl(
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

fn scalar_converter_name(type_: &Type) -> Option<&'static str> {
    match type_ {
        Type::UInt8 => Some("FfiConverterUInt8"),
        Type::Int8 => Some("FfiConverterInt8"),
        Type::UInt16 => Some("FfiConverterUInt16"),
        Type::Int16 => Some("FfiConverterInt16"),
        Type::UInt32 => Some("FfiConverterUInt32"),
        Type::Int32 => Some("FfiConverterInt32"),
        Type::UInt64 => Some("FfiConverterUInt64"),
        Type::Int64 => Some("FfiConverterInt64"),
        Type::Float32 => Some("FfiConverterFloat32"),
        Type::Float64 => Some("FfiConverterFloat64"),
        Type::Boolean => Some("FfiConverterBool"),
        _ => None,
    }
}

fn builtin_converter_name(type_: &Type) -> Option<&'static str> {
    match type_ {
        Type::String => Some("FfiConverterString"),
        Type::Bytes => Some("FfiConverterBytes"),
        Type::Timestamp => Some("FfiConverterTimestamp"),
        Type::Duration => Some("FfiConverterDuration"),
        _ => scalar_converter_name(type_),
    }
}

fn primitive_koffi_type_literal(type_: &Type) -> Option<&'static str> {
    match type_ {
        Type::UInt8 => Some("\"uint8_t\""),
        Type::Int8 => Some("\"int8_t\""),
        Type::UInt16 => Some("\"uint16_t\""),
        Type::Int16 => Some("\"int16_t\""),
        Type::UInt32 => Some("\"uint32_t\""),
        Type::Int32 => Some("\"int32_t\""),
        Type::UInt64 | Type::CallbackInterface { .. } => Some("\"uint64_t\""),
        Type::Int64 => Some("\"int64_t\""),
        Type::Float32 => Some("\"float\""),
        Type::Float64 => Some("\"double\""),
        Type::Boolean => Some("\"int8_t\""),
        _ => None,
    }
}

fn uses_buffer_converter(type_: &Type) -> bool {
    matches!(
        type_,
        Type::Bytes
            | Type::Record { .. }
            | Type::Enum { .. }
            | Type::Optional { .. }
            | Type::Sequence { .. }
            | Type::Map { .. }
            | Type::Timestamp
            | Type::Duration
    )
}

fn uses_rust_buffer_ffi(type_: &Type) -> bool {
    matches!(type_, Type::String) || uses_buffer_converter(type_)
}

fn render_scalar_converter_call(
    type_: &Type,
    value_expr: &str,
    method_name: &str,
) -> Option<String> {
    scalar_converter_name(type_)
        .map(|converter_name| format!("{converter_name}.{method_name}({value_expr})"))
}

#[derive(Copy, Clone)]
enum JsValueTransform {
    Lower,
    Lift,
}

impl JsValueTransform {
    fn converter_method(self) -> &'static str {
        match self {
            Self::Lower => "lower",
            Self::Lift => "lift",
        }
    }

    fn buffer_helper(self) -> &'static str {
        match self {
            Self::Lower => "uniffiLowerIntoRustBuffer",
            Self::Lift => "uniffiLiftFromRustBuffer",
        }
    }

    fn string_expression(self, value_expr: &str) -> String {
        match self {
            Self::Lower => format!("uniffiLowerString({value_expr})"),
            Self::Lift => format!("uniffiLiftStringFromRustBuffer({value_expr})"),
        }
    }

    fn object_factory_method(self) -> &'static str {
        match self {
            Self::Lower => "cloneHandle",
            Self::Lift => "create",
        }
    }
}

fn render_js_buffer_expression(
    type_: &Type,
    value_expr: &str,
    transform: JsValueTransform,
) -> Result<String> {
    Ok(format!(
        "{}({}, {value_expr})",
        transform.buffer_helper(),
        render_js_type_converter_expression(type_)?
    ))
}

fn render_js_converter_expression(
    type_: &Type,
    value_expr: &str,
    transform: JsValueTransform,
) -> Result<String> {
    Ok(format!(
        "{}.{}({value_expr})",
        render_js_type_converter_expression(type_)?,
        transform.converter_method()
    ))
}

fn render_js_object_transform(
    object_name: &str,
    has_callback_interface: bool,
    value_expr: &str,
    transform: JsValueTransform,
) -> String {
    if has_callback_interface {
        return format!(
            "{}.{}({value_expr})",
            type_converter_name(object_name),
            transform.converter_method()
        );
    }

    format!(
        "{}.{}({value_expr})",
        object_factory_name(object_name),
        transform.object_factory_method()
    )
}

fn render_js_value_transform(
    type_: &Type,
    value_expr: &str,
    transform: JsValueTransform,
) -> Result<String> {
    if let Some(expression) =
        render_scalar_converter_call(type_, value_expr, transform.converter_method())
    {
        return Ok(expression);
    }

    if uses_buffer_converter(type_) {
        return render_js_buffer_expression(type_, value_expr, transform);
    }

    match type_ {
        Type::String => Ok(transform.string_expression(value_expr)),
        Type::Object { name, imp, .. } => Ok(render_js_object_transform(
            name,
            imp.has_callback_interface(),
            value_expr,
            transform,
        )),
        Type::CallbackInterface { .. } => {
            render_js_converter_expression(type_, value_expr, transform)
        }
        Type::Custom { name, .. } => bail!("custom type '{name}' is not supported"),
        _ => unreachable!("all supported value transforms should have been handled"),
    }
}

pub(crate) fn render_js_lower_expression(type_: &Type, value_expr: &str) -> Result<String> {
    render_js_value_transform(type_, value_expr, JsValueTransform::Lower)
}

pub(crate) fn render_js_lift_expression(type_: &Type, value_expr: &str) -> Result<String> {
    render_js_value_transform(type_, value_expr, JsValueTransform::Lift)
}

fn async_opaque_object_name(return_type: Option<&Type>) -> Option<&str> {
    match return_type {
        Some(Type::Object { name, imp, .. }) if !imp.has_callback_interface() => Some(name),
        _ => None,
    }
}

pub(crate) fn render_js_async_lift_closure(return_type: Option<&Type>) -> Result<String> {
    if let Some(object_name) = async_opaque_object_name(return_type) {
        return Ok(format!(
            "(uniffiResult) => {}.createRawExternal(uniffiResult)",
            object_factory_name(object_name)
        ));
    }

    match return_type {
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
    if async_opaque_object_name(return_type).is_some() {
        return render_js_async_object_complete_setup(complete_identifier, indent);
    }

    Ok(vec![format!(
        "{indent}const completeFunc = (rustFuture, status) => ffiFunctions.{complete_identifier}(rustFuture, status);"
    )])
}

pub(crate) fn render_js_async_object_complete_setup(
    complete_identifier: &str,
    indent: &str,
) -> Result<Vec<String>> {
    Ok(vec![
        format!("{indent}const completePointer = uniffiGetCachedLibraryFunction("),
        format!(
            "{indent}  {},",
            json_string_literal(&format!("complete:{complete_identifier}"))?
        ),
        format!("{indent}  (bindings) => bindings.library.func("),
        format!("{indent}    {},", json_string_literal(complete_identifier)?),
        format!("{indent}    bindings.ffiTypes.VoidPointer,"),
        format!(
            "{indent}    [bindings.ffiTypes.UniffiHandle, koffi.pointer(bindings.ffiTypes.RustCallStatus)],"
        ),
        format!("{indent}  ),"),
        format!("{indent});"),
        format!(
            "{indent}const completeFunc = (rustFuture, status) => completePointer(rustFuture, status);"
        ),
    ])
}

pub(crate) fn render_js_type_converter_expression(type_: &Type) -> Result<String> {
    if let Some(converter_name) = builtin_converter_name(type_) {
        return Ok(converter_name.to_string());
    }

    match type_ {
        Type::Optional { inner_type } => Ok(format!(
            "uniffiOptionalConverter({})",
            render_js_type_converter_expression(inner_type)?
        )),
        Type::Sequence { inner_type } => Ok(format!(
            "uniffiArrayConverter({})",
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
        Type::Custom { name, .. } => bail!("custom type '{name}' is not supported"),
        _ => unreachable!("all supported converter-backed types should have been handled"),
    }
}

pub(crate) fn render_js_koffi_type_expression(
    type_: &Type,
    ffi_bindings_expr: &str,
) -> Result<String> {
    if let Some(type_literal) = primitive_koffi_type_literal(type_) {
        return Ok(type_literal.to_string());
    }

    if uses_rust_buffer_ffi(type_) {
        return Ok(format!("{ffi_bindings_expr}.ffiTypes.RustBuffer"));
    }

    match type_ {
        Type::Object { .. } => Ok(format!("{ffi_bindings_expr}.ffiTypes.UniffiHandle")),
        Type::Custom { name, .. } => bail!("custom type '{name}' is not supported"),
        _ => unreachable!("all supported koffi types should have been handled"),
    }
}

pub(crate) fn render_js_default_async_callback_return_value_expression(type_: &Type) -> String {
    if scalar_converter_name(type_).is_some() {
        return "0".to_string();
    }

    if uses_rust_buffer_ffi(type_) {
        return "EMPTY_RUST_BUFFER".to_string();
    }

    match type_ {
        Type::Object { .. } | Type::CallbackInterface { .. } => "0n".to_string(),
        Type::Custom { name, .. } => {
            unreachable!("custom type '{name}' should have been rejected before codegen")
        }
        _ => unreachable!("all supported async callback return types should have been handled"),
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use uniffi_bindgen::interface::ComponentInterface;
    use uniffi_meta::{
        EnumMetadata, EnumShape, Metadata, MetadataGroup, MethodMetadata, NamespaceMetadata,
        ObjectImpl, ObjectMetadata, RecordMetadata, UniffiTraitMetadata,
        VariantMetadata,
    };

    use super::{render_doc_comment, validate_supported_features};

    fn component_interface_from_metadata(
        items: impl IntoIterator<Item = Metadata>,
    ) -> ComponentInterface {
        let mut metadata_items = BTreeSet::new();
        metadata_items.extend(items);

        ComponentInterface::from_metadata(MetadataGroup {
            namespace: NamespaceMetadata {
                crate_name: "fixture_crate".to_string(),
                name: "example".to_string(),
            },
            namespace_docstring: None,
            items: metadata_items,
        })
        .expect("metadata should build a ComponentInterface")
    }

    fn record_metadata(name: &str) -> Metadata {
        RecordMetadata {
            module_path: "fixture_crate".to_string(),
            name: name.to_string(),
            remote: false,
            fields: vec![],
            docstring: None,
        }
        .into()
    }

    fn enum_metadata(name: &str) -> Metadata {
        enum_metadata_with_non_exhaustive(name, false)
    }

    fn enum_metadata_with_non_exhaustive(name: &str, non_exhaustive: bool) -> Metadata {
        EnumMetadata {
            module_path: "fixture_crate".to_string(),
            name: name.to_string(),
            shape: EnumShape::Enum,
            remote: false,
            discr_type: None,
            variants: vec![VariantMetadata {
                name: "Variant".to_string(),
                discr: None,
                fields: vec![],
                docstring: None,
            }],
            non_exhaustive,
            docstring: None,
        }
        .into()
    }

    fn object_metadata(name: &str) -> Metadata {
        ObjectMetadata {
            module_path: "fixture_crate".to_string(),
            name: name.to_string(),
            remote: false,
            imp: ObjectImpl::Struct,
            docstring: None,
        }
        .into()
    }

    fn method_metadata(self_name: &str, name: &str) -> Metadata {
        MethodMetadata {
            module_path: "fixture_crate".to_string(),
            self_name: self_name.to_string(),
            name: name.to_string(),
            is_async: false,
            inputs: vec![],
            return_type: Some(uniffi_meta::Type::String),
            throws: None,
            takes_self_by_arc: false,
            checksum: None,
            docstring: None,
        }
        .into()
    }

    fn uniffi_trait_method(
        self_name: &str,
        name: &str,
        return_type: uniffi_meta::Type,
    ) -> MethodMetadata {
        MethodMetadata {
            module_path: "fixture_crate".to_string(),
            self_name: self_name.to_string(),
            name: name.to_string(),
            is_async: false,
            inputs: vec![],
            return_type: Some(return_type),
            throws: None,
            takes_self_by_arc: false,
            checksum: None,
            docstring: None,
        }
    }

    fn debug_trait_metadata(self_name: &str) -> Metadata {
        UniffiTraitMetadata::Debug {
            fmt: uniffi_trait_method(self_name, "uniffi_trait_debug", uniffi_meta::Type::String),
        }
        .into()
    }

    fn display_trait_metadata(self_name: &str) -> Metadata {
        UniffiTraitMetadata::Display {
            fmt: uniffi_trait_method(
                self_name,
                "uniffi_trait_display",
                uniffi_meta::Type::String,
            ),
        }
        .into()
    }

    fn hash_trait_metadata(self_name: &str) -> Metadata {
        UniffiTraitMetadata::Hash {
            hash: uniffi_trait_method(self_name, "uniffi_trait_hash", uniffi_meta::Type::UInt64),
        }
        .into()
    }

    #[test]
    fn render_doc_comment_returns_empty_for_missing_or_blank_docs() {
        assert_eq!(render_doc_comment(None, ""), "");
        assert_eq!(render_doc_comment(Some(" \n\n"), ""), "");
    }

    #[test]
    fn render_doc_comment_dedents_and_preserves_blank_lines() {
        assert_eq!(
            render_doc_comment(
                Some(
                    "
                        Summary line.

                          Indented detail.
                    "
                ),
                ""
            ),
            "/**\n * Summary line.\n *\n *   Indented detail.\n */\n"
        );
    }

    #[test]
    fn render_doc_comment_escapes_terminators_and_indents_output() {
        assert_eq!(
            render_doc_comment(Some("Ends with */ here."), "  "),
            "  /**\n   * Ends with *\\/ here.\n   */\n"
        );
    }

    #[test]
    fn validate_supported_features_rejects_record_and_enum_methods() {
        let ci = component_interface_from_metadata([
            record_metadata("Profile"),
            enum_metadata("Flavor"),
            method_metadata("Profile", "display_name"),
            method_metadata("Flavor", "label"),
        ]);

        let error = validate_supported_features(&ci)
            .expect_err("record and enum methods should be rejected");

        assert_eq!(
            error.to_string(),
            concat!(
                "unsupported UniFFI features for generated Node bindings:\n",
                "- record methods are not supported in generated Node bindings: Profile.display_name\n",
                "- enum methods are not supported in generated Node bindings: Flavor.label",
            )
        );
    }

    #[test]
    fn validate_supported_features_rejects_record_and_enum_uniffi_trait_methods() {
        let ci = component_interface_from_metadata([
            record_metadata("Profile"),
            enum_metadata("Flavor"),
            debug_trait_metadata("Profile"),
            hash_trait_metadata("Profile"),
            display_trait_metadata("Flavor"),
        ]);

        let error = validate_supported_features(&ci)
            .expect_err("record and enum UniFFI trait methods should be rejected");

        assert_eq!(
            error.to_string(),
            concat!(
                "unsupported UniFFI features for generated Node bindings:\n",
                "- record UniFFI trait methods are not supported in generated Node bindings: Profile (Debug, Hash)\n",
                "- enum UniFFI trait methods are not supported in generated Node bindings: Flavor (Display)",
            )
        );
    }

    #[test]
    fn validate_supported_features_rejects_non_exhaustive_enums_and_object_trait_methods() {
        let ci = component_interface_from_metadata([
            enum_metadata_with_non_exhaustive("Flavor", true),
            object_metadata("Store"),
            display_trait_metadata("Store"),
        ]);

        let error = validate_supported_features(&ci)
            .expect_err("ignored UniFFI 0.31 surfaces should be rejected");

        assert_eq!(
            error.to_string(),
            concat!(
                "unsupported UniFFI features for generated Node bindings:\n",
                "- non-exhaustive enums are not supported in generated Node bindings: Flavor\n",
                "- object UniFFI trait methods are not supported in generated Node bindings: Store (Display)",
            )
        );
    }
}
