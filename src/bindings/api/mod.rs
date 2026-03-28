mod model;
mod render;
mod support;

use anyhow::{Context, Result};
use uniffi_bindgen::interface::Type;

pub(crate) use self::model::{
    ArgumentModel, CallbackInterfaceModel, ComponentModel, ConstructorModel, EnumModel,
    ErrorModel, FieldModel, FunctionModel, MethodModel, ObjectModel, RecordModel,
    RenderedComponentApi,
};
pub(crate) use self::support::*;

impl ComponentModel {
    pub(crate) fn render_public_api(&self) -> Result<RenderedComponentApi> {
        let mut js_sections = Vec::new();
        let mut dts_sections = Vec::new();
        let requires_async_rust_future_hooks = self.requires_async_rust_future_hooks();

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

        if requires_async_rust_future_hooks {
            js_sections.push(render_js_async_rust_future_helpers());
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

        if !self.callback_interfaces.is_empty() || requires_async_rust_future_hooks {
            js_sections.push(render_js_runtime_hooks(
                &self.callback_interfaces,
                requires_async_rust_future_hooks,
            )?);
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
            requires_async_rust_future_hooks,
        })
    }

    fn has_placeholder_converters(&self) -> bool {
        !self.records.is_empty()
            || !self.flat_enums.is_empty()
            || !self.tagged_enums.is_empty()
            || !self.errors.is_empty()
            || !self.callback_interfaces.is_empty()
    }

    fn requires_async_rust_future_hooks(&self) -> bool {
        self.functions.iter().any(|function| function.is_async)
            || self.objects.iter().any(|object| {
                object
                    .constructors
                    .iter()
                    .any(|constructor| constructor.is_async)
                    || object.methods.iter().any(|method| method.is_async)
            })
            || self.callback_interfaces.iter().any(|callback_interface| {
                callback_interface
                    .methods
                    .iter()
                    .any(|method| method.is_async)
            })
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
            "  {}{}({}) {{",
            if method.is_async { "async " } else { "" },
            js_member_identifier(&method.name),
            render_js_params(&method.arguments)
        ));
        if method.is_async {
            lines.extend(render_js_async_method_body(
                method,
                &factory_name,
                &callback_interface.name,
            )?);
        } else {
            lines.extend(render_js_sync_method_body(
                method,
                &factory_name,
                &callback_interface.name,
            )?);
        }
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
    lines.push("    return uniffiRustCaller.rustCall(".to_string());
    lines.push(format!(
        "      (status) => ffiFunctions.{}(handle, status),",
        callback_interface.ffi_object_clone_identifier
    ));
    lines.push("      uniffiRustCallOptions(),".to_string());
    lines.push("    );".to_string());
    lines.push("  },".to_string());
    lines.push("  freeHandle(handle) {".to_string());
    lines.push("    uniffiRustCaller.rustCall(".to_string());
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
        "function {}(bindings, registrations, vtableReferences) {{",
        register_name
    ));

    for method in &callback_interface.methods {
        lines.extend(render_js_callback_vtable_registration(
            callback_interface,
            method,
            &registry_name,
        )?);
    }

    lines.push("  const uniffiFree = koffi.register(".to_string());
    lines.push(format!(
        "    (uniffiHandle) => {}.remove(uniffiHandle),",
        registry_name
    ));
    lines.push("    koffi.pointer(bindings.ffiCallbacks.CallbackInterfaceFree),".to_string());
    lines.push("  );".to_string());
    lines.push("  registrations.push(uniffiFree);".to_string());
    let vtable_struct_name = callback_interface_vtable_struct_name(&callback_interface.name);
    lines.push(format!(
        "  const uniffiVtable = koffi.alloc(bindings.ffiStructs.{}, 1);",
        vtable_struct_name
    ));
    lines.push(format!(
        "  koffi.encode(uniffiVtable, bindings.ffiStructs.{}, {{",
        vtable_struct_name
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
    lines.push("  vtableReferences.push(uniffiVtable);".to_string());
    lines.push(format!(
        "  bindings.ffiFunctions.{}(uniffiVtable);",
        callback_interface.ffi_init_callback_identifier
    ));
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

fn render_js_callback_vtable_registration(
    callback_interface: &CallbackInterfaceModel,
    method: &MethodModel,
    registry_name: &str,
) -> Result<Vec<String>> {
    if method.is_async {
        render_js_async_callback_vtable_registration(callback_interface, method, registry_name)
    } else {
        render_js_sync_callback_vtable_registration(callback_interface, method, registry_name)
    }
}

fn render_js_sync_callback_vtable_registration(
    callback_interface: &CallbackInterfaceModel,
    method: &MethodModel,
    registry_name: &str,
) -> Result<Vec<String>> {
    let callback_identifier = method.ffi_callback_identifier.as_deref().with_context(|| {
        format!(
            "callback interface {}.{} is missing an FFI callback identifier",
            callback_interface.name, method.name
        )
    })?;
    let mut lines = vec![format!(
        "  const {}Callback = koffi.register(",
        js_member_identifier(&method.name)
    )];
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
    lines.push("        lowerString: (value) => uniffiLowerString(value),".to_string());
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
    Ok(lines)
}

fn render_js_async_callback_vtable_registration(
    callback_interface: &CallbackInterfaceModel,
    method: &MethodModel,
    registry_name: &str,
) -> Result<Vec<String>> {
    let callback_identifier = method.ffi_callback_identifier.as_deref().with_context(|| {
        format!(
            "callback interface {}.{} is missing an FFI callback identifier",
            callback_interface.name, method.name
        )
    })?;
    let async_callback_ffi = method.async_callback_ffi.as_ref().with_context(|| {
        format!(
            "async callback interface {}.{} is missing ForeignFuture metadata",
            callback_interface.name, method.name
        )
    })?;
    let method_identifier = js_identifier(&method.name);
    let future_free_identifier = format!("{method_identifier}FutureFree");
    let mut lines = vec![format!(
        "  const {} = koffi.register(",
        future_free_identifier
    )];
    lines.push("    (uniffiFutureHandle) => {".to_string());
    lines.push("      freePendingForeignFuture(uniffiFutureHandle);".to_string());
    lines.push("    },".to_string());
    lines.push("    koffi.pointer(bindings.ffiCallbacks.ForeignFutureFree),".to_string());
    lines.push("  );".to_string());
    lines.push(format!("  registrations.push({});", future_free_identifier));
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
                .chain(
                    [
                        "uniffiFutureCallback".to_string(),
                        "uniffiCallbackData".to_string(),
                        "uniffiOutReturn".to_string(),
                    ]
                    .into_iter(),
                )
                .collect::<Vec<_>>()
                .join(", ")
            + ") => {",
    );
    lines.push("      const uniffiFutureHandle = invokeAsyncCallbackMethod({".to_string());
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
    lines.push(format!(
        "        complete: (callbackData, result) => koffi.call(uniffiFutureCallback, bindings.ffiCallbacks.{}, callbackData, result),",
        async_callback_ffi.complete_identifier
    ));
    lines.push("        callbackData: uniffiCallbackData,".to_string());
    if let Some(throws_type) = method.throws_type.as_ref() {
        lines.push(format!(
            "        lowerError: (error) => error instanceof {} ? uniffiLowerIntoRustBuffer({}, error) : null,",
            render_public_type(throws_type)?,
            render_js_type_converter_expression(throws_type)?
        ));
    }
    if async_callback_ffi.result_struct_has_return_value {
        let return_type = method.return_type.as_ref().with_context(|| {
            format!(
                "async callback interface {}.{} is missing a return type",
                callback_interface.name, method.name
            )
        })?;
        lines.push(format!(
            "        lowerReturn: (value) => {},",
            render_js_lower_expression(return_type, "value")?
        ));
        let default_return_value = async_callback_ffi
            .default_error_return_value_expression
            .as_deref()
            .with_context(|| {
                format!(
                    "async callback interface {}.{} is missing a default error return value",
                    callback_interface.name, method.name
                )
            })?;
        lines.push(format!(
            "        defaultReturnValue: {},",
            default_return_value
        ));
    }
    lines.push("        lowerString: (value) => uniffiLowerString(value),".to_string());
    lines.push("      });".to_string());
    lines.push(
        "      koffi.encode(uniffiOutReturn, bindings.ffiStructs.ForeignFuture, {".to_string(),
    );
    lines.push("        handle: uniffiFutureHandle,".to_string());
    lines.push(format!("        free: {},", future_free_identifier));
    lines.push("      });".to_string());
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
    Ok(lines)
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
        format!(
            "export declare class {} extends globalThis.Error {{",
            error.name
        ),
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
    lines.push("  cloneHandleGeneric(handle) {".to_string());
    lines.push("    const bindings = getFfiBindings();".to_string());
    lines.push("    const genericCloneHandle = bindings.library.func(".to_string());
    lines.push(format!(
        "      {},",
        json_string_literal(&object.ffi_object_clone_identifier)?
    ));
    lines.push("      bindings.ffiTypes.RustArcPtr,".to_string());
    lines.push(
        "      [bindings.ffiTypes.RustArcPtr, koffi.pointer(bindings.ffiTypes.RustCallStatus)],"
            .to_string(),
    );
    lines.push("    );".to_string());
    lines.push("    return uniffiRustCaller.rustCall(".to_string());
    lines.push("      (status) => genericCloneHandle(handle, status),".to_string());
    lines.push("      uniffiRustCallOptions(),".to_string());
    lines.push("    );".to_string());
    lines.push("  },".to_string());
    lines.push("  cloneHandleRawExternal(handle) {".to_string());
    lines.push("    const bindings = getFfiBindings();".to_string());
    lines.push("    const rawExternalCloneHandle = bindings.library.func(".to_string());
    lines.push(format!(
        "      {},",
        json_string_literal(&object.ffi_object_clone_identifier)?
    ));
    lines.push("      bindings.ffiTypes.VoidPointer,".to_string());
    lines.push(
        "      [bindings.ffiTypes.VoidPointer, koffi.pointer(bindings.ffiTypes.RustCallStatus)],"
            .to_string(),
    );
    lines.push("    );".to_string());
    lines.push("    return uniffiRustCaller.rustCall(".to_string());
    lines.push("      (status) => rawExternalCloneHandle(handle, status),".to_string());
    lines.push("      uniffiRustCallOptions(),".to_string());
    lines.push("    );".to_string());
    lines.push("  },".to_string());
    lines.push("  cloneHandle(handle) {".to_string());
    lines.push("    return uniffiRustCaller.rustCall(".to_string());
    lines.push(format!(
        "      (status) => ffiFunctions.{}(handle, status),",
        object.ffi_object_clone_identifier
    ));
    lines.push("      uniffiRustCallOptions(),".to_string());
    lines.push("    );".to_string());
    lines.push("  },".to_string());
    lines.push("  freeHandleGeneric(handle) {".to_string());
    lines.push("    uniffiRustCaller.rustCall(".to_string());
    lines.push(format!(
        "      (status) => ffiFunctions.{}_generic_abi(handle, status),",
        object.ffi_object_free_identifier
    ));
    lines.push("      uniffiRustCallOptions(),".to_string());
    lines.push("    );".to_string());
    lines.push("  },".to_string());
    lines.push("  freeHandleRawExternal(handle) {".to_string());
    lines.push("    const bindings = getFfiBindings();".to_string());
    lines.push("    const rawExternalFreeHandle = bindings.library.func(".to_string());
    lines.push(format!(
        "      {},",
        json_string_literal(&object.ffi_object_free_identifier)?
    ));
    lines.push("      \"void\",".to_string());
    lines.push(
        "      [bindings.ffiTypes.VoidPointer, koffi.pointer(bindings.ffiTypes.RustCallStatus)],"
            .to_string(),
    );
    lines.push("    );".to_string());
    lines.push("    uniffiRustCaller.rustCall(".to_string());
    lines.push("      (status) => rawExternalFreeHandle(handle, status),".to_string());
    lines.push("      uniffiRustCallOptions(),".to_string());
    lines.push("    );".to_string());
    lines.push("  },".to_string());
    lines.push("  freeHandle(handle) {".to_string());
    lines.push("    uniffiRustCaller.rustCall(".to_string());
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

fn render_js_runtime_helpers(
    ffi_rustbuffer_from_bytes_identifier: &str,
    ffi_rustbuffer_free_identifier: &str,
) -> String {
    format!(
        "const uniffiTextEncoder = new TextEncoder();\nconst uniffiTextDecoder = new TextDecoder();\n\nfunction uniffiLiftString(bytes) {{\n  return uniffiTextDecoder.decode(bytes);\n}}\n\nfunction uniffiDecodeRustCallStatus(status) {{\n  return status == null\n    ? createRustCallStatus()\n    : koffi.decode(status, getFfiBindings().ffiTypes.RustCallStatus);\n}}\n\nfunction uniffiWriteRustCallStatus(status, value) {{\n  if (status != null) {{\n    koffi.encode(status, getFfiBindings().ffiTypes.RustCallStatus, value);\n  }}\n  return status;\n}}\n\nconst uniffiRustCaller = new UniffiRustCaller({{\n  createStatus: () => koffi.alloc(getFfiBindings().ffiTypes.RustCallStatus, 1),\n  readStatus: uniffiDecodeRustCallStatus,\n  writeStatus: uniffiWriteRustCallStatus,\n  liftString: uniffiLiftString,\n}});\n\nfunction uniffiFreeRustBuffer(buffer) {{\n  return uniffiRustCaller.rustCall(\n    (status) => ffiFunctions.{ffi_rustbuffer_free_identifier}(buffer, status),\n    {{ liftString: uniffiLiftString }},\n  );\n}}\n\nfunction uniffiRustCallOptions(errorConverter = undefined) {{\n  const options = {{\n    freeRustBuffer: uniffiFreeRustBuffer,\n    liftString: uniffiLiftString,\n    rustCaller: uniffiRustCaller,\n  }};\n  if (errorConverter != null) {{\n    options.errorHandler = (errorBytes) => errorConverter.lift(errorBytes);\n  }}\n  return options;\n}}\n\nfunction uniffiCopyIntoRustBuffer(bytes) {{\n  return uniffiRustCaller.rustCall(\n    (status) => ffiFunctions.{ffi_rustbuffer_from_bytes_identifier}(createForeignBytes(bytes), status),\n    uniffiRustCallOptions(),\n  );\n}}\n\nfunction uniffiLowerString(value) {{\n  return uniffiCopyIntoRustBuffer(uniffiTextEncoder.encode(value));\n}}\n\nfunction uniffiLiftStringFromRustBuffer(value) {{\n  return uniffiLiftString(new RustBufferValue(value).consumeIntoUint8Array(uniffiFreeRustBuffer));\n}}\n\nfunction uniffiLowerBytes(value) {{\n  return uniffiCopyIntoRustBuffer(value);\n}}\n\nfunction uniffiLiftBytesFromRustBuffer(value) {{\n  return new RustBufferValue(value).consumeIntoUint8Array(uniffiFreeRustBuffer);\n}}\n\nfunction uniffiLowerIntoRustBuffer(converter, value) {{\n  return uniffiCopyIntoRustBuffer(converter.lower(value));\n}}\n\nfunction uniffiLiftFromRustBuffer(converter, value) {{\n  return converter.lift(uniffiLiftBytesFromRustBuffer(value));\n}}\n\nfunction uniffiRequireRecordObject(typeName, value) {{\n  if (typeof value !== \"object\" || value == null) {{\n    throw new TypeError(`${{typeName}} values must be non-null objects.`);\n  }}\n  return value;\n}}\n\nfunction uniffiRequireFlatEnumValue(enumValues, typeName, value) {{\n  for (const enumValue of Object.values(enumValues)) {{\n    if (enumValue === value) {{\n      return enumValue;\n    }}\n  }}\n  throw new TypeError(`${{typeName}} values must be one of ${{Object.values(enumValues).map((item) => JSON.stringify(item)).join(\", \")}}.`);\n}}\n\nfunction uniffiRequireTaggedEnumValue(typeName, value) {{\n  const enumValue = uniffiRequireRecordObject(typeName, value);\n  if (typeof enumValue.tag !== \"string\") {{\n    throw new TypeError(`${{typeName}} values must be tagged objects with a string tag field.`);\n  }}\n  return enumValue;\n}}\n\nfunction uniffiNotImplementedConverter(typeName) {{\n  const fail = (member) => {{\n    throw new Error(`${{typeName}} converter ${{member}} is not implemented yet.`);\n  }};\n  return Object.freeze({{\n    lower() {{\n      return fail(\"lower\");\n    }},\n    lift() {{\n      return fail(\"lift\");\n    }},\n    write() {{\n      return fail(\"write\");\n    }},\n    read() {{\n      return fail(\"read\");\n    }},\n    allocationSize() {{\n      return fail(\"allocationSize\");\n    }},\n  }});\n}}"
    )
}

fn render_js_async_rust_future_helpers() -> String {
    "let uniffiRustFutureContinuationPointer = null;\n\nfunction uniffiGetRustFutureContinuationPointer() {\n  if (uniffiRustFutureContinuationPointer == null) {\n    const bindings = getFfiBindings();\n    uniffiRustFutureContinuationPointer = koffi.register(\n      rustFutureContinuationCallback,\n      koffi.pointer(bindings.ffiCallbacks.RustFutureContinuationCallback),\n    );\n  }\n  return uniffiRustFutureContinuationPointer;\n}"
        .to_string()
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
    lines.push("    const pointer = uniffiRustCaller.rustCall(".to_string());
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
    lines.extend(render_js_async_object_complete_setup(
        &async_ffi.complete_identifier,
        "    ",
    )?);
    lines.push("    return rustCallAsync({".to_string());
    lines.push(format!(
        "      rustFutureFunc: () => ffiFunctions.{}({}),",
        constructor.ffi_func_identifier, start_args
    ));
    lines.push(format!(
        "      pollFunc: (rustFuture, _continuationCallback, continuationHandle) => ffiFunctions.{}(rustFuture, uniffiGetRustFutureContinuationPointer(), continuationHandle),",
        async_ffi.poll_identifier
    ));
    lines.push(format!(
        "      cancelFunc: (rustFuture) => ffiFunctions.{}(rustFuture),",
        async_ffi.cancel_identifier
    ));
    lines.push(format!(
        "      freeFunc: (rustFuture) => ffiFunctions.{}(rustFuture),",
        async_ffi.free_identifier
    ));
    lines.push("      completeFunc,".to_string());
    lines.push(format!(
        "      liftFunc: (pointer) => {}.createRawExternal(pointer),",
        factory_name
    ));
    lines.push(format!(
        "      ...{},",
        render_js_rust_call_options_expression(constructor.throws_type.as_ref())?
    ));
    lines.push("    });".to_string());
    Ok(lines)
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
    lines.push("    const ffiMethod =".to_string());
    lines.push(format!("      {}.usesGenericAbi(this)", factory_name));
    lines.push(format!(
        "        ? ffiFunctions.{}_generic_abi",
        method.ffi_func_identifier
    ));
    lines.push(format!(
        "        : ffiFunctions.{};",
        method.ffi_func_identifier
    ));
    lines.extend(render_js_argument_lowering(&method.arguments)?);
    let call_args = render_js_ffi_call_args_with_leading(
        &[String::from("loweredSelf")],
        &method.arguments,
        Some("status"),
    );

    if let Some(return_type) = method.return_type.as_ref() {
        lines.push("    const uniffiResult = uniffiRustCaller.rustCall(".to_string());
        lines.push(format!("      (status) => ffiMethod({}),", call_args));
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
        lines.push("    uniffiRustCaller.rustCall(".to_string());
        lines.push(format!("      (status) => ffiMethod({}),", call_args));
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
    lines.push("    const ffiMethod =".to_string());
    lines.push(format!("      {}.usesGenericAbi(this)", factory_name));
    lines.push(format!(
        "        ? ffiFunctions.{}_generic_abi",
        method.ffi_func_identifier
    ));
    lines.push(format!(
        "        : ffiFunctions.{};",
        method.ffi_func_identifier
    ));
    lines.extend(render_js_argument_lowering(&method.arguments)?);
    let start_args = render_js_ffi_call_args_with_leading(
        &[String::from("loweredSelf")],
        &method.arguments,
        None,
    );
    lines.extend(render_js_async_complete_setup(
        method.return_type.as_ref(),
        &async_ffi.complete_identifier,
        "    ",
    )?);

    lines.push("    return rustCallAsync({".to_string());
    lines.push(format!(
        "      rustFutureFunc: () => ffiMethod({}),",
        start_args
    ));
    lines.push(format!(
        "      pollFunc: (rustFuture, _continuationCallback, continuationHandle) => ffiFunctions.{}(rustFuture, uniffiGetRustFutureContinuationPointer(), continuationHandle),",
        async_ffi.poll_identifier
    ));
    lines.push(format!(
        "      cancelFunc: (rustFuture) => ffiFunctions.{}(rustFuture),",
        async_ffi.cancel_identifier
    ));
    lines.push("      completeFunc,".to_string());
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

fn render_js_runtime_hooks(
    callback_interfaces: &[CallbackInterfaceModel],
    needs_async_rust_future_hooks: bool,
) -> Result<String> {
    let mut lines = vec![
        "const uniffiRegisteredCallbackPointers = [];".to_string(),
        "const uniffiRegisteredCallbackVtables = [];".to_string(),
        String::new(),
        "function uniffiRegisterCallbackVtables(bindings) {".to_string(),
    ];

    for callback_interface in callback_interfaces {
        lines.push(format!(
            "  {}(bindings, uniffiRegisteredCallbackPointers, uniffiRegisteredCallbackVtables);",
            callback_interface_register_name(&callback_interface.name)
        ));
    }
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push("function uniffiUnregisterCallbackVtables() {".to_string());
    lines.push("  clearPendingForeignFutures();".to_string());
    for callback_interface in callback_interfaces {
        lines.push(format!(
            "  {}.clear();",
            callback_interface_registry_name(&callback_interface.name)
        ));
    }
    lines.push("  while (uniffiRegisteredCallbackPointers.length > 0) {".to_string());
    lines.push("    koffi.unregister(uniffiRegisteredCallbackPointers.pop());".to_string());
    lines.push("  }".to_string());
    lines.push("  uniffiRegisteredCallbackVtables.length = 0;".to_string());
    lines.push("}".to_string());
    lines.push(String::new());
    lines.push("configureRuntimeHooks({".to_string());
    lines.push("  onLoad(bindings) {".to_string());
    if callback_interfaces.is_empty() {
        lines.push("    void bindings;".to_string());
    } else {
        lines.push("    uniffiRegisterCallbackVtables(bindings);".to_string());
    }
    lines.push("  },".to_string());
    lines.push("  onUnload() {".to_string());
    if needs_async_rust_future_hooks {
        lines.push("    if (uniffiRustFutureContinuationPointer != null) {".to_string());
        lines.push("      koffi.unregister(uniffiRustFutureContinuationPointer);".to_string());
        lines.push("      uniffiRustFutureContinuationPointer = null;".to_string());
        lines.push("    }".to_string());
    }
    lines.push("    uniffiUnregisterCallbackVtables();".to_string());
    lines.push("  },".to_string());
    lines.push("});".to_string());

    Ok(lines.join("\n"))
}

fn render_js_sync_function_body(function: &FunctionModel) -> Result<Vec<String>> {
    let mut lines = render_js_argument_lowering(&function.arguments)?;
    let call_args = render_js_ffi_call_args(&function.arguments, Some("status"));

    if let Some(return_type) = function.return_type.as_ref() {
        lines.push("  const uniffiResult = uniffiRustCaller.rustCall(".to_string());
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
        lines.push("  uniffiRustCaller.rustCall(".to_string());
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
    lines.extend(render_js_async_complete_setup(
        function.return_type.as_ref(),
        &async_ffi.complete_identifier,
        "  ",
    )?);

    lines.push("  return rustCallAsync({".to_string());
    lines.push(format!(
        "    rustFutureFunc: () => ffiFunctions.{}({}),",
        function.ffi_func_identifier, start_args
    ));
    lines.push(format!(
        "    pollFunc: (rustFuture, _continuationCallback, continuationHandle) => ffiFunctions.{}(rustFuture, uniffiGetRustFutureContinuationPointer(), continuationHandle),",
        async_ffi.poll_identifier
    ));
    lines.push(format!(
        "    cancelFunc: (rustFuture) => ffiFunctions.{}(rustFuture),",
        async_ffi.cancel_identifier
    ));
    lines.push("    completeFunc,".to_string());
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

#[cfg(test)]
mod tests {
    use super::*;
    use uniffi_bindgen::interface::ComponentInterface;
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
            "ForeignFutureStructRustBuffer"
        );
        assert!(async_callback_ffi.result_struct_has_return_value);
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
            "ForeignFutureStructVoid"
        );
        assert!(!async_callback_ffi.result_struct_has_return_value);
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
                .contains("const completePointer = bindings.library.func("),
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
                .contains("const completePointer = bindings.library.func("),
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
                .contains("const completePointer = bindings.library.func("),
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
            rendered.js.contains(
                "(uniffiHandle, message, uniffiFutureCallback, uniffiCallbackData, uniffiOutReturn) => {"
            ),
            "unexpected JS output: {}",
            rendered.js
        );
        assert!(
            rendered
                .js
                .contains("koffi.encode(uniffiOutReturn, bindings.ffiStructs.ForeignFuture, {"),
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
}
