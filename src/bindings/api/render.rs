use anyhow::Result;
use askama::Template;

use super::model::VariantModel;
use super::{
    CallbackInterfaceModel, ComponentModel, ConstructorModel, EnumModel, ErrorModel, FieldModel,
    FunctionModel, MethodModel, ObjectModel, RecordModel, ffi_opaque_identifier, js_identifier,
    js_member_identifier, json_string_literal, object_converter_name, object_factory_name,
    quoted_property_name, render_dts_fields_as_params, render_dts_params,
    render_js_fields_as_params, render_js_function_body_lines,
    render_js_object_constructor_body_lines, render_js_object_method_body_lines, render_js_params,
    render_js_primary_constructor_body_lines, render_named_return_type, render_public_type,
    render_return_type, variant_type_name,
};

#[derive(Default)]
pub(crate) struct JsRenderSections {
    pub(crate) unimplemented_helper: String,
    pub(crate) runtime_helpers: String,
    pub(crate) async_rust_future_helpers: String,
    pub(crate) flat_enums: Vec<String>,
    pub(crate) tagged_enums: Vec<String>,
    pub(crate) errors: Vec<String>,
    pub(crate) placeholder_converters: String,
    pub(crate) runtime_hooks: String,
    pub(crate) functions: Vec<String>,
    pub(crate) objects: Vec<String>,
}

impl JsRenderSections {
    fn has_unimplemented_helper(&self) -> bool {
        !self.unimplemented_helper.is_empty()
    }

    fn has_runtime_helpers(&self) -> bool {
        !self.runtime_helpers.is_empty()
    }

    fn has_async_rust_future_helpers(&self) -> bool {
        !self.async_rust_future_helpers.is_empty()
    }

    fn has_placeholder_converters(&self) -> bool {
        !self.placeholder_converters.is_empty()
    }

    fn has_runtime_hooks(&self) -> bool {
        !self.runtime_hooks.is_empty()
    }

    fn has_sections_after_unimplemented_helper(&self) -> bool {
        self.has_runtime_helpers()
            || self.has_async_rust_future_helpers()
            || !self.flat_enums.is_empty()
            || !self.tagged_enums.is_empty()
            || !self.errors.is_empty()
            || self.has_placeholder_converters()
            || self.has_runtime_hooks()
            || !self.functions.is_empty()
            || !self.objects.is_empty()
    }

    fn has_sections_after_runtime_helpers(&self) -> bool {
        self.has_async_rust_future_helpers()
            || !self.flat_enums.is_empty()
            || !self.tagged_enums.is_empty()
            || !self.errors.is_empty()
            || self.has_placeholder_converters()
            || self.has_runtime_hooks()
            || !self.functions.is_empty()
            || !self.objects.is_empty()
    }

    fn has_sections_after_async_rust_future_helpers(&self) -> bool {
        !self.flat_enums.is_empty()
            || !self.tagged_enums.is_empty()
            || !self.errors.is_empty()
            || self.has_placeholder_converters()
            || self.has_runtime_hooks()
            || !self.functions.is_empty()
            || !self.objects.is_empty()
    }

    fn has_sections_after_flat_enums(&self) -> bool {
        !self.tagged_enums.is_empty()
            || !self.errors.is_empty()
            || self.has_placeholder_converters()
            || self.has_runtime_hooks()
            || !self.functions.is_empty()
            || !self.objects.is_empty()
    }

    fn has_sections_after_tagged_enums(&self) -> bool {
        !self.errors.is_empty()
            || self.has_placeholder_converters()
            || self.has_runtime_hooks()
            || !self.functions.is_empty()
            || !self.objects.is_empty()
    }

    fn has_sections_after_errors(&self) -> bool {
        self.has_placeholder_converters()
            || self.has_runtime_hooks()
            || !self.functions.is_empty()
            || !self.objects.is_empty()
    }

    fn has_sections_after_placeholder_converters(&self) -> bool {
        self.has_runtime_hooks() || !self.functions.is_empty() || !self.objects.is_empty()
    }

    fn has_sections_after_runtime_hooks(&self) -> bool {
        !self.functions.is_empty() || !self.objects.is_empty()
    }

    fn has_sections_after_functions(&self) -> bool {
        !self.objects.is_empty()
    }
}

pub(crate) struct PublicApiRenderer<'a> {
    model: &'a ComponentModel,
}

impl<'a> PublicApiRenderer<'a> {
    pub(crate) fn new(model: &'a ComponentModel) -> Self {
        Self { model }
    }

    pub(crate) fn render_js(&self, sections: JsRenderSections) -> Result<String> {
        let _ = self.model;
        Ok(PublicApiJsTemplate { renderer: sections }.render()?)
    }

    pub(crate) fn render_dts(&self) -> Result<String> {
        Ok(PublicApiDtsTemplate {
            renderer: DtsRenderer::new(self.model)?,
        }
        .render()?)
    }
}

struct DtsRenderer {
    records: Vec<String>,
    flat_enums: Vec<String>,
    tagged_enums: Vec<String>,
    errors: Vec<String>,
    callback_interfaces: Vec<String>,
    functions: Vec<String>,
    objects: Vec<String>,
}

impl DtsRenderer {
    fn new(model: &ComponentModel) -> Result<Self> {
        Ok(Self {
            records: model
                .records
                .iter()
                .map(render_dts_record_fragment)
                .collect::<Result<_>>()?,
            flat_enums: model
                .flat_enums
                .iter()
                .map(render_dts_flat_enum_fragment)
                .collect::<Result<_>>()?,
            tagged_enums: model
                .tagged_enums
                .iter()
                .map(render_dts_tagged_enum_fragment)
                .collect::<Result<_>>()?,
            errors: model
                .errors
                .iter()
                .map(render_dts_error_fragment)
                .collect::<Result<_>>()?,
            callback_interfaces: model
                .callback_interfaces
                .iter()
                .map(render_dts_callback_interface_fragment)
                .collect::<Result<_>>()?,
            functions: model
                .functions
                .iter()
                .map(render_dts_function_fragment)
                .collect::<Result<_>>()?,
            objects: model
                .objects
                .iter()
                .map(render_dts_object_fragment)
                .collect::<Result<_>>()?,
        })
    }

    fn has_declarations_after_records(&self) -> bool {
        !self.flat_enums.is_empty()
            || !self.tagged_enums.is_empty()
            || !self.errors.is_empty()
            || !self.callback_interfaces.is_empty()
            || !self.functions.is_empty()
            || !self.objects.is_empty()
    }

    fn has_declarations_after_flat_enums(&self) -> bool {
        !self.tagged_enums.is_empty()
            || !self.errors.is_empty()
            || !self.callback_interfaces.is_empty()
            || !self.functions.is_empty()
            || !self.objects.is_empty()
    }

    fn has_declarations_after_tagged_enums(&self) -> bool {
        !self.errors.is_empty()
            || !self.callback_interfaces.is_empty()
            || !self.functions.is_empty()
            || !self.objects.is_empty()
    }

    fn has_declarations_after_errors(&self) -> bool {
        !self.callback_interfaces.is_empty()
            || !self.functions.is_empty()
            || !self.objects.is_empty()
    }

    fn has_declarations_after_callback_interfaces(&self) -> bool {
        !self.functions.is_empty() || !self.objects.is_empty()
    }

    fn has_declarations_after_functions(&self) -> bool {
        !self.objects.is_empty()
    }
}

struct FunctionJsView {
    name: String,
    is_async: bool,
    params: String,
    body_lines: Vec<String>,
}

impl FunctionJsView {
    fn from_function(function: &FunctionModel) -> Result<Self> {
        Ok(Self {
            name: js_identifier(&function.name),
            is_async: function.is_async,
            params: render_js_params(&function.arguments),
            body_lines: render_js_function_body_lines(function)?,
        })
    }
}

pub(crate) fn render_js_function_fragment(function: &FunctionModel) -> Result<String> {
    Ok(JsFunctionTemplate {
        function: FunctionJsView::from_function(function)?,
    }
    .render()?
    .trim_end()
    .to_string())
}

struct ObjectJsView {
    name: String,
    factory_name: String,
    converter_name: String,
    type_name_literal: String,
    ffi_opaque_identifier: String,
    ffi_object_clone_symbol: String,
    ffi_object_clone_identifier: String,
    ffi_object_free_symbol: String,
    ffi_object_free_identifier: String,
    has_primary_constructor: bool,
    primary_constructor_params: String,
    primary_constructor_body_lines: Vec<String>,
    unimplemented_constructor_member: String,
    constructors: Vec<ConstructorJsView>,
    methods: Vec<ObjectMethodJsView>,
}

impl ObjectJsView {
    fn from_object(object: &ObjectModel) -> Result<Self> {
        let factory_name = object_factory_name(&object.name);
        let primary_constructor = object
            .constructors
            .iter()
            .find(|constructor| constructor.is_primary && !constructor.is_async);

        Ok(Self {
            name: object.name.clone(),
            factory_name: factory_name.clone(),
            converter_name: object_converter_name(&object.name),
            type_name_literal: json_string_literal(&object.name)?,
            ffi_opaque_identifier: ffi_opaque_identifier(&object.name),
            ffi_object_clone_symbol: json_string_literal(&object.ffi_object_clone_identifier)?,
            ffi_object_clone_identifier: object.ffi_object_clone_identifier.clone(),
            ffi_object_free_symbol: json_string_literal(&object.ffi_object_free_identifier)?,
            ffi_object_free_identifier: object.ffi_object_free_identifier.clone(),
            has_primary_constructor: primary_constructor.is_some(),
            primary_constructor_params: primary_constructor
                .map(|constructor| render_js_params(&constructor.arguments))
                .unwrap_or_default(),
            primary_constructor_body_lines: primary_constructor
                .map(|constructor| {
                    render_js_primary_constructor_body_lines(constructor, &factory_name)
                })
                .transpose()?
                .unwrap_or_default(),
            unimplemented_constructor_member: json_string_literal(&format!(
                "{}.constructor",
                object.name
            ))?,
            constructors: object
                .constructors
                .iter()
                .filter(|constructor| !(constructor.is_primary && !constructor.is_async))
                .map(|constructor| ConstructorJsView::from_constructor(object, constructor))
                .collect::<Result<_>>()?,
            methods: object
                .methods
                .iter()
                .map(|method| ObjectMethodJsView::from_method(object, method))
                .collect::<Result<_>>()?,
        })
    }
}

struct ConstructorJsView {
    name: String,
    is_async: bool,
    params: String,
    body_lines: Vec<String>,
}

impl ConstructorJsView {
    fn from_constructor(object: &ObjectModel, constructor: &ConstructorModel) -> Result<Self> {
        let factory_name = object_factory_name(&object.name);

        Ok(Self {
            name: js_member_identifier(&constructor.name),
            is_async: constructor.is_async,
            params: render_js_params(&constructor.arguments),
            body_lines: render_js_object_constructor_body_lines(
                constructor,
                &factory_name,
                &object.name,
            )?,
        })
    }
}

struct ObjectMethodJsView {
    name: String,
    is_async: bool,
    params: String,
    body_lines: Vec<String>,
}

impl ObjectMethodJsView {
    fn from_method(object: &ObjectModel, method: &MethodModel) -> Result<Self> {
        let factory_name = object_factory_name(&object.name);

        Ok(Self {
            name: js_member_identifier(&method.name),
            is_async: method.is_async,
            params: render_js_params(&method.arguments),
            body_lines: render_js_object_method_body_lines(method, &factory_name, &object.name)?,
        })
    }
}

pub(crate) fn render_js_object_fragment(object: &ObjectModel) -> Result<String> {
    Ok(JsObjectTemplate {
        object: ObjectJsView::from_object(object)?,
    }
    .render()?
    .trim_end()
    .to_string())
}

struct FlatEnumJsView {
    name: String,
    variants: Vec<FlatEnumVariantJsView>,
}

impl FlatEnumJsView {
    fn from_enum(enum_def: &EnumModel) -> Result<Self> {
        Ok(Self {
            name: enum_def.name.clone(),
            variants: enum_def
                .variants
                .iter()
                .map(FlatEnumVariantJsView::from_variant)
                .collect::<Result<_>>()?,
        })
    }
}

struct FlatEnumVariantJsView {
    property_name: String,
    value_literal: String,
}

impl FlatEnumVariantJsView {
    fn from_variant(variant: &VariantModel) -> Result<Self> {
        Ok(Self {
            property_name: quoted_property_name(&variant.name)?,
            value_literal: json_string_literal(&variant.name)?,
        })
    }
}

pub(crate) fn render_js_flat_enum_fragment(enum_def: &EnumModel) -> Result<String> {
    Ok(JsFlatEnumTemplate {
        enum_def: FlatEnumJsView::from_enum(enum_def)?,
    }
    .render()?
    .trim_end()
    .to_string())
}

struct TaggedEnumJsView {
    name: String,
    variants: Vec<TaggedEnumVariantJsView>,
}

impl TaggedEnumJsView {
    fn from_enum(enum_def: &EnumModel) -> Result<Self> {
        Ok(Self {
            name: enum_def.name.clone(),
            variants: enum_def
                .variants
                .iter()
                .map(TaggedEnumVariantJsView::from_variant)
                .collect::<Result<_>>()?,
        })
    }
}

struct TaggedEnumVariantJsView {
    constructor_name: String,
    params: String,
    tag_literal: String,
    fields: Vec<TaggedEnumFieldJsView>,
}

impl TaggedEnumVariantJsView {
    fn from_variant(variant: &VariantModel) -> Result<Self> {
        Ok(Self {
            constructor_name: js_member_identifier(&variant.name),
            params: render_js_fields_as_params(&variant.fields),
            tag_literal: json_string_literal(&variant.name)?,
            fields: variant
                .fields
                .iter()
                .map(TaggedEnumFieldJsView::from_field)
                .collect::<Result<_>>()?,
        })
    }
}

struct TaggedEnumFieldJsView {
    property_name: String,
    value_expr: String,
}

impl TaggedEnumFieldJsView {
    fn from_field(field: &FieldModel) -> Result<Self> {
        Ok(Self {
            property_name: quoted_property_name(&field.name)?,
            value_expr: js_identifier(&field.name),
        })
    }
}

pub(crate) fn render_js_tagged_enum_fragment(enum_def: &EnumModel) -> Result<String> {
    Ok(JsTaggedEnumTemplate {
        enum_def: TaggedEnumJsView::from_enum(enum_def)?,
    }
    .render()?
    .trim_end()
    .to_string())
}

struct ErrorJsView {
    name: String,
    name_literal: String,
    is_flat: bool,
    variants: Vec<ErrorVariantJsView>,
}

impl ErrorJsView {
    fn from_error(error: &ErrorModel) -> Result<Self> {
        Ok(Self {
            name: error.name.clone(),
            name_literal: json_string_literal(&error.name)?,
            is_flat: error.is_flat,
            variants: error
                .variants
                .iter()
                .map(|variant| ErrorVariantJsView::from_variant(error, variant))
                .collect::<Result<_>>()?,
        })
    }
}

struct ErrorVariantJsView {
    class_name: String,
    class_name_literal: String,
    tag_literal: String,
    constructor_params: String,
    field_assignments: Vec<ErrorFieldAssignmentJsView>,
}

impl ErrorVariantJsView {
    fn from_variant(error: &ErrorModel, variant: &VariantModel) -> Result<Self> {
        let class_name = variant_type_name(&error.name, &variant.name);

        Ok(Self {
            class_name_literal: json_string_literal(&class_name)?,
            class_name,
            tag_literal: json_string_literal(&variant.name)?,
            constructor_params: if error.is_flat {
                "message = undefined".to_string()
            } else {
                render_js_fields_as_params(&variant.fields)
            },
            field_assignments: variant
                .fields
                .iter()
                .map(ErrorFieldAssignmentJsView::from_field)
                .collect::<Result<_>>()?,
        })
    }
}

struct ErrorFieldAssignmentJsView {
    property_name: String,
    value_expr: String,
}

impl ErrorFieldAssignmentJsView {
    fn from_field(field: &FieldModel) -> Result<Self> {
        Ok(Self {
            property_name: json_string_literal(&field.name)?,
            value_expr: js_identifier(&field.name),
        })
    }
}

pub(crate) fn render_js_error_fragment(error: &ErrorModel) -> Result<String> {
    Ok(JsErrorTemplate {
        error: ErrorJsView::from_error(error)?,
    }
    .render()?
    .trim_end()
    .to_string())
}

struct RecordDtsView {
    name: String,
    fields: Vec<FieldDtsView>,
}

impl RecordDtsView {
    fn from_record(record: &RecordModel) -> Result<Self> {
        Ok(Self {
            name: record.name.clone(),
            fields: record
                .fields
                .iter()
                .map(FieldDtsView::from_field)
                .collect::<Result<_>>()?,
        })
    }
}

struct FieldDtsView {
    property_name: String,
    type_name: String,
}

impl FieldDtsView {
    fn from_field(field: &FieldModel) -> Result<Self> {
        Ok(Self {
            property_name: quoted_property_name(&field.name)?,
            type_name: render_public_type(&field.type_)?,
        })
    }
}

fn render_dts_record_fragment(record: &RecordModel) -> Result<String> {
    Ok(DtsRecordTemplate {
        record: RecordDtsView::from_record(record)?,
    }
    .render()?
    .trim_end()
    .to_string())
}

struct CallbackInterfaceDtsView {
    name: String,
    methods: Vec<CallbackMethodDtsView>,
}

impl CallbackInterfaceDtsView {
    fn from_callback_interface(callback_interface: &CallbackInterfaceModel) -> Result<Self> {
        Ok(Self {
            name: callback_interface.name.clone(),
            methods: callback_interface
                .methods
                .iter()
                .map(CallbackMethodDtsView::from_method)
                .collect::<Result<_>>()?,
        })
    }
}

struct CallbackMethodDtsView {
    name: String,
    params: String,
    return_type: String,
}

impl CallbackMethodDtsView {
    fn from_method(method: &MethodModel) -> Result<Self> {
        Ok(Self {
            name: js_member_identifier(&method.name),
            params: render_dts_params(&method.arguments)?,
            return_type: render_return_type(method.return_type.as_ref(), method.is_async)?,
        })
    }
}

fn render_dts_callback_interface_fragment(
    callback_interface: &CallbackInterfaceModel,
) -> Result<String> {
    Ok(DtsCallbackInterfaceTemplate {
        callback_interface: CallbackInterfaceDtsView::from_callback_interface(callback_interface)?,
    }
    .render()?
    .trim_end()
    .to_string())
}

struct FunctionDtsView {
    name: String,
    params: String,
    return_type: String,
}

impl FunctionDtsView {
    fn from_function(function: &FunctionModel) -> Result<Self> {
        Ok(Self {
            name: js_member_identifier(&function.name),
            params: render_dts_params(&function.arguments)?,
            return_type: render_return_type(function.return_type.as_ref(), function.is_async)?,
        })
    }
}

fn render_dts_function_fragment(function: &FunctionModel) -> Result<String> {
    Ok(DtsFunctionTemplate {
        function: FunctionDtsView::from_function(function)?,
    }
    .render()?
    .trim_end()
    .to_string())
}

struct FlatEnumDtsView {
    name: String,
    variants: Vec<FlatEnumVariantDtsView>,
}

impl FlatEnumDtsView {
    fn from_enum(enum_def: &EnumModel) -> Result<Self> {
        Ok(Self {
            name: enum_def.name.clone(),
            variants: enum_def
                .variants
                .iter()
                .map(FlatEnumVariantDtsView::from_variant)
                .collect::<Result<_>>()?,
        })
    }
}

struct FlatEnumVariantDtsView {
    property_name: String,
    value_literal: String,
}

impl FlatEnumVariantDtsView {
    fn from_variant(variant: &VariantModel) -> Result<Self> {
        Ok(Self {
            property_name: quoted_property_name(&variant.name)?,
            value_literal: json_string_literal(&variant.name)?,
        })
    }
}

fn render_dts_flat_enum_fragment(enum_def: &EnumModel) -> Result<String> {
    Ok(DtsFlatEnumTemplate {
        enum_def: FlatEnumDtsView::from_enum(enum_def)?,
    }
    .render()?
    .trim_end()
    .to_string())
}

struct TaggedEnumDtsView {
    name: String,
    variant_types: String,
    variants: Vec<TaggedEnumVariantDtsView>,
}

impl TaggedEnumDtsView {
    fn from_enum(enum_def: &EnumModel) -> Result<Self> {
        let variants = enum_def
            .variants
            .iter()
            .map(|variant| TaggedEnumVariantDtsView::from_variant(&enum_def.name, variant))
            .collect::<Result<Vec<_>>>()?;
        let variant_types = variants
            .iter()
            .map(|variant| variant.type_name.clone())
            .collect::<Vec<_>>()
            .join(" | ");

        Ok(Self {
            name: enum_def.name.clone(),
            variant_types,
            variants,
        })
    }
}

struct TaggedEnumVariantDtsView {
    type_name: String,
    tag_literal: String,
    fields: Vec<FieldDtsView>,
    constructor_name: String,
    constructor_params: String,
}

impl TaggedEnumVariantDtsView {
    fn from_variant(enum_name: &str, variant: &VariantModel) -> Result<Self> {
        Ok(Self {
            type_name: variant_type_name(enum_name, &variant.name),
            tag_literal: json_string_literal(&variant.name)?,
            fields: variant
                .fields
                .iter()
                .map(FieldDtsView::from_field)
                .collect::<Result<_>>()?,
            constructor_name: js_member_identifier(&variant.name),
            constructor_params: render_dts_fields_as_params(&variant.fields)?,
        })
    }
}

fn render_dts_tagged_enum_fragment(enum_def: &EnumModel) -> Result<String> {
    Ok(DtsTaggedEnumTemplate {
        enum_def: TaggedEnumDtsView::from_enum(enum_def)?,
    }
    .render()?
    .trim_end()
    .to_string())
}

struct ErrorDtsView {
    name: String,
    variants: Vec<ErrorVariantDtsView>,
}

impl ErrorDtsView {
    fn from_error(error: &ErrorModel) -> Result<Self> {
        Ok(Self {
            name: error.name.clone(),
            variants: error
                .variants
                .iter()
                .map(|variant| ErrorVariantDtsView::from_variant(error, variant))
                .collect::<Result<_>>()?,
        })
    }
}

struct ErrorVariantDtsView {
    class_name: String,
    tag_literal: String,
    fields: Vec<FieldDtsView>,
    constructor_params: String,
}

impl ErrorVariantDtsView {
    fn from_variant(error: &ErrorModel, variant: &VariantModel) -> Result<Self> {
        Ok(Self {
            class_name: variant_type_name(&error.name, &variant.name),
            tag_literal: json_string_literal(&variant.name)?,
            fields: variant
                .fields
                .iter()
                .map(FieldDtsView::from_field)
                .collect::<Result<_>>()?,
            constructor_params: if error.is_flat {
                "message?: string".to_string()
            } else {
                render_dts_fields_as_params(&variant.fields)?
            },
        })
    }
}

fn render_dts_error_fragment(error: &ErrorModel) -> Result<String> {
    Ok(DtsErrorTemplate {
        error: ErrorDtsView::from_error(error)?,
    }
    .render()?
    .trim_end()
    .to_string())
}

struct ObjectDtsView {
    name: String,
    has_primary_constructor: bool,
    primary_constructor_params: String,
    constructors: Vec<ConstructorDtsView>,
    methods: Vec<ObjectMethodDtsView>,
}

impl ObjectDtsView {
    fn from_object(object: &ObjectModel) -> Result<Self> {
        let primary_constructor = object
            .constructors
            .iter()
            .find(|constructor| constructor.is_primary && !constructor.is_async);

        Ok(Self {
            name: object.name.clone(),
            has_primary_constructor: primary_constructor.is_some(),
            primary_constructor_params: primary_constructor
                .map(|constructor| render_dts_params(&constructor.arguments))
                .transpose()?
                .unwrap_or_default(),
            constructors: object
                .constructors
                .iter()
                .filter(|constructor| !(constructor.is_primary && !constructor.is_async))
                .map(|constructor| ConstructorDtsView::from_constructor(&object.name, constructor))
                .collect::<Result<_>>()?,
            methods: object
                .methods
                .iter()
                .map(ObjectMethodDtsView::from_method)
                .collect::<Result<_>>()?,
        })
    }
}

struct ConstructorDtsView {
    name: String,
    params: String,
    return_type: String,
}

impl ConstructorDtsView {
    fn from_constructor(object_name: &str, constructor: &ConstructorModel) -> Result<Self> {
        Ok(Self {
            name: js_member_identifier(&constructor.name),
            params: render_dts_params(&constructor.arguments)?,
            return_type: render_named_return_type(object_name, constructor.is_async),
        })
    }
}

struct ObjectMethodDtsView {
    name: String,
    params: String,
    return_type: String,
}

impl ObjectMethodDtsView {
    fn from_method(method: &MethodModel) -> Result<Self> {
        Ok(Self {
            name: js_member_identifier(&method.name),
            params: render_dts_params(&method.arguments)?,
            return_type: render_return_type(method.return_type.as_ref(), method.is_async)?,
        })
    }
}

fn render_dts_object_fragment(object: &ObjectModel) -> Result<String> {
    Ok(DtsObjectTemplate {
        object: ObjectDtsView::from_object(object)?,
    }
    .render()?
    .trim_end()
    .to_string())
}

#[derive(Template)]
#[template(path = "api/public-api.js.j2", escape = "none")]
struct PublicApiJsTemplate {
    renderer: JsRenderSections,
}

#[derive(Template)]
#[template(path = "api/js/function.js.j2", escape = "none")]
struct JsFunctionTemplate {
    function: FunctionJsView,
}

#[derive(Template)]
#[template(path = "api/js/object.js.j2", escape = "none")]
struct JsObjectTemplate {
    object: ObjectJsView,
}

#[derive(Template)]
#[template(path = "api/js/flat-enum.js.j2", escape = "none")]
struct JsFlatEnumTemplate {
    enum_def: FlatEnumJsView,
}

#[derive(Template)]
#[template(path = "api/js/tagged-enum.js.j2", escape = "none")]
struct JsTaggedEnumTemplate {
    enum_def: TaggedEnumJsView,
}

#[derive(Template)]
#[template(path = "api/js/error.js.j2", escape = "none")]
struct JsErrorTemplate {
    error: ErrorJsView,
}

#[derive(Template)]
#[template(path = "api/public-api.d.ts.j2", escape = "none")]
struct PublicApiDtsTemplate {
    renderer: DtsRenderer,
}

#[derive(Template)]
#[template(path = "api/dts/record.d.ts.j2", escape = "none")]
struct DtsRecordTemplate {
    record: RecordDtsView,
}

#[derive(Template)]
#[template(path = "api/dts/callback-interface.d.ts.j2", escape = "none")]
struct DtsCallbackInterfaceTemplate {
    callback_interface: CallbackInterfaceDtsView,
}

#[derive(Template)]
#[template(path = "api/dts/flat-enum.d.ts.j2", escape = "none")]
struct DtsFlatEnumTemplate {
    enum_def: FlatEnumDtsView,
}

#[derive(Template)]
#[template(path = "api/dts/tagged-enum.d.ts.j2", escape = "none")]
struct DtsTaggedEnumTemplate {
    enum_def: TaggedEnumDtsView,
}

#[derive(Template)]
#[template(path = "api/dts/error.d.ts.j2", escape = "none")]
struct DtsErrorTemplate {
    error: ErrorDtsView,
}

#[derive(Template)]
#[template(path = "api/dts/function.d.ts.j2", escape = "none")]
struct DtsFunctionTemplate {
    function: FunctionDtsView,
}

#[derive(Template)]
#[template(path = "api/dts/object.d.ts.j2", escape = "none")]
struct DtsObjectTemplate {
    object: ObjectDtsView,
}
