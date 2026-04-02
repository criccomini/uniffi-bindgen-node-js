mod ir;
mod render;
mod support;

use anyhow::{Context, Result};
use uniffi_bindgen::interface::Type;

use self::ir::AsyncScaffoldingModel;
pub(crate) use self::ir::{
    ArgumentModel, CallbackInterfaceModel, ComponentModel, ConstructorModel, EnumModel, ErrorModel,
    FieldModel, FunctionModel, MethodModel, ObjectModel, RecordModel, build_public_api_ir,
};
use self::render::{
    JsRenderSections, PublicApiRenderer, render_js_async_rust_future_helpers_fragment,
    render_js_callback_interface_converter_fragment, render_js_error_converter_fragment,
    render_js_error_fragment, render_js_flat_enum_converter_fragment, render_js_flat_enum_fragment,
    render_js_function_fragment, render_js_object_fragment, render_js_record_converter_fragment,
    render_js_runtime_helpers_fragment, render_js_runtime_hooks_fragment,
    render_js_tagged_enum_converter_fragment, render_js_tagged_enum_fragment,
};
pub(crate) use self::support::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderedComponentApi {
    pub namespace_doc_comment: String,
    pub js: String,
    pub dts: String,
    pub requires_async_rust_future_hooks: bool,
}

pub(crate) fn render_public_api(model: &ComponentModel) -> Result<RenderedComponentApi> {
    PublicApiTemplateModel::new(model).render()
}

struct PublicApiTemplateModel<'a> {
    component: &'a ComponentModel,
    namespace_doc_comment: String,
    requires_async_rust_future_hooks: bool,
}

impl<'a> PublicApiTemplateModel<'a> {
    fn new(component: &'a ComponentModel) -> Self {
        Self {
            component,
            namespace_doc_comment: render_doc_comment(component.namespace_docstring.as_deref(), ""),
            requires_async_rust_future_hooks: requires_async_rust_future_hooks(component),
        }
    }

    fn render(&self) -> Result<RenderedComponentApi> {
        let renderer = PublicApiRenderer::new(self.component);
        let js_sections = self.build_js_render_sections()?;

        Ok(RenderedComponentApi {
            namespace_doc_comment: self.namespace_doc_comment.clone(),
            js: renderer.render_js(js_sections)?,
            dts: renderer.render_dts()?,
            requires_async_rust_future_hooks: self.requires_async_rust_future_hooks,
        })
    }

    fn build_js_render_sections(&self) -> Result<JsRenderSections> {
        let mut sections = self.build_js_declaration_sections()?;
        self.populate_js_runtime_sections(&mut sections)?;
        Ok(sections)
    }

    fn build_js_declaration_sections(&self) -> Result<JsRenderSections> {
        Ok(JsRenderSections {
            unimplemented_helper: self.render_unimplemented_helper(),
            flat_enums: render_fragments(&self.component.flat_enums, render_js_flat_enum_fragment)?,
            tagged_enums: render_fragments(
                &self.component.tagged_enums,
                render_js_tagged_enum_fragment,
            )?,
            errors: render_fragments(&self.component.errors, render_js_error_fragment)?,
            placeholder_converters: self.render_optional_placeholder_converters()?,
            functions: render_fragments(&self.component.functions, render_js_function_fragment)?,
            objects: render_fragments(&self.component.objects, render_js_object_fragment)?,
            ..JsRenderSections::default()
        })
    }

    fn populate_js_runtime_sections(&self, sections: &mut JsRenderSections) -> Result<()> {
        sections.runtime_helpers = self.render_runtime_helpers()?;
        sections.async_rust_future_helpers = self.render_async_rust_future_helpers()?;
        sections.runtime_hooks = self.render_runtime_hooks()?;
        Ok(())
    }

    fn render_unimplemented_helper(&self) -> String {
        if self.component.functions.is_empty() && self.component.objects.is_empty() {
            return String::new();
        }

        "function uniffiNotImplemented(member) {\n  throw new Error(`${member} is not implemented yet. Koffi-backed bindings are still pending.`);\n}"
            .to_string()
    }

    fn render_runtime_helpers(&self) -> Result<String> {
        if self.component.objects.is_empty() && !has_placeholder_converters(self.component) {
            return Ok(String::new());
        }

        render_js_runtime_helpers_fragment(
            &self.component.ffi_rustbuffer_from_bytes_identifier,
            &self.component.ffi_rustbuffer_free_identifier,
        )
    }

    fn render_async_rust_future_helpers(&self) -> Result<String> {
        if !self.requires_async_rust_future_hooks {
            return Ok(String::new());
        }

        render_js_async_rust_future_helpers_fragment()
    }

    fn render_optional_placeholder_converters(&self) -> Result<String> {
        if !has_placeholder_converters(self.component) {
            return Ok(String::new());
        }

        self.render_js_placeholder_converters()
    }

    fn render_runtime_hooks(&self) -> Result<String> {
        if self.component.callback_interfaces.is_empty() && !self.requires_async_rust_future_hooks {
            return Ok(String::new());
        }

        render_js_runtime_hooks_fragment(
            &self.component.callback_interfaces,
            self.requires_async_rust_future_hooks,
        )
    }

    fn render_js_placeholder_converters(&self) -> Result<String> {
        let mut lines =
            render_fragments(&self.component.records, render_js_record_converter_fragment)?;
        lines.extend(render_fragments(
            &self.component.flat_enums,
            render_js_flat_enum_converter_fragment,
        )?);
        lines.extend(render_fragments(
            &self.component.tagged_enums,
            render_js_tagged_enum_converter_fragment,
        )?);
        lines.extend(render_fragments(
            &self.component.errors,
            render_js_error_converter_fragment,
        )?);
        lines.extend(render_fragments(
            &self.component.callback_interfaces,
            render_js_callback_interface_converter_fragment,
        )?);

        Ok(lines.join("\n"))
    }
}

impl ComponentModel {
    fn validate_renderable_types(&self) -> Result<()> {
        self.validate_renderable_functions()?;
        self.validate_renderable_data_types()?;
        self.validate_renderable_callback_interfaces()?;
        self.validate_renderable_objects()?;
        Ok(())
    }

    fn validate_renderable_functions(&self) -> Result<()> {
        self.functions
            .iter()
            .try_for_each(validate_function_renderable)
    }

    fn validate_renderable_data_types(&self) -> Result<()> {
        self.records
            .iter()
            .try_for_each(validate_record_renderable)?;
        self.flat_enums
            .iter()
            .try_for_each(validate_enum_renderable)?;
        self.tagged_enums
            .iter()
            .try_for_each(validate_enum_renderable)?;
        self.errors.iter().try_for_each(validate_error_renderable)
    }

    fn validate_renderable_callback_interfaces(&self) -> Result<()> {
        self.callback_interfaces
            .iter()
            .try_for_each(validate_callback_interface_renderable)
    }

    fn validate_renderable_objects(&self) -> Result<()> {
        self.objects.iter().try_for_each(validate_object_renderable)
    }
}

#[cfg(test)]
impl ComponentModel {
    pub(crate) fn render_public_api(&self) -> Result<RenderedComponentApi> {
        render_public_api(self)
    }
}

fn has_placeholder_converters(component: &ComponentModel) -> bool {
    has_data_type_placeholder_converters(component) || !component.callback_interfaces.is_empty()
}

fn has_data_type_placeholder_converters(component: &ComponentModel) -> bool {
    !component.records.is_empty()
        || !component.flat_enums.is_empty()
        || !component.tagged_enums.is_empty()
        || !component.errors.is_empty()
}

fn requires_async_rust_future_hooks(component: &ComponentModel) -> bool {
    has_async_functions(component)
        || has_async_objects(component)
        || has_async_callback_interfaces(component)
}

fn has_async_functions(component: &ComponentModel) -> bool {
    component.functions.iter().any(|function| function.is_async)
}

fn has_async_objects(component: &ComponentModel) -> bool {
    component.objects.iter().any(object_has_async_members)
}

fn has_async_callback_interfaces(component: &ComponentModel) -> bool {
    component
        .callback_interfaces
        .iter()
        .any(callback_interface_has_async_methods)
}

fn render_fragments<T>(items: &[T], render: impl Fn(&T) -> Result<String>) -> Result<Vec<String>> {
    items.iter().map(render).collect()
}

fn object_has_async_members(object: &ObjectModel) -> bool {
    object
        .constructors
        .iter()
        .any(|constructor| constructor.is_async)
        || object.methods.iter().any(|method| method.is_async)
}

fn callback_interface_has_async_methods(callback_interface: &CallbackInterfaceModel) -> bool {
    callback_interface
        .methods
        .iter()
        .any(|method| method.is_async)
}

fn validate_function_renderable(function: &FunctionModel) -> Result<()> {
    validate_callable_renderable(
        &function.arguments,
        function.return_type.as_ref(),
        function.throws_type.as_ref(),
        &format!("function {}", function.name),
    )
}

fn validate_record_renderable(record: &RecordModel) -> Result<()> {
    for field in &record.fields {
        validate_type_renderable(
            &field.type_,
            &format!("record {} field {}", record.name, field.name),
        )?;
    }

    Ok(())
}

fn validate_enum_renderable(enum_def: &EnumModel) -> Result<()> {
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

    Ok(())
}

fn validate_error_renderable(error: &ErrorModel) -> Result<()> {
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

    Ok(())
}

fn validate_callback_interface_renderable(
    callback_interface: &CallbackInterfaceModel,
) -> Result<()> {
    callback_interface.methods.iter().try_for_each(|method| {
        validate_callable_renderable(
            &method.arguments,
            method.return_type.as_ref(),
            method.throws_type.as_ref(),
            &format!(
                "callback interface {}.{}",
                callback_interface.name, method.name
            ),
        )
    })
}

fn validate_object_renderable(object: &ObjectModel) -> Result<()> {
    object.constructors.iter().try_for_each(|constructor| {
        validate_callable_renderable(
            &constructor.arguments,
            None,
            constructor.throws_type.as_ref(),
            &format!("constructor {}.{}", object.name, constructor.name),
        )
    })?;

    object.methods.iter().try_for_each(|method| {
        validate_callable_renderable(
            &method.arguments,
            method.return_type.as_ref(),
            method.throws_type.as_ref(),
            &format!("method {}.{}", object.name, method.name),
        )
    })
}

fn validate_callable_renderable(
    arguments: &[ArgumentModel],
    return_type: Option<&Type>,
    throws_type: Option<&Type>,
    context: &str,
) -> Result<()> {
    validate_arguments_renderable(arguments, context)?;
    validate_optional_type_renderable(return_type, &format!("{context} return type"))?;
    validate_optional_type_renderable(throws_type, &format!("{context} error type"))?;
    Ok(())
}

fn render_sync_rust_call_lines(
    indent: &str,
    ffi_call_expr: String,
    throws_type: Option<&Type>,
    result_name: Option<&str>,
) -> Result<Vec<String>> {
    let nested_indent = format!("{indent}  ");

    Ok(vec![
        match result_name {
            Some(result_name) => {
                format!("{indent}const {result_name} = uniffiRustCaller.rustCall(")
            }
            None => format!("{indent}uniffiRustCaller.rustCall("),
        },
        format!("{nested_indent}(status) => {ffi_call_expr},"),
        format!(
            "{nested_indent}{},",
            render_js_rust_call_options_expression(throws_type)?
        ),
        format!("{indent});"),
    ])
}

fn render_async_rust_call_lines(
    indent: &str,
    rust_future_func_expr: String,
    async_ffi: &AsyncScaffoldingModel,
    lift_func_expr: String,
    throws_type: Option<&Type>,
    complete_before_free: bool,
) -> Result<Vec<String>> {
    let nested_indent = format!("{indent}  ");
    let free_func = format!(
        "{nested_indent}freeFunc: (rustFuture) => ffiFunctions.{}(rustFuture),",
        async_ffi.free_identifier
    );
    let mut lines = vec![
        format!("{indent}return rustCallAsync({{"),
        format!("{nested_indent}rustFutureFunc: () => {rust_future_func_expr},"),
        format!(
            "{nested_indent}pollFunc: (rustFuture, _continuationCallback, continuationHandle) => ffiFunctions.{}(rustFuture, uniffiGetRustFutureContinuationPointer(), continuationHandle),",
            async_ffi.poll_identifier
        ),
        format!(
            "{nested_indent}cancelFunc: (rustFuture) => ffiFunctions.{}(rustFuture),",
            async_ffi.cancel_identifier
        ),
    ];

    if complete_before_free {
        lines.push(format!("{nested_indent}completeFunc,"));
        lines.push(free_func);
    } else {
        lines.push(free_func);
        lines.push(format!("{nested_indent}completeFunc,"));
    }

    lines.extend([
        format!("{nested_indent}liftFunc: {lift_func_expr},"),
        format!(
            "{nested_indent}...{},",
            render_js_rust_call_options_expression(throws_type)?
        ),
        format!("{indent}}});"),
    ]);

    Ok(lines)
}

fn render_object_method_dispatch_lines(
    factory_name: &str,
    ffi_func_identifier: &str,
) -> Vec<String> {
    vec![
        format!(
            "    const loweredSelf = {}.cloneHandle(this);",
            factory_name
        ),
        "    const ffiMethod =".to_string(),
        format!("      {}.usesGenericAbi(this)", factory_name),
        format!("        ? ffiFunctions.{}_generic_abi", ffi_func_identifier),
        format!("        : ffiFunctions.{};", ffi_func_identifier),
    ]
}

enum AsyncCompleteSetup<'a> {
    Value {
        return_type: Option<&'a Type>,
        complete_identifier: &'a str,
    },
    Object {
        complete_identifier: &'a str,
    },
}

struct AsyncJsCall<'a> {
    indent: &'a str,
    arguments: &'a [ArgumentModel],
    start_call_expr: String,
    async_ffi: &'a AsyncScaffoldingModel,
    lift_func_expr: String,
    throws_type: Option<&'a Type>,
    complete_setup: AsyncCompleteSetup<'a>,
    complete_before_free: bool,
}

fn render_async_complete_setup_lines(
    indent: &str,
    complete_setup: AsyncCompleteSetup<'_>,
) -> Result<Vec<String>> {
    match complete_setup {
        AsyncCompleteSetup::Value {
            return_type,
            complete_identifier,
        } => render_js_async_complete_setup(return_type, complete_identifier, indent),
        AsyncCompleteSetup::Object {
            complete_identifier,
        } => render_js_async_object_complete_setup(complete_identifier, indent),
    }
}

fn render_async_js_call_body(call: AsyncJsCall<'_>) -> Result<Vec<String>> {
    let mut lines = render_js_argument_lowering(call.arguments)?;
    lines.extend(render_async_complete_setup_lines(
        call.indent,
        call.complete_setup,
    )?);
    lines.extend(render_async_rust_call_lines(
        call.indent,
        call.start_call_expr,
        call.async_ffi,
        call.lift_func_expr,
        call.throws_type,
        call.complete_before_free,
    )?);
    Ok(lines)
}

fn render_sync_js_return_line(indent: &str, return_type: Option<&Type>) -> Result<Option<String>> {
    return_type
        .map(|return_type| {
            Ok(format!(
                "{indent}return {};",
                render_js_lift_expression(return_type, "uniffiResult")?
            ))
        })
        .transpose()
}

fn render_sync_js_call_body(
    indent: &str,
    arguments: &[ArgumentModel],
    ffi_call_expr: String,
    throws_type: Option<&Type>,
    return_type: Option<&Type>,
) -> Result<Vec<String>> {
    let mut lines = render_js_argument_lowering(arguments)?;
    lines.extend(render_sync_rust_call_lines(
        indent,
        ffi_call_expr,
        throws_type,
        return_type.map(|_| "uniffiResult"),
    )?);

    if let Some(return_line) = render_sync_js_return_line(indent, return_type)? {
        lines.push(return_line);
    }

    Ok(lines)
}

fn render_constructor_return_line(attach_target: Option<&str>, factory_name: &str) -> String {
    match attach_target {
        Some(target) => format!("    return {}.attach({}, pointer);", factory_name, target),
        None => format!("    return {}.create(pointer);", factory_name),
    }
}

pub(super) fn render_js_function_body_lines(function: &FunctionModel) -> Result<Vec<String>> {
    if function.is_async {
        render_js_async_function_body(function)
    } else {
        render_js_sync_function_body(function)
    }
}

pub(super) fn render_js_primary_constructor_body_lines(
    constructor: &ConstructorModel,
    factory_name: &str,
) -> Result<Vec<String>> {
    render_js_sync_constructor_body(constructor, Some("this"), factory_name)
}

pub(super) fn render_js_object_constructor_body_lines(
    constructor: &ConstructorModel,
    factory_name: &str,
    object_name: &str,
) -> Result<Vec<String>> {
    render_js_constructor_body(constructor, factory_name, object_name)
}

pub(super) fn render_js_object_method_body_lines(
    method: &MethodModel,
    factory_name: &str,
    object_name: &str,
) -> Result<Vec<String>> {
    if method.is_async {
        render_js_async_method_body(method, factory_name, object_name)
    } else {
        render_js_sync_method_body(method, factory_name, object_name)
    }
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
    lines.extend(render_sync_rust_call_lines(
        "    ",
        format!(
            "ffiFunctions.{}({})",
            constructor.ffi_func_identifier, call_args
        ),
        constructor.throws_type.as_ref(),
        Some("pointer"),
    )?);
    lines.push(render_constructor_return_line(attach_target, factory_name));
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
    let start_args = render_js_ffi_call_args(&constructor.arguments, None);
    render_async_js_call_body(AsyncJsCall {
        indent: "    ",
        arguments: &constructor.arguments,
        start_call_expr: format!(
            "ffiFunctions.{}({})",
            constructor.ffi_func_identifier, start_args
        ),
        async_ffi,
        lift_func_expr: format!("(pointer) => {}.createRawExternal(pointer)", factory_name),
        throws_type: constructor.throws_type.as_ref(),
        complete_setup: AsyncCompleteSetup::Object {
            complete_identifier: &async_ffi.complete_identifier,
        },
        complete_before_free: false,
    })
}

fn render_js_sync_method_body(
    method: &MethodModel,
    factory_name: &str,
    _object_name: &str,
) -> Result<Vec<String>> {
    let mut lines = render_object_method_dispatch_lines(factory_name, &method.ffi_func_identifier);
    let call_args = render_js_ffi_call_args_with_leading(
        &[String::from("loweredSelf")],
        &method.arguments,
        Some("status"),
    );

    lines.extend(render_sync_js_call_body(
        "    ",
        &method.arguments,
        format!("ffiMethod({call_args})"),
        method.throws_type.as_ref(),
        method.return_type.as_ref(),
    )?);

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
    let mut lines = render_object_method_dispatch_lines(factory_name, &method.ffi_func_identifier);
    let start_args = render_js_ffi_call_args_with_leading(
        &[String::from("loweredSelf")],
        &method.arguments,
        None,
    );
    lines.extend(render_async_js_call_body(AsyncJsCall {
        indent: "    ",
        arguments: &method.arguments,
        start_call_expr: format!("ffiMethod({start_args})"),
        async_ffi,
        lift_func_expr: render_js_async_lift_closure(method.return_type.as_ref())?,
        throws_type: method.throws_type.as_ref(),
        complete_setup: AsyncCompleteSetup::Value {
            return_type: method.return_type.as_ref(),
            complete_identifier: &async_ffi.complete_identifier,
        },
        complete_before_free: true,
    })?);

    Ok(lines)
}

fn render_js_sync_function_body(function: &FunctionModel) -> Result<Vec<String>> {
    let call_args = render_js_ffi_call_args(&function.arguments, Some("status"));
    render_sync_js_call_body(
        "  ",
        &function.arguments,
        format!("ffiFunctions.{}({call_args})", function.ffi_func_identifier),
        function.throws_type.as_ref(),
        function.return_type.as_ref(),
    )
}

fn render_js_async_function_body(function: &FunctionModel) -> Result<Vec<String>> {
    let async_ffi = function.async_ffi.as_ref().with_context(|| {
        format!(
            "async function {} is missing future scaffolding identifiers",
            function.name
        )
    })?;
    let start_args = render_js_ffi_call_args(&function.arguments, None);
    render_async_js_call_body(AsyncJsCall {
        indent: "  ",
        arguments: &function.arguments,
        start_call_expr: format!(
            "ffiFunctions.{}({start_args})",
            function.ffi_func_identifier
        ),
        async_ffi,
        lift_func_expr: render_js_async_lift_closure(function.return_type.as_ref())?,
        throws_type: function.throws_type.as_ref(),
        complete_setup: AsyncCompleteSetup::Value {
            return_type: function.return_type.as_ref(),
            complete_identifier: &async_ffi.complete_identifier,
        },
        complete_before_free: true,
    })
}

// GENERATED CODE
#[cfg(test)]
mod tests {
    use super::*;
    use uniffi_bindgen::interface::AsType;
    use uniffi_bindgen::interface::ComponentInterface;
    use uniffi_bindgen::interface::Type;

    fn assert_component_js_imports_include_converters(rendered_js: &str, expected: &[&str]) {
        let imports = crate::bindings::ComponentJsImports::from_public_api(rendered_js);

        for expected in expected {
            assert!(
                imports
                    .ffi_converter_imports
                    .contains(&expected.to_string()),
                "missing {expected} in {:?}",
                imports.ffi_converter_imports
            );
        }
    }

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
    fn component_model_uses_uniffi_callback_symbols_for_callback_interfaces() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {};

            callback interface Logger {
                void write(string message);
                [Async] string latest();
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");

        let model = ComponentModel::from_ci(&ci).expect("callback interfaces should build");
        let expected_identifiers = ci.callback_interface_definitions()[0]
            .ffi_callbacks()
            .into_iter()
            .map(|callback| ffi_symbol_identifier(callback.name()))
            .collect::<Vec<_>>();
        let actual_identifiers = model.callback_interfaces[0]
            .methods
            .iter()
            .map(|method| {
                method
                    .ffi_callback_identifier
                    .clone()
                    .expect("callback methods should capture their FFI callback name")
            })
            .collect::<Vec<_>>();

        assert_eq!(actual_identifiers, expected_identifiers);
    }

    #[test]
    fn component_model_uses_uniffi_vtable_field_order_for_callback_interfaces() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {};

            callback interface Logger {
                void write(string message);
                [Async] string latest();
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");

        let model = ComponentModel::from_ci(&ci).expect("callback interfaces should build");
        let vtable_definition = ci.callback_interface_definitions()[0].vtable_definition();
        let vtable_fields = vtable_definition
            .fields()
            .iter()
            .map(|field| field.name())
            .collect::<Vec<_>>();

        assert_eq!(&vtable_fields[..2], ["uniffi_free", "uniffi_clone"]);
        assert_eq!(
            model.callback_interfaces[0]
                .methods
                .iter()
                .map(|method| method.name.as_str())
                .collect::<Vec<_>>(),
            vtable_fields[2..].to_vec()
        );
    }

    #[test]
    fn callback_interface_clone_free_symbols_use_the_crate_name_from_full_module_paths() {
        assert_eq!(
            uniffi_meta::clone_fn_symbol_name("fixture_crate::logging", "Logger"),
            "uniffi_fixture_crate_fn_clone_logger"
        );
        assert_eq!(
            uniffi_meta::free_fn_symbol_name("fixture_crate::logging", "Logger"),
            "uniffi_fixture_crate_fn_free_logger"
        );
    }

    #[test]
    fn component_model_uses_uniffi_callback_symbols_for_callback_trait_objects() {
        let mut ci = ComponentInterface::from_webidl(
            r#"
            namespace example {};

            [Trait, WithForeign]
            interface Logger {
                void write(string message);
                [Async] string latest();
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");
        ci.derive_ffi_funcs()
            .expect("trait callback objects should derive their FFI symbols");

        let model = ComponentModel::from_ci(&ci).expect("callback trait objects should build");
        let expected_identifiers = ci.object_definitions()[0]
            .ffi_callbacks()
            .into_iter()
            .map(|callback| ffi_symbol_identifier(callback.name()))
            .collect::<Vec<_>>();
        let actual_identifiers = model.callback_interfaces[0]
            .methods
            .iter()
            .map(|method| {
                method
                    .ffi_callback_identifier
                    .clone()
                    .expect("callback trait methods should capture their FFI callback name")
            })
            .collect::<Vec<_>>();

        assert_eq!(model.callback_interfaces.len(), 1);
        assert!(model.objects.is_empty());
        assert_eq!(actual_identifiers, expected_identifiers);
    }

    #[test]
    fn component_model_uses_uniffi_vtable_field_order_for_callback_trait_objects() {
        let mut ci = ComponentInterface::from_webidl(
            r#"
            namespace example {};

            [Trait, WithForeign]
            interface Logger {
                void write(string message);
                [Async] string latest();
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");
        ci.derive_ffi_funcs()
            .expect("trait callback objects should derive their FFI symbols");

        let model = ComponentModel::from_ci(&ci).expect("callback trait objects should build");
        let vtable_definition = ci.object_definitions()[0]
            .vtable_definition()
            .expect("trait callback objects should expose a vtable");
        let vtable_fields = vtable_definition
            .fields()
            .iter()
            .map(|field| field.name())
            .collect::<Vec<_>>();

        assert_eq!(&vtable_fields[..2], ["uniffi_free", "uniffi_clone"]);
        assert_eq!(
            model.callback_interfaces[0]
                .methods
                .iter()
                .map(|method| method.name.as_str())
                .collect::<Vec<_>>(),
            vtable_fields[2..].to_vec()
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
        assert!(
            model.callback_interfaces[0].methods[0]
                .async_callback_ffi
                .is_some()
        );
    }

    #[test]
    fn component_model_captures_async_callback_foreign_future_metadata() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {};

            callback interface Logger {
                [Async] string write(string message);
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");

        let model = ComponentModel::from_ci(&ci).expect("async callback interfaces should build");
        let method = &model.callback_interfaces[0].methods[0];
        let async_callback_ffi = method
            .async_callback_ffi
            .as_ref()
            .expect("async callback method should capture ForeignFuture metadata");

        assert_eq!(
            async_callback_ffi.complete_identifier,
            "ForeignFutureCompleteRustBuffer"
        );
        assert_eq!(
            async_callback_ffi.result_struct_identifier,
            "ForeignFutureResultRustBuffer"
        );
        assert!(async_callback_ffi.result_struct_has_return_value);
        assert_eq!(
            async_callback_ffi.dropped_callback_struct_identifier,
            "ForeignFutureDroppedCallbackStruct"
        );
        assert_eq!(
            async_callback_ffi.dropped_callback_identifier,
            "ForeignFutureDroppedCallback"
        );
        assert_eq!(
            async_callback_ffi
                .default_error_return_value_expression
                .as_deref(),
            Some("EMPTY_RUST_BUFFER")
        );
    }

    #[test]
    fn component_model_captures_async_void_callback_foreign_future_metadata() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {};

            callback interface Logger {
                [Async] void flush();
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");

        let model = ComponentModel::from_ci(&ci).expect("async callback interfaces should build");
        let method = &model.callback_interfaces[0].methods[0];
        let async_callback_ffi = method
            .async_callback_ffi
            .as_ref()
            .expect("async callback method should capture ForeignFuture metadata");

        assert_eq!(
            async_callback_ffi.complete_identifier,
            "ForeignFutureCompleteVoid"
        );
        assert_eq!(
            async_callback_ffi.result_struct_identifier,
            "ForeignFutureResultVoid"
        );
        assert!(!async_callback_ffi.result_struct_has_return_value);
        assert_eq!(
            async_callback_ffi.dropped_callback_struct_identifier,
            "ForeignFutureDroppedCallbackStruct"
        );
        assert_eq!(
            async_callback_ffi.dropped_callback_identifier,
            "ForeignFutureDroppedCallback"
        );
        assert!(
            async_callback_ffi
                .default_error_return_value_expression
                .is_none()
        );
    }

    #[test]
    fn render_js_default_async_callback_return_value_expression_maps_ffi_families() {
        assert_eq!(
            render_js_default_async_callback_return_value_expression(&Type::Int32),
            "0"
        );
        assert_eq!(
            render_js_default_async_callback_return_value_expression(&Type::UInt64),
            "0"
        );
        assert_eq!(
            render_js_default_async_callback_return_value_expression(&Type::String),
            "EMPTY_RUST_BUFFER"
        );
        assert_eq!(
            render_js_default_async_callback_return_value_expression(&Type::Bytes),
            "EMPTY_RUST_BUFFER"
        );
        assert_eq!(
            render_js_default_async_callback_return_value_expression(&Type::Object {
                name: "Store".to_string(),
                module_path: "fixture_crate".to_string(),
                imp: uniffi_bindgen::interface::ObjectImpl::Struct,
            }),
            "0n"
        );
        assert_eq!(
            render_js_default_async_callback_return_value_expression(&Type::CallbackInterface {
                name: "Logger".to_string(),
                module_path: "fixture_crate".to_string(),
            }),
            "0n"
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
    fn component_model_keeps_object_method_arguments_receiverless() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {};

            interface Store {
                constructor(string prefix);
                void put(string key, string value);
            };
            "#,
            "fixture_crate",
        )
        .expect("UDL should parse");

        let object = &ci.object_definitions()[0];
        let constructor = object.constructors()[0];
        let method = object.methods()[0];
        let model = ComponentModel::from_ci(&ci).expect("component model should build");

        assert_eq!(constructor.full_arguments().len(), 1);
        assert_eq!(method.full_arguments().len(), 3);
        assert_eq!(model.objects[0].constructors[0].arguments.len(), 1);
        assert_eq!(model.objects[0].methods[0].arguments.len(), 2);
        assert_eq!(model.objects[0].methods[0].arguments[0].name, "key");
        assert_eq!(model.objects[0].methods[0].arguments[1].name, "value");
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
    fn render_public_type_maps_timestamp_to_date() {
        assert_eq!(render_public_type(&Type::Timestamp).unwrap(), "Date");
    }

    #[test]
    fn render_public_type_maps_duration_to_number() {
        assert_eq!(render_public_type(&Type::Duration).unwrap(), "number");
    }

    #[test]
    fn render_public_type_maps_nested_timestamp_shapes() {
        assert_eq!(
            render_public_type(&Type::Optional {
                inner_type: Box::new(Type::Sequence {
                    inner_type: Box::new(Type::Timestamp),
                }),
            })
            .unwrap(),
            "Array<Date> | undefined"
        );
        assert_eq!(
            render_public_type(&Type::Sequence {
                inner_type: Box::new(Type::Optional {
                    inner_type: Box::new(Type::Timestamp),
                }),
            })
            .unwrap(),
            "Array<Date | undefined>"
        );
    }

    #[test]
    fn render_public_type_maps_nested_duration_map_shapes() {
        assert_eq!(
            render_public_type(&Type::Map {
                key_type: Box::new(Type::String),
                value_type: Box::new(Type::Map {
                    key_type: Box::new(Type::String),
                    value_type: Box::new(Type::Duration),
                }),
            })
            .unwrap(),
            "Map<string, Map<string, number>>"
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
            rendered.js.contains("cloneFreeUsesUniffiHandle: true,"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("cloneHandleRawExternal(handle) {"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "[bindings.ffiTypes.VoidPointer, koffi.pointer(bindings.ffiTypes.RustCallStatus)]"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("freeHandleRawExternal(handle) {"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("const pointer = uniffiRustCaller.rustCall("),
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
    fn render_public_api_emits_temporal_converters_for_functions() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {
                timestamp echo_timestamp(timestamp when);
                duration echo_duration(duration delay_ms);
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
                .contains("export declare function echo_timestamp(when: Date): Date;"),
            "unexpected DTS output: {}",
            rendered.dts
        );
        assert!(
            rendered
                .dts
                .contains("export declare function echo_duration(delay_ms: number): number;"),
            "unexpected DTS output: {}",
            rendered.dts
        );
        assert!(
            rendered.js.contains(
                "const loweredWhen = uniffiLowerIntoRustBuffer(FfiConverterTimestamp, when);"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("return uniffiLiftFromRustBuffer(FfiConverterTimestamp, uniffiResult);"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "const loweredDelayMs = uniffiLowerIntoRustBuffer(FfiConverterDuration, delay_ms);"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("return uniffiLiftFromRustBuffer(FfiConverterDuration, uniffiResult);"),
            "unexpected JS output: {}",
            rendered.js
        );

        assert_component_js_imports_include_converters(
            &rendered.js,
            &["FfiConverterTimestamp", "FfiConverterDuration"],
        );
    }

    #[test]
    fn render_public_api_emits_temporal_converters_for_callback_interfaces() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {
                void init_clock(Clock callback);
            };

            callback interface Clock {
                duration record(timestamp when);
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
                .contains("export interface Clock {\n  record(when: Date): number;\n}"),
            "unexpected DTS output: {}",
            rendered.dts
        );
        assert!(
            rendered.js.contains(
                "args: [\n          uniffiLiftFromRustBuffer(FfiConverterTimestamp, when),\n        ],"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("const loweredReturn = uniffiLowerIntoRustBuffer(FfiConverterDuration, uniffiResult);"),
            "unexpected JS output: {}",
            rendered.js
        );

        assert_component_js_imports_include_converters(
            &rendered.js,
            &["FfiConverterTimestamp", "FfiConverterDuration"],
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
            rendered
                .js
                .contains("export class ErrorInvalid extends Error {"),
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
                .contains("const completePointer = uniffiGetCachedLibraryFunction("),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("bindings.ffiTypes.VoidPointer,"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "liftFunc: (pointer) => uniffiAsyncStoreObjectFactory.createRawExternal(pointer),"
            ),
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
                .contains("return uniffiLiftStringFromRustBuffer(uniffiResult);"),
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
                "pollFunc: (rustFuture, _continuationCallback, continuationHandle) => ffiFunctions."
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("uniffiGetRustFutureContinuationPointer(), continuationHandle)"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("const completePointer = uniffiGetCachedLibraryFunction("),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("bindings.ffiTypes.VoidPointer,"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "liftFunc: (uniffiResult) => uniffiStoreObjectFactory.createRawExternal(uniffiResult),"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
    }

    #[test]
    fn render_public_api_emits_async_constructor_object_lifting() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {};

            interface Store {
                [Name=new_async, Async] constructor();
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
                .contains("const completePointer = uniffiGetCachedLibraryFunction("),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "liftFunc: (pointer) => uniffiStoreObjectFactory.createRawExternal(pointer),"
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
                .contains(
                    "function uniffiRegisterLogCallbackVtable(bindings, registrations, vtableReferences) {"
                ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("configureRuntimeHooks({"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("const uniffiClone = koffi.register("),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("uniffi_clone: uniffiClone,"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("uniffiRustCaller.rustCall("),
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
    fn render_public_api_emits_async_callback_proxy_methods() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {
                void init_logging(LogCallback callback);
            };

            callback interface LogCallback {
                [Async] string log(string message);
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
            rendered.requires_async_rust_future_hooks,
            "async callback proxies should request Rust future hooks"
        );
        assert!(
            rendered.dts.contains(
                "export interface LogCallback {\n  log(message: string): Promise<string>;\n}"
            ),
            "unexpected DTS output: {}",
            rendered.dts
        );
        assert!(
            rendered
                .js
                .contains("class UniffiLogCallbackProxy extends UniffiObjectBase {"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("  async log(message) {"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("    return rustCallAsync({"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("rustFutureContinuationCallback"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("function uniffiGetRustFutureContinuationPointer() {"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "liftFunc: (uniffiResult) => uniffiLiftStringFromRustBuffer(uniffiResult),"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
    }

    #[test]
    fn render_public_api_emits_async_runtime_hooks_without_placeholder_converters() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {
                [Async] u32 current_generation();
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
            rendered.js.contains("configureRuntimeHooks({"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains("rustFutureContinuationCallback"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("function uniffiGetRustFutureContinuationPointer() {"),
            "unexpected JS output: {}",
            rendered.js
        );
    }

    #[test]
    fn render_public_api_emits_async_callback_vtable_signatures() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {
                void init_logging(LogCallback callback);
            };

            callback interface LogCallback {
                [Async] string log(string message);
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
                .contains("const logFutureFree = koffi.register("),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("koffi.pointer(bindings.ffiCallbacks.ForeignFutureDroppedCallback)"),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "(uniffiHandle, message, uniffiFutureCallback, uniffiCallbackData, uniffiOutDroppedCallback) => {"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains(
                    "koffi.encode(uniffiOutDroppedCallback, bindings.ffiStructs.ForeignFutureDroppedCallbackStruct, {"
                ),
            "unexpected JS output: {}",
            rendered.js
        );
    }

    #[test]
    fn render_public_api_emits_async_callback_error_lowering() {
        let ci = ComponentInterface::from_webidl(
            r#"
            namespace example {
                void init_logging(LogCallback callback);
            };

            [Error]
            interface CallbackError {
                Rejected(string message);
            };

            callback interface LogCallback {
                [Async, Throws=CallbackError] string log(string message);
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
            rendered.dts.contains(
                "export interface LogCallback {\n  log(message: string): Promise<string>;\n}"
            ),
            "unexpected DTS output: {}",
            rendered.dts
        );
        assert!(
            rendered.js.contains(
                "lowerError: (error) => error instanceof CallbackError ? uniffiLowerIntoRustBuffer(FfiConverterCallbackError, error) : null,"
            ),
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
                "const loweredChunks = uniffiLowerIntoRustBuffer(uniffiArrayConverter(FfiConverterBytes), chunks);"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "return uniffiLiftFromRustBuffer(uniffiArrayConverter(FfiConverterBytes), uniffiResult);"
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
                "const loweredValue = uniffiLowerIntoRustBuffer(uniffiOptionalConverter(FfiConverterBytes), value);"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "return uniffiLiftFromRustBuffer(uniffiOptionalConverter(FfiConverterBytes), uniffiResult);"
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
                "const loweredValue = uniffiLowerIntoRustBuffer(uniffiOptionalConverter(FfiConverterStore), value);"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered.js.contains(
                "return uniffiLiftFromRustBuffer(uniffiOptionalConverter(FfiConverterStore), uniffiResult);"
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

    #[test]
    fn render_js_lower_expression_uses_converter_for_callback_trait_objects() {
        let expression = render_js_lower_expression(
            &Type::Object {
                module_path: "crate".to_string(),
                name: "MergeOperator".to_string(),
                imp: uniffi_bindgen::interface::ObjectImpl::CallbackTrait,
            },
            "merge_operator",
        )
        .expect("callback-trait object lowering should succeed");

        assert_eq!(
            expression,
            "FfiConverterMergeOperator.lower(merge_operator)"
        );
    }

    #[test]
    fn render_js_lift_expression_uses_converter_for_callback_trait_objects() {
        let expression = render_js_lift_expression(
            &Type::Object {
                module_path: "crate".to_string(),
                name: "MergeOperator".to_string(),
                imp: uniffi_bindgen::interface::ObjectImpl::CallbackTrait,
            },
            "handle",
        )
        .expect("callback-trait object lifting should succeed");

        assert_eq!(expression, "FfiConverterMergeOperator.lift(handle)");
    }

    #[test]
    fn render_js_koffi_type_expression_keeps_buffer_and_handle_mappings() {
        assert_eq!(
            render_js_koffi_type_expression(&Type::String, "bindings").unwrap(),
            "bindings.ffiTypes.RustBuffer"
        );
        assert_eq!(
            render_js_koffi_type_expression(
                &Type::Object {
                    module_path: "crate".to_string(),
                    name: "Store".to_string(),
                    imp: uniffi_bindgen::interface::ObjectImpl::Struct,
                },
                "bindings",
            )
            .unwrap(),
            "bindings.ffiTypes.UniffiHandle"
        );
        assert_eq!(
            render_js_koffi_type_expression(
                &Type::CallbackInterface {
                    module_path: "crate".to_string(),
                    name: "Logger".to_string(),
                },
                "bindings",
            )
            .unwrap(),
            "\"uint64_t\""
        );
    }
}
