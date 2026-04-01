use anyhow::{Context, Result};
use askama::Template;

use super::ir::{AsyncCallbackMethodModel, VariantModel};
use super::{
    CallbackInterfaceModel, ComponentModel, ConstructorModel, EnumModel, ErrorModel, FieldModel,
    FunctionModel, MethodModel, ObjectModel, RecordModel, callback_interface_factory_name,
    callback_interface_proxy_class_name, callback_interface_register_name,
    callback_interface_registry_name, callback_interface_validator_name,
    callback_interface_vtable_struct_name, js_identifier, js_member_identifier,
    json_string_literal, object_converter_name, object_factory_name, quoted_property_name,
    render_doc_comment, render_dts_fields_as_params, render_dts_params, render_js_fields_as_params,
    render_js_function_body_lines, render_js_koffi_type_expression, render_js_lift_expression,
    render_js_lower_expression, render_js_object_constructor_body_lines,
    render_js_object_method_body_lines, render_js_params, render_js_primary_constructor_body_lines,
    render_js_property_access, render_js_record_allocation_size_expression,
    render_js_type_converter_expression, render_named_return_type, render_public_type,
    render_return_type, type_converter_name, variant_type_name,
};
use uniffi_bindgen::interface::Type;

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

    fn section_presence(&self) -> [bool; 10] {
        [
            self.has_unimplemented_helper(),
            self.has_runtime_helpers(),
            self.has_async_rust_future_helpers(),
            !self.flat_enums.is_empty(),
            !self.tagged_enums.is_empty(),
            !self.errors.is_empty(),
            self.has_placeholder_converters(),
            self.has_runtime_hooks(),
            !self.functions.is_empty(),
            !self.objects.is_empty(),
        ]
    }

    fn has_sections_after_unimplemented_helper(&self) -> bool {
        has_content_after(&self.section_presence(), 0)
    }

    fn has_sections_after_runtime_helpers(&self) -> bool {
        has_content_after(&self.section_presence(), 1)
    }

    fn has_sections_after_async_rust_future_helpers(&self) -> bool {
        has_content_after(&self.section_presence(), 2)
    }

    fn has_sections_after_flat_enums(&self) -> bool {
        has_content_after(&self.section_presence(), 3)
    }

    fn has_sections_after_tagged_enums(&self) -> bool {
        has_content_after(&self.section_presence(), 4)
    }

    fn has_sections_after_errors(&self) -> bool {
        has_content_after(&self.section_presence(), 5)
    }

    fn has_sections_after_placeholder_converters(&self) -> bool {
        has_content_after(&self.section_presence(), 6)
    }

    fn has_sections_after_runtime_hooks(&self) -> bool {
        has_content_after(&self.section_presence(), 7)
    }

    fn has_sections_after_functions(&self) -> bool {
        has_content_after(&self.section_presence(), 8)
    }
}

fn has_content_after(presence: &[bool], current_index: usize) -> bool {
    presence
        .iter()
        .skip(current_index + 1)
        .any(|present| *present)
}

fn collect_results<T, U>(items: &[T], render: impl Fn(&T) -> Result<U>) -> Result<Vec<U>> {
    items.iter().map(render).collect()
}

fn render_trimmed_template<T: Template>(template: T) -> Result<String> {
    Ok(template.render()?.trim_end().to_string())
}

pub(crate) struct PublicApiRenderer<'a> {
    model: &'a ComponentModel,
}

impl<'a> PublicApiRenderer<'a> {
    pub(crate) fn new(model: &'a ComponentModel) -> Self {
        Self { model }
    }

    pub(crate) fn render_js(&self, sections: JsRenderSections) -> Result<String> {
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
            records: collect_results(&model.records, render_dts_record_fragment)?,
            flat_enums: collect_results(&model.flat_enums, render_dts_flat_enum_fragment)?,
            tagged_enums: collect_results(&model.tagged_enums, render_dts_tagged_enum_fragment)?,
            errors: collect_results(&model.errors, render_dts_error_fragment)?,
            callback_interfaces: collect_results(
                &model.callback_interfaces,
                render_dts_callback_interface_fragment,
            )?,
            functions: collect_results(&model.functions, render_dts_function_fragment)?,
            objects: collect_results(&model.objects, render_dts_object_fragment)?,
        })
    }

    fn declaration_presence(&self) -> [bool; 7] {
        [
            !self.records.is_empty(),
            !self.flat_enums.is_empty(),
            !self.tagged_enums.is_empty(),
            !self.errors.is_empty(),
            !self.callback_interfaces.is_empty(),
            !self.functions.is_empty(),
            !self.objects.is_empty(),
        ]
    }

    fn has_declarations_after_records(&self) -> bool {
        has_content_after(&self.declaration_presence(), 0)
    }

    fn has_declarations_after_flat_enums(&self) -> bool {
        has_content_after(&self.declaration_presence(), 1)
    }

    fn has_declarations_after_tagged_enums(&self) -> bool {
        has_content_after(&self.declaration_presence(), 2)
    }

    fn has_declarations_after_errors(&self) -> bool {
        has_content_after(&self.declaration_presence(), 3)
    }

    fn has_declarations_after_callback_interfaces(&self) -> bool {
        has_content_after(&self.declaration_presence(), 4)
    }

    fn has_declarations_after_functions(&self) -> bool {
        has_content_after(&self.declaration_presence(), 5)
    }
}

struct FunctionJsView {
    doc_comment: String,
    name: String,
    is_async: bool,
    params: String,
    body_lines: Vec<String>,
}

impl FunctionJsView {
    fn from_function(function: &FunctionModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(function.docstring.as_deref(), ""),
            name: js_identifier(&function.name),
            is_async: function.is_async,
            params: render_js_params(&function.arguments),
            body_lines: render_js_function_body_lines(function)?,
        })
    }
}

pub(crate) fn render_js_function_fragment(function: &FunctionModel) -> Result<String> {
    render_trimmed_template(JsFunctionTemplate {
        function: FunctionJsView::from_function(function)?,
    })
}

struct ObjectJsView {
    doc_comment: String,
    name: String,
    factory_name: String,
    converter_name: String,
    type_name_literal: String,
    ffi_object_clone_symbol: String,
    ffi_object_clone_identifier: String,
    ffi_object_clone_raw_external_cache_key: String,
    ffi_object_free_symbol: String,
    ffi_object_free_identifier: String,
    ffi_object_free_raw_external_cache_key: String,
    has_primary_constructor: bool,
    primary_constructor_doc_comment: String,
    primary_constructor_params: String,
    primary_constructor_body_lines: Vec<String>,
    unimplemented_constructor_member: String,
    constructors: Vec<ConstructorJsView>,
    methods: Vec<ObjectMethodJsView>,
}

#[derive(Default)]
struct PrimaryConstructorJsView {
    has_primary_constructor: bool,
    doc_comment: String,
    params: String,
    body_lines: Vec<String>,
}

impl PrimaryConstructorJsView {
    fn from_object(object: &ObjectModel, factory_name: &str) -> Result<Self> {
        let Some(constructor) = primary_sync_constructor(object) else {
            return Ok(Self::default());
        };

        Ok(Self {
            has_primary_constructor: true,
            doc_comment: render_doc_comment(constructor.docstring.as_deref(), "  "),
            params: render_js_params(&constructor.arguments),
            body_lines: render_js_primary_constructor_body_lines(constructor, factory_name)?,
        })
    }
}

fn is_sync_primary_constructor(constructor: &ConstructorModel) -> bool {
    constructor.is_primary && !constructor.is_async
}

fn primary_sync_constructor(object: &ObjectModel) -> Option<&ConstructorModel> {
    object
        .constructors
        .iter()
        .find(|constructor| is_sync_primary_constructor(constructor))
}

fn render_secondary_constructors(object: &ObjectModel) -> Result<Vec<ConstructorJsView>> {
    object
        .constructors
        .iter()
        .filter(|constructor| !is_sync_primary_constructor(constructor))
        .map(|constructor| ConstructorJsView::from_constructor(object, constructor))
        .collect()
}

fn render_object_method_views(object: &ObjectModel) -> Result<Vec<ObjectMethodJsView>> {
    object
        .methods
        .iter()
        .map(|method| ObjectMethodJsView::from_method(object, method))
        .collect()
}

impl ObjectJsView {
    fn from_object(object: &ObjectModel) -> Result<Self> {
        let factory_name = object_factory_name(&object.name);
        let primary_constructor = PrimaryConstructorJsView::from_object(object, &factory_name)?;

        Ok(Self {
            doc_comment: render_doc_comment(object.docstring.as_deref(), ""),
            name: object.name.clone(),
            factory_name: factory_name.clone(),
            converter_name: object_converter_name(&object.name),
            type_name_literal: json_string_literal(&object.name)?,
            ffi_object_clone_symbol: json_string_literal(&object.ffi_object_clone_identifier)?,
            ffi_object_clone_identifier: object.ffi_object_clone_identifier.clone(),
            ffi_object_clone_raw_external_cache_key: json_string_literal(&format!(
                "{}:raw-external",
                object.ffi_object_clone_identifier
            ))?,
            ffi_object_free_symbol: json_string_literal(&object.ffi_object_free_identifier)?,
            ffi_object_free_identifier: object.ffi_object_free_identifier.clone(),
            ffi_object_free_raw_external_cache_key: json_string_literal(&format!(
                "{}:raw-external",
                object.ffi_object_free_identifier
            ))?,
            has_primary_constructor: primary_constructor.has_primary_constructor,
            primary_constructor_doc_comment: primary_constructor.doc_comment,
            primary_constructor_params: primary_constructor.params,
            primary_constructor_body_lines: primary_constructor.body_lines,
            unimplemented_constructor_member: json_string_literal(&format!(
                "{}.constructor",
                object.name
            ))?,
            constructors: render_secondary_constructors(object)?,
            methods: render_object_method_views(object)?,
        })
    }
}

struct ConstructorJsView {
    doc_comment: String,
    name: String,
    is_async: bool,
    params: String,
    body_lines: Vec<String>,
}

impl ConstructorJsView {
    fn from_constructor(object: &ObjectModel, constructor: &ConstructorModel) -> Result<Self> {
        let factory_name = object_factory_name(&object.name);

        Ok(Self {
            doc_comment: render_doc_comment(constructor.docstring.as_deref(), "  "),
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
    doc_comment: String,
    name: String,
    is_async: bool,
    params: String,
    body_lines: Vec<String>,
}

impl ObjectMethodJsView {
    fn from_method(object: &ObjectModel, method: &MethodModel) -> Result<Self> {
        let factory_name = object_factory_name(&object.name);

        Ok(Self {
            doc_comment: render_doc_comment(method.docstring.as_deref(), "  "),
            name: js_member_identifier(&method.name),
            is_async: method.is_async,
            params: render_js_params(&method.arguments),
            body_lines: render_js_object_method_body_lines(method, &factory_name, &object.name)?,
        })
    }
}

pub(crate) fn render_js_object_fragment(object: &ObjectModel) -> Result<String> {
    render_trimmed_template(JsObjectTemplate {
        object: ObjectJsView::from_object(object)?,
    })
}

struct FlatEnumJsView {
    doc_comment: String,
    name: String,
    variants: Vec<FlatEnumVariantJsView>,
}

impl FlatEnumJsView {
    fn from_enum(enum_def: &EnumModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(enum_def.docstring.as_deref(), ""),
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
    doc_comment: String,
    property_name: String,
    value_literal: String,
}

impl FlatEnumVariantJsView {
    fn from_variant(variant: &VariantModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(variant.docstring.as_deref(), "  "),
            property_name: quoted_property_name(&variant.name)?,
            value_literal: json_string_literal(&variant.name)?,
        })
    }
}

pub(crate) fn render_js_flat_enum_fragment(enum_def: &EnumModel) -> Result<String> {
    render_trimmed_template(JsFlatEnumTemplate {
        enum_def: FlatEnumJsView::from_enum(enum_def)?,
    })
}

struct TaggedEnumJsView {
    doc_comment: String,
    name: String,
    variants: Vec<TaggedEnumVariantJsView>,
}

impl TaggedEnumJsView {
    fn from_enum(enum_def: &EnumModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(enum_def.docstring.as_deref(), ""),
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
    doc_comment: String,
    constructor_name: String,
    params: String,
    tag_literal: String,
    fields: Vec<TaggedEnumFieldJsView>,
}

impl TaggedEnumVariantJsView {
    fn from_variant(variant: &VariantModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(variant.docstring.as_deref(), "  "),
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
    render_trimmed_template(JsTaggedEnumTemplate {
        enum_def: TaggedEnumJsView::from_enum(enum_def)?,
    })
}

struct ErrorJsView {
    doc_comment: String,
    name: String,
    name_literal: String,
    is_flat: bool,
    variants: Vec<ErrorVariantJsView>,
}

impl ErrorJsView {
    fn from_error(error: &ErrorModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(error.docstring.as_deref(), ""),
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
    doc_comment: String,
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
            doc_comment: render_doc_comment(variant.docstring.as_deref(), ""),
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
    render_trimmed_template(JsErrorTemplate {
        error: ErrorJsView::from_error(error)?,
    })
}

struct ConverterWriteFieldJsView {
    converter_expr: String,
    value_expr: String,
}

impl ConverterWriteFieldJsView {
    fn from_field(field: &FieldModel, value_expr: &str) -> Result<Self> {
        Ok(Self {
            converter_expr: render_js_type_converter_expression(&field.type_)?,
            value_expr: render_js_property_access(value_expr, &field.name)?,
        })
    }
}

struct ConverterReadFieldJsView {
    property_name: String,
    read_expr: String,
}

impl ConverterReadFieldJsView {
    fn from_field(field: &FieldModel) -> Result<Self> {
        Ok(Self {
            property_name: quoted_property_name(&field.name)?,
            read_expr: format!(
                "{}.read(reader)",
                render_js_type_converter_expression(&field.type_)?
            ),
        })
    }
}

fn render_converter_write_fields(
    fields: &[FieldModel],
    value_expr: &str,
) -> Result<Vec<ConverterWriteFieldJsView>> {
    fields
        .iter()
        .map(|field| ConverterWriteFieldJsView::from_field(field, value_expr))
        .collect()
}

fn render_converter_read_fields(fields: &[FieldModel]) -> Result<Vec<ConverterReadFieldJsView>> {
    fields
        .iter()
        .map(ConverterReadFieldJsView::from_field)
        .collect()
}

fn render_converter_read_expressions(fields: &[FieldModel]) -> Result<Vec<String>> {
    fields
        .iter()
        .map(|field| {
            Ok(format!(
                "{}.read(reader)",
                render_js_type_converter_expression(&field.type_)?
            ))
        })
        .collect()
}

fn render_buffered_variant_allocation_size_expression(
    fields: &[FieldModel],
    value_expr: &str,
) -> Result<String> {
    let allocation_terms = fields
        .iter()
        .map(|field| {
            Ok(format!(
                "{}.allocationSize({})",
                render_js_type_converter_expression(&field.type_)?,
                render_js_property_access(value_expr, &field.name)?
            ))
        })
        .collect::<Result<Vec<_>>>()?;

    if allocation_terms.is_empty() {
        Ok("4".to_string())
    } else {
        Ok(format!("4 + {}", allocation_terms.join(" + ")))
    }
}

struct RecordConverterJsView {
    converter_name: String,
    record_type_name: String,
    allocation_size_expr: String,
    write_fields: Vec<ConverterWriteFieldJsView>,
    read_fields: Vec<ConverterReadFieldJsView>,
}

impl RecordConverterJsView {
    fn from_record(record: &RecordModel) -> Result<Self> {
        Ok(Self {
            converter_name: type_converter_name(&record.name),
            record_type_name: json_string_literal(&record.name)?,
            allocation_size_expr: render_js_record_allocation_size_expression(record)?,
            write_fields: render_converter_write_fields(&record.fields, "recordValue")?,
            read_fields: render_converter_read_fields(&record.fields)?,
        })
    }
}

pub(crate) fn render_js_record_converter_fragment(record: &RecordModel) -> Result<String> {
    render_trimmed_template(JsRecordConverterTemplate {
        converter: RecordConverterJsView::from_record(record)?,
    })
}

struct FlatEnumConverterJsView {
    converter_name: String,
    enum_name: String,
    enum_type_name: String,
    variants: Vec<FlatEnumConverterVariantJsView>,
}

impl FlatEnumConverterJsView {
    fn from_enum(enum_def: &EnumModel) -> Result<Self> {
        Ok(Self {
            converter_name: type_converter_name(&enum_def.name),
            enum_name: enum_def.name.clone(),
            enum_type_name: json_string_literal(&enum_def.name)?,
            variants: enum_def
                .variants
                .iter()
                .enumerate()
                .map(|(index, variant)| {
                    FlatEnumConverterVariantJsView::from_variant(&enum_def.name, variant, index + 1)
                })
                .collect::<Result<_>>()?,
        })
    }
}

struct FlatEnumConverterVariantJsView {
    tag_index: usize,
    case_value_expr: String,
}

impl FlatEnumConverterVariantJsView {
    fn from_variant(enum_name: &str, variant: &VariantModel, tag_index: usize) -> Result<Self> {
        Ok(Self {
            tag_index,
            case_value_expr: render_js_property_access(enum_name, &variant.name)?,
        })
    }
}

pub(crate) fn render_js_flat_enum_converter_fragment(enum_def: &EnumModel) -> Result<String> {
    render_trimmed_template(JsFlatEnumConverterTemplate {
        converter: FlatEnumConverterJsView::from_enum(enum_def)?,
    })
}

struct TaggedEnumConverterJsView {
    converter_name: String,
    enum_name: String,
    enum_type_name: String,
    variants: Vec<TaggedEnumConverterVariantJsView>,
}

impl TaggedEnumConverterJsView {
    fn from_enum(enum_def: &EnumModel) -> Result<Self> {
        Ok(Self {
            converter_name: type_converter_name(&enum_def.name),
            enum_name: enum_def.name.clone(),
            enum_type_name: json_string_literal(&enum_def.name)?,
            variants: enum_def
                .variants
                .iter()
                .enumerate()
                .map(|(index, variant)| {
                    TaggedEnumConverterVariantJsView::from_variant(
                        &enum_def.name,
                        variant,
                        index + 1,
                    )
                })
                .collect::<Result<_>>()?,
        })
    }
}

struct TaggedEnumConverterVariantJsView {
    tag_literal: String,
    tag_index: usize,
    allocation_size_expr: String,
    write_fields: Vec<ConverterWriteFieldJsView>,
    read_return_expr: String,
}

impl TaggedEnumConverterVariantJsView {
    fn from_variant(enum_name: &str, variant: &VariantModel, tag_index: usize) -> Result<Self> {
        let read_values = render_converter_read_expressions(&variant.fields)?;

        Ok(Self {
            tag_literal: json_string_literal(&variant.name)?,
            tag_index,
            allocation_size_expr: render_buffered_variant_allocation_size_expression(
                &variant.fields,
                "enumValue",
            )?,
            write_fields: render_converter_write_fields(&variant.fields, "enumValue")?,
            read_return_expr: format!(
                "{}.{}({})",
                enum_name,
                js_member_identifier(&variant.name),
                read_values.join(", ")
            ),
        })
    }
}

pub(crate) fn render_js_tagged_enum_converter_fragment(enum_def: &EnumModel) -> Result<String> {
    render_trimmed_template(JsTaggedEnumConverterTemplate {
        converter: TaggedEnumConverterJsView::from_enum(enum_def)?,
    })
}

struct ErrorConverterJsView {
    converter_name: String,
    error_name: String,
    invalid_value_message: String,
    variants: Vec<ErrorConverterVariantJsView>,
}

impl ErrorConverterJsView {
    fn from_error(error: &ErrorModel) -> Result<Self> {
        let allowed_variants = error
            .variants
            .iter()
            .map(|variant| variant_type_name(&error.name, &variant.name))
            .collect::<Vec<_>>()
            .join(", ");

        Ok(Self {
            converter_name: type_converter_name(&error.name),
            error_name: error.name.clone(),
            invalid_value_message: json_string_literal(&format!(
                "{} values must be instances of {}.",
                error.name, allowed_variants
            ))?,
            variants: error
                .variants
                .iter()
                .enumerate()
                .map(|(index, variant)| {
                    ErrorConverterVariantJsView::from_variant(error, variant, index + 1)
                })
                .collect::<Result<_>>()?,
        })
    }
}

struct ErrorConverterVariantJsView {
    class_name: String,
    tag_index: usize,
    allocation_size_expr: String,
    write_fields: Vec<ConverterWriteFieldJsView>,
    read_expr: String,
}

impl ErrorConverterVariantJsView {
    fn from_variant(error: &ErrorModel, variant: &VariantModel, tag_index: usize) -> Result<Self> {
        let class_name = variant_type_name(&error.name, &variant.name);
        let allocation_size_expr = if error.is_flat {
            "4".to_string()
        } else {
            render_buffered_variant_allocation_size_expression(&variant.fields, "value")?
        };
        let read_expr = if error.is_flat {
            format!(
                "new {}({}.read(reader))",
                class_name,
                render_js_type_converter_expression(&Type::String)?
            )
        } else {
            let field_values = render_converter_read_expressions(&variant.fields)?;
            format!("new {}({})", class_name, field_values.join(", "))
        };

        Ok(Self {
            class_name,
            tag_index,
            allocation_size_expr,
            write_fields: if error.is_flat {
                Vec::new()
            } else {
                render_converter_write_fields(&variant.fields, "value")?
            },
            read_expr,
        })
    }
}

pub(crate) fn render_js_error_converter_fragment(error: &ErrorModel) -> Result<String> {
    render_trimmed_template(JsErrorConverterTemplate {
        converter: ErrorConverterJsView::from_error(error)?,
    })
}

struct CallbackInterfaceConverterJsView {
    proxy_class_name: String,
    factory_name: String,
    registry_name: String,
    validator_name: String,
    register_name: String,
    converter_name: String,
    interface_name: String,
    interface_name_template_expr: String,
    ffi_object_clone_identifier: String,
    ffi_object_free_identifier: String,
    ffi_init_callback_identifier: String,
    methods: Vec<CallbackInterfaceMethodJsView>,
    registrations: Vec<String>,
    vtable_struct_name: String,
}

impl CallbackInterfaceConverterJsView {
    fn from_callback_interface(callback_interface: &CallbackInterfaceModel) -> Result<Self> {
        let factory_name = callback_interface_factory_name(&callback_interface.name);
        let registry_name = callback_interface_registry_name(&callback_interface.name);
        let methods = callback_interface
            .methods
            .iter()
            .map(|method| {
                CallbackInterfaceMethodJsView::from_method(
                    method,
                    &factory_name,
                    &callback_interface.name,
                )
            })
            .collect::<Result<Vec<_>>>()?;
        let registrations = callback_interface
            .methods
            .iter()
            .map(|method| {
                render_js_callback_vtable_registration_fragment(
                    callback_interface,
                    method,
                    &registry_name,
                )
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            proxy_class_name: callback_interface_proxy_class_name(&callback_interface.name),
            factory_name,
            registry_name,
            validator_name: callback_interface_validator_name(&callback_interface.name),
            register_name: callback_interface_register_name(&callback_interface.name),
            converter_name: type_converter_name(&callback_interface.name),
            interface_name: json_string_literal(&callback_interface.name)?,
            interface_name_template_expr: format!(
                "${{{}}}",
                json_string_literal(&callback_interface.name)?
            ),
            ffi_object_clone_identifier: callback_interface.ffi_object_clone_identifier.clone(),
            ffi_object_free_identifier: callback_interface.ffi_object_free_identifier.clone(),
            ffi_init_callback_identifier: callback_interface.ffi_init_callback_identifier.clone(),
            methods,
            registrations,
            vtable_struct_name: callback_interface_vtable_struct_name(&callback_interface.name),
        })
    }
}

struct CallbackInterfaceMethodJsView {
    name: String,
    is_async: bool,
    params: String,
    body_lines: Vec<String>,
    property_name: String,
    callback_name: String,
}

impl CallbackInterfaceMethodJsView {
    fn from_method(method: &MethodModel, factory_name: &str, interface_name: &str) -> Result<Self> {
        let name = js_member_identifier(&method.name);

        Ok(Self {
            property_name: quoted_property_name(&method.name)?,
            callback_name: format!("{name}Callback"),
            name,
            is_async: method.is_async,
            params: render_js_params(&method.arguments),
            body_lines: render_js_object_method_body_lines(method, factory_name, interface_name)?,
        })
    }
}

pub(crate) fn render_js_callback_interface_converter_fragment(
    callback_interface: &CallbackInterfaceModel,
) -> Result<String> {
    render_trimmed_template(JsCallbackInterfaceConverterTemplate {
        converter: CallbackInterfaceConverterJsView::from_callback_interface(callback_interface)?,
    })
}

struct RuntimeHelpersJsView {
    ffi_rustbuffer_from_bytes_identifier: String,
    ffi_rustbuffer_free_identifier: String,
}

pub(crate) fn render_js_runtime_helpers_fragment(
    ffi_rustbuffer_from_bytes_identifier: &str,
    ffi_rustbuffer_free_identifier: &str,
) -> Result<String> {
    render_trimmed_template(JsRuntimeHelpersTemplate {
        helpers: RuntimeHelpersJsView {
            ffi_rustbuffer_from_bytes_identifier: ffi_rustbuffer_from_bytes_identifier.to_string(),
            ffi_rustbuffer_free_identifier: ffi_rustbuffer_free_identifier.to_string(),
        },
    })
}

pub(crate) fn render_js_async_rust_future_helpers_fragment() -> Result<String> {
    render_trimmed_template(JsAsyncRustFutureHelpersTemplate)
}

struct RuntimeHooksJsView {
    callback_register_names: Vec<String>,
    callback_registry_names: Vec<String>,
    has_callback_interfaces: bool,
    needs_async_rust_future_hooks: bool,
}

impl RuntimeHooksJsView {
    fn from_callback_interfaces(
        callback_interfaces: &[CallbackInterfaceModel],
        needs_async_rust_future_hooks: bool,
    ) -> Self {
        Self {
            callback_register_names: callback_interfaces
                .iter()
                .map(|callback_interface| {
                    callback_interface_register_name(&callback_interface.name)
                })
                .collect(),
            callback_registry_names: callback_interfaces
                .iter()
                .map(|callback_interface| {
                    callback_interface_registry_name(&callback_interface.name)
                })
                .collect(),
            has_callback_interfaces: !callback_interfaces.is_empty(),
            needs_async_rust_future_hooks,
        }
    }
}

pub(crate) fn render_js_runtime_hooks_fragment(
    callback_interfaces: &[CallbackInterfaceModel],
    needs_async_rust_future_hooks: bool,
) -> Result<String> {
    render_trimmed_template(JsRuntimeHooksTemplate {
        hooks: RuntimeHooksJsView::from_callback_interfaces(
            callback_interfaces,
            needs_async_rust_future_hooks,
        ),
    })
}

#[derive(Default)]
struct CallbackErrorLowering {
    lower_error_type: String,
    lower_error_converter_expr: String,
    has_lower_error: bool,
}

impl CallbackErrorLowering {
    fn from_method(method: &MethodModel) -> Result<Self> {
        let Some(throws_type) = method.throws_type.as_ref() else {
            return Ok(Self::default());
        };

        Ok(Self {
            lower_error_type: render_public_type(throws_type)?,
            lower_error_converter_expr: render_js_type_converter_expression(throws_type)?,
            has_lower_error: true,
        })
    }
}

#[derive(Default)]
struct SyncCallbackReturnLowering {
    lowered_return_expr: String,
    return_koffi_type_expr: String,
    has_return_value: bool,
}

impl SyncCallbackReturnLowering {
    fn from_method(method: &MethodModel) -> Result<Self> {
        let Some(return_type) = method.return_type.as_ref() else {
            return Ok(Self::default());
        };

        Ok(Self {
            lowered_return_expr: render_js_lower_expression(return_type, "uniffiResult")?,
            return_koffi_type_expr: render_js_koffi_type_expression(return_type, "bindings")?,
            has_return_value: true,
        })
    }
}

#[derive(Default)]
struct AsyncCallbackReturnLowering {
    lower_return_expr: String,
    default_return_value_expr: String,
    has_lower_return: bool,
}

impl AsyncCallbackReturnLowering {
    fn from_method(
        callback_interface: &CallbackInterfaceModel,
        method: &MethodModel,
        async_callback_ffi: &AsyncCallbackMethodModel,
    ) -> Result<Self> {
        if !async_callback_ffi.result_struct_has_return_value {
            return Ok(Self::default());
        }

        Ok(Self {
            lower_return_expr: render_js_lower_expression(
                async_callback_return_type(callback_interface, method)?,
                "value",
            )?,
            default_return_value_expr: async_callback_default_return_value(
                callback_interface,
                method,
                async_callback_ffi,
            )?
            .to_string(),
            has_lower_return: true,
        })
    }
}

fn async_callback_return_type<'a>(
    callback_interface: &CallbackInterfaceModel,
    method: &'a MethodModel,
) -> Result<&'a Type> {
    method.return_type.as_ref().with_context(|| {
        format!(
            "async callback interface {}.{} is missing a return type",
            callback_interface.name, method.name
        )
    })
}

fn async_callback_default_return_value<'a>(
    callback_interface: &CallbackInterfaceModel,
    method: &MethodModel,
    async_callback_ffi: &'a AsyncCallbackMethodModel,
) -> Result<&'a str> {
    async_callback_ffi
        .default_error_return_value_expression
        .as_deref()
        .with_context(|| {
            format!(
                "async callback interface {}.{} is missing a default error return value",
                callback_interface.name, method.name
            )
        })
}

fn callback_registration_name(method: &MethodModel) -> String {
    format!("{}Callback", js_member_identifier(&method.name))
}

fn callback_registration_params(method: &MethodModel, trailing: &[&str]) -> String {
    method
        .arguments
        .iter()
        .map(|argument| js_identifier(&argument.name))
        .chain(trailing.iter().map(|name| (*name).to_string()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn callback_registration_lifted_args(method: &MethodModel) -> Result<Vec<String>> {
    method
        .arguments
        .iter()
        .map(|argument| render_js_lift_expression(&argument.type_, &js_identifier(&argument.name)))
        .collect()
}

fn callback_ffi_identifier<'a>(
    callback_interface: &CallbackInterfaceModel,
    method: &'a MethodModel,
) -> Result<&'a str> {
    method.ffi_callback_identifier.as_deref().with_context(|| {
        format!(
            "callback interface {}.{} is missing an FFI callback identifier",
            callback_interface.name, method.name
        )
    })
}

fn callback_async_metadata<'a>(
    callback_interface: &CallbackInterfaceModel,
    method: &'a MethodModel,
) -> Result<&'a AsyncCallbackMethodModel> {
    method.async_callback_ffi.as_ref().with_context(|| {
        format!(
            "async callback interface {}.{} is missing ForeignFuture metadata",
            callback_interface.name, method.name
        )
    })
}

struct CallbackVtableRegistrationCommon {
    callback_name: String,
    callback_params: String,
    registry_name: String,
    method_name_literal: String,
    lifted_args: Vec<String>,
    ffi_callback_identifier: String,
}

impl CallbackVtableRegistrationCommon {
    fn from_method(
        callback_interface: &CallbackInterfaceModel,
        method: &MethodModel,
        registry_name: &str,
        trailing_params: &[&str],
    ) -> Result<Self> {
        Ok(Self {
            callback_name: callback_registration_name(method),
            callback_params: callback_registration_params(method, trailing_params),
            registry_name: registry_name.to_string(),
            method_name_literal: json_string_literal(&method.name)?,
            lifted_args: callback_registration_lifted_args(method)?,
            ffi_callback_identifier: callback_ffi_identifier(callback_interface, method)?
                .to_string(),
        })
    }
}

struct CallbackVtableRegistrationBase {
    common: CallbackVtableRegistrationCommon,
    error_lowering: CallbackErrorLowering,
}

const SYNC_CALLBACK_TRAILING_PARAMS: &[&str] = &["uniffiOutReturn", "callStatus"];
const ASYNC_CALLBACK_TRAILING_PARAMS: &[&str] = &[
    "uniffiFutureCallback",
    "uniffiCallbackData",
    "uniffiOutReturn",
];

impl CallbackVtableRegistrationBase {
    fn from_method(
        callback_interface: &CallbackInterfaceModel,
        method: &MethodModel,
        registry_name: &str,
        trailing_params: &[&str],
    ) -> Result<Self> {
        Ok(Self {
            common: CallbackVtableRegistrationCommon::from_method(
                callback_interface,
                method,
                registry_name,
                trailing_params,
            )?,
            error_lowering: CallbackErrorLowering::from_method(method)?,
        })
    }
}

struct SyncCallbackVtableRegistrationJsView {
    callback_name: String,
    callback_params: String,
    registry_name: String,
    method_name_literal: String,
    lifted_args: Vec<String>,
    ffi_callback_identifier: String,
    lowered_return_expr: String,
    return_koffi_type_expr: String,
    has_return_value: bool,
    lower_error_type: String,
    lower_error_converter_expr: String,
    has_lower_error: bool,
}

impl SyncCallbackVtableRegistrationJsView {
    fn from_method(
        callback_interface: &CallbackInterfaceModel,
        method: &MethodModel,
        registry_name: &str,
    ) -> Result<Self> {
        let base = CallbackVtableRegistrationBase::from_method(
            callback_interface,
            method,
            registry_name,
            SYNC_CALLBACK_TRAILING_PARAMS,
        )?;
        Ok(Self::from_parts(
            base,
            SyncCallbackReturnLowering::from_method(method)?,
        ))
    }

    fn from_parts(
        base: CallbackVtableRegistrationBase,
        return_lowering: SyncCallbackReturnLowering,
    ) -> Self {
        let CallbackVtableRegistrationBase {
            common: registration,
            error_lowering,
        } = base;

        Self {
            callback_name: registration.callback_name,
            callback_params: registration.callback_params,
            registry_name: registration.registry_name,
            method_name_literal: registration.method_name_literal,
            lifted_args: registration.lifted_args,
            ffi_callback_identifier: registration.ffi_callback_identifier,
            lowered_return_expr: return_lowering.lowered_return_expr,
            return_koffi_type_expr: return_lowering.return_koffi_type_expr,
            has_return_value: return_lowering.has_return_value,
            lower_error_type: error_lowering.lower_error_type,
            lower_error_converter_expr: error_lowering.lower_error_converter_expr,
            has_lower_error: error_lowering.has_lower_error,
        }
    }
}

struct AsyncCallbackVtableRegistrationJsView {
    future_free_name: String,
    callback_name: String,
    callback_params: String,
    registry_name: String,
    method_name_literal: String,
    lifted_args: Vec<String>,
    complete_callback_identifier: String,
    dropped_callback_identifier: String,
    ffi_callback_identifier: String,
    lower_error_type: String,
    lower_error_converter_expr: String,
    has_lower_error: bool,
    lower_return_expr: String,
    default_return_value_expr: String,
    has_lower_return: bool,
}

struct AsyncCallbackRegistrationMetadata {
    future_free_name: String,
    complete_callback_identifier: String,
    dropped_callback_identifier: String,
}

impl AsyncCallbackRegistrationMetadata {
    fn from_method(method: &MethodModel, async_callback_ffi: &AsyncCallbackMethodModel) -> Self {
        Self {
            future_free_name: format!("{}FutureFree", js_identifier(&method.name)),
            complete_callback_identifier: async_callback_ffi.complete_identifier.clone(),
            dropped_callback_identifier: async_callback_ffi.dropped_callback_identifier.clone(),
        }
    }
}

impl AsyncCallbackVtableRegistrationJsView {
    fn from_method(
        callback_interface: &CallbackInterfaceModel,
        method: &MethodModel,
        registry_name: &str,
    ) -> Result<Self> {
        let base = CallbackVtableRegistrationBase::from_method(
            callback_interface,
            method,
            registry_name,
            ASYNC_CALLBACK_TRAILING_PARAMS,
        )?;
        let async_callback_ffi = callback_async_metadata(callback_interface, method)?;
        let async_metadata =
            AsyncCallbackRegistrationMetadata::from_method(method, async_callback_ffi);
        let return_lowering = AsyncCallbackReturnLowering::from_method(
            callback_interface,
            method,
            async_callback_ffi,
        )?;

        Ok(Self::from_parts(base, async_metadata, return_lowering))
    }

    fn from_parts(
        base: CallbackVtableRegistrationBase,
        async_metadata: AsyncCallbackRegistrationMetadata,
        return_lowering: AsyncCallbackReturnLowering,
    ) -> Self {
        let CallbackVtableRegistrationBase {
            common: registration,
            error_lowering,
        } = base;

        Self {
            future_free_name: async_metadata.future_free_name,
            callback_name: registration.callback_name,
            callback_params: registration.callback_params,
            registry_name: registration.registry_name,
            method_name_literal: registration.method_name_literal,
            lifted_args: registration.lifted_args,
            complete_callback_identifier: async_metadata.complete_callback_identifier,
            dropped_callback_identifier: async_metadata.dropped_callback_identifier,
            ffi_callback_identifier: registration.ffi_callback_identifier,
            lower_error_type: error_lowering.lower_error_type,
            lower_error_converter_expr: error_lowering.lower_error_converter_expr,
            has_lower_error: error_lowering.has_lower_error,
            lower_return_expr: return_lowering.lower_return_expr,
            default_return_value_expr: return_lowering.default_return_value_expr,
            has_lower_return: return_lowering.has_lower_return,
        }
    }
}

fn render_js_callback_vtable_registration_fragment(
    callback_interface: &CallbackInterfaceModel,
    method: &MethodModel,
    registry_name: &str,
) -> Result<String> {
    if method.is_async {
        return render_js_async_callback_vtable_registration_fragment(
            callback_interface,
            method,
            registry_name,
        );
    }

    render_js_sync_callback_vtable_registration_fragment(callback_interface, method, registry_name)
}

fn render_js_sync_callback_vtable_registration_fragment(
    callback_interface: &CallbackInterfaceModel,
    method: &MethodModel,
    registry_name: &str,
) -> Result<String> {
    render_trimmed_template(JsSyncCallbackVtableRegistrationTemplate {
        registration: SyncCallbackVtableRegistrationJsView::from_method(
            callback_interface,
            method,
            registry_name,
        )?,
    })
}

fn render_js_async_callback_vtable_registration_fragment(
    callback_interface: &CallbackInterfaceModel,
    method: &MethodModel,
    registry_name: &str,
) -> Result<String> {
    render_trimmed_template(JsAsyncCallbackVtableRegistrationTemplate {
        registration: AsyncCallbackVtableRegistrationJsView::from_method(
            callback_interface,
            method,
            registry_name,
        )?,
    })
}

struct RecordDtsView {
    doc_comment: String,
    name: String,
    fields: Vec<FieldDtsView>,
}

impl RecordDtsView {
    fn from_record(record: &RecordModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(record.docstring.as_deref(), ""),
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
    doc_comment: String,
    property_name: String,
    type_name: String,
}

impl FieldDtsView {
    fn from_field(field: &FieldModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(field.docstring.as_deref(), "  "),
            property_name: quoted_property_name(&field.name)?,
            type_name: render_public_type(&field.type_)?,
        })
    }
}

fn render_dts_record_fragment(record: &RecordModel) -> Result<String> {
    render_trimmed_template(DtsRecordTemplate {
        record: RecordDtsView::from_record(record)?,
    })
}

struct CallbackInterfaceDtsView {
    doc_comment: String,
    name: String,
    methods: Vec<CallbackMethodDtsView>,
}

impl CallbackInterfaceDtsView {
    fn from_callback_interface(callback_interface: &CallbackInterfaceModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(callback_interface.docstring.as_deref(), ""),
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
    doc_comment: String,
    name: String,
    params: String,
    return_type: String,
}

impl CallbackMethodDtsView {
    fn from_method(method: &MethodModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(method.docstring.as_deref(), "  "),
            name: js_member_identifier(&method.name),
            params: render_dts_params(&method.arguments)?,
            return_type: render_return_type(method.return_type.as_ref(), method.is_async)?,
        })
    }
}

fn render_dts_callback_interface_fragment(
    callback_interface: &CallbackInterfaceModel,
) -> Result<String> {
    render_trimmed_template(DtsCallbackInterfaceTemplate {
        callback_interface: CallbackInterfaceDtsView::from_callback_interface(callback_interface)?,
    })
}

struct FunctionDtsView {
    doc_comment: String,
    name: String,
    params: String,
    return_type: String,
}

impl FunctionDtsView {
    fn from_function(function: &FunctionModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(function.docstring.as_deref(), ""),
            name: js_member_identifier(&function.name),
            params: render_dts_params(&function.arguments)?,
            return_type: render_return_type(function.return_type.as_ref(), function.is_async)?,
        })
    }
}

fn render_dts_function_fragment(function: &FunctionModel) -> Result<String> {
    render_trimmed_template(DtsFunctionTemplate {
        function: FunctionDtsView::from_function(function)?,
    })
}

struct FlatEnumDtsView {
    doc_comment: String,
    name: String,
    variants: Vec<FlatEnumVariantDtsView>,
}

impl FlatEnumDtsView {
    fn from_enum(enum_def: &EnumModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(enum_def.docstring.as_deref(), ""),
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
    doc_comment: String,
    property_name: String,
    value_literal: String,
}

impl FlatEnumVariantDtsView {
    fn from_variant(variant: &VariantModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(variant.docstring.as_deref(), "  "),
            property_name: quoted_property_name(&variant.name)?,
            value_literal: json_string_literal(&variant.name)?,
        })
    }
}

fn render_dts_flat_enum_fragment(enum_def: &EnumModel) -> Result<String> {
    render_trimmed_template(DtsFlatEnumTemplate {
        enum_def: FlatEnumDtsView::from_enum(enum_def)?,
    })
}

struct TaggedEnumDtsView {
    doc_comment: String,
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
            doc_comment: render_doc_comment(enum_def.docstring.as_deref(), ""),
            name: enum_def.name.clone(),
            variant_types,
            variants,
        })
    }
}

struct TaggedEnumVariantDtsView {
    doc_comment: String,
    type_name: String,
    tag_literal: String,
    fields: Vec<FieldDtsView>,
    constructor_doc_comment: String,
    constructor_name: String,
    constructor_params: String,
}

impl TaggedEnumVariantDtsView {
    fn from_variant(enum_name: &str, variant: &VariantModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(variant.docstring.as_deref(), ""),
            type_name: variant_type_name(enum_name, &variant.name),
            tag_literal: json_string_literal(&variant.name)?,
            fields: variant
                .fields
                .iter()
                .map(FieldDtsView::from_field)
                .collect::<Result<_>>()?,
            constructor_doc_comment: render_doc_comment(variant.docstring.as_deref(), "  "),
            constructor_name: js_member_identifier(&variant.name),
            constructor_params: render_dts_fields_as_params(&variant.fields)?,
        })
    }
}

fn render_dts_tagged_enum_fragment(enum_def: &EnumModel) -> Result<String> {
    render_trimmed_template(DtsTaggedEnumTemplate {
        enum_def: TaggedEnumDtsView::from_enum(enum_def)?,
    })
}

struct ErrorDtsView {
    doc_comment: String,
    name: String,
    variants: Vec<ErrorVariantDtsView>,
}

impl ErrorDtsView {
    fn from_error(error: &ErrorModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(error.docstring.as_deref(), ""),
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
    doc_comment: String,
    class_name: String,
    tag_literal: String,
    fields: Vec<FieldDtsView>,
    constructor_params: String,
}

impl ErrorVariantDtsView {
    fn from_variant(error: &ErrorModel, variant: &VariantModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(variant.docstring.as_deref(), ""),
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
    render_trimmed_template(DtsErrorTemplate {
        error: ErrorDtsView::from_error(error)?,
    })
}

struct ObjectDtsView {
    doc_comment: String,
    name: String,
    has_primary_constructor: bool,
    primary_constructor_doc_comment: String,
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
            doc_comment: render_doc_comment(object.docstring.as_deref(), ""),
            name: object.name.clone(),
            has_primary_constructor: primary_constructor.is_some(),
            primary_constructor_doc_comment: render_doc_comment(
                primary_constructor.and_then(|constructor| constructor.docstring.as_deref()),
                "  ",
            ),
            primary_constructor_params: primary_constructor
                .map(|constructor| render_dts_params(&constructor.arguments))
                .transpose()?
                .unwrap_or_default(),
            constructors: object
                .constructors
                .iter()
                .filter(|constructor| !constructor.is_primary || constructor.is_async)
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
    doc_comment: String,
    name: String,
    params: String,
    return_type: String,
}

impl ConstructorDtsView {
    fn from_constructor(object_name: &str, constructor: &ConstructorModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(constructor.docstring.as_deref(), "  "),
            name: js_member_identifier(&constructor.name),
            params: render_dts_params(&constructor.arguments)?,
            return_type: render_named_return_type(object_name, constructor.is_async),
        })
    }
}

struct ObjectMethodDtsView {
    doc_comment: String,
    name: String,
    params: String,
    return_type: String,
}

impl ObjectMethodDtsView {
    fn from_method(method: &MethodModel) -> Result<Self> {
        Ok(Self {
            doc_comment: render_doc_comment(method.docstring.as_deref(), "  "),
            name: js_member_identifier(&method.name),
            params: render_dts_params(&method.arguments)?,
            return_type: render_return_type(method.return_type.as_ref(), method.is_async)?,
        })
    }
}

fn render_dts_object_fragment(object: &ObjectModel) -> Result<String> {
    render_trimmed_template(DtsObjectTemplate {
        object: ObjectDtsView::from_object(object)?,
    })
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
#[template(path = "api/js/record-converter.js.j2", escape = "none")]
struct JsRecordConverterTemplate {
    converter: RecordConverterJsView,
}

#[derive(Template)]
#[template(path = "api/js/flat-enum-converter.js.j2", escape = "none")]
struct JsFlatEnumConverterTemplate {
    converter: FlatEnumConverterJsView,
}

#[derive(Template)]
#[template(path = "api/js/tagged-enum-converter.js.j2", escape = "none")]
struct JsTaggedEnumConverterTemplate {
    converter: TaggedEnumConverterJsView,
}

#[derive(Template)]
#[template(path = "api/js/error-converter.js.j2", escape = "none")]
struct JsErrorConverterTemplate {
    converter: ErrorConverterJsView,
}

#[derive(Template)]
#[template(path = "api/js/callback-interface-converter.js.j2", escape = "none")]
struct JsCallbackInterfaceConverterTemplate {
    converter: CallbackInterfaceConverterJsView,
}

#[derive(Template)]
#[template(path = "api/js/runtime-helpers.js.j2", escape = "none")]
struct JsRuntimeHelpersTemplate {
    helpers: RuntimeHelpersJsView,
}

#[derive(Template)]
#[template(path = "api/js/async-rust-future-helpers.js.j2", escape = "none")]
struct JsAsyncRustFutureHelpersTemplate;

#[derive(Template)]
#[template(path = "api/js/runtime-hooks.js.j2", escape = "none")]
struct JsRuntimeHooksTemplate {
    hooks: RuntimeHooksJsView,
}

#[derive(Template)]
#[template(
    path = "api/js/sync-callback-vtable-registration.js.j2",
    escape = "none"
)]
struct JsSyncCallbackVtableRegistrationTemplate {
    registration: SyncCallbackVtableRegistrationJsView,
}

#[derive(Template)]
#[template(
    path = "api/js/async-callback-vtable-registration.js.j2",
    escape = "none"
)]
struct JsAsyncCallbackVtableRegistrationTemplate {
    registration: AsyncCallbackVtableRegistrationJsView,
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
