use anyhow::Result;
use askama::Template;

use super::ComponentModel;

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

    pub(crate) fn render_dts(&self, sections: &[String]) -> Result<String> {
        let _ = self.model;
        Ok(PublicApiDtsTemplate {
            contents: sections.join("\n\n"),
        }
        .render()?)
    }
}

#[derive(Template)]
#[template(source = "{{ contents }}", ext = "txt", escape = "none")]
struct PublicApiJsTemplate {
    contents: String,
}

#[derive(Template)]
#[template(source = "{{ contents }}", ext = "txt", escape = "none")]
struct PublicApiDtsTemplate {
    contents: String,
}
