use anyhow::Result;
use askama::Template;

use super::{
    CallbackInterfaceModel, ComponentModel, FieldModel, MethodModel, RecordModel,
    js_member_identifier, quoted_property_name, render_dts_error, render_dts_flat_enum,
    render_dts_function, render_dts_object, render_dts_params, render_dts_tagged_enum,
    render_public_type, render_return_type,
};

pub(crate) struct PublicApiRenderer<'a> {
    model: &'a ComponentModel,
}

impl<'a> PublicApiRenderer<'a> {
    pub(crate) fn new(model: &'a ComponentModel) -> Self {
        Self { model }
    }

    pub(crate) fn render_js(&self, sections: &[String]) -> Result<String> {
        let _ = self.model;
        Ok(PublicApiJsTemplate {
            contents: sections.join("\n\n"),
        }
        .render()?)
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
                .map(render_dts_flat_enum)
                .collect::<Result<_>>()?,
            tagged_enums: model
                .tagged_enums
                .iter()
                .map(render_dts_tagged_enum)
                .collect::<Result<_>>()?,
            errors: model
                .errors
                .iter()
                .map(render_dts_error)
                .collect::<Result<_>>()?,
            callback_interfaces: model
                .callback_interfaces
                .iter()
                .map(render_dts_callback_interface_fragment)
                .collect::<Result<_>>()?,
            functions: model
                .functions
                .iter()
                .map(render_dts_function)
                .collect::<Result<_>>()?,
            objects: model
                .objects
                .iter()
                .map(render_dts_object)
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

#[derive(Template)]
#[template(source = "{{ contents }}", ext = "txt", escape = "none")]
struct PublicApiJsTemplate {
    contents: String,
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
