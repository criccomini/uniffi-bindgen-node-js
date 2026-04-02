use std::fs;

use anyhow::Result;
use askama::Template;
use camino::Utf8PathBuf;

fn write_contents(path: &Utf8PathBuf, contents: String) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent.as_std_path())?;
    }
    fs::write(path.as_std_path(), contents)?;
    Ok(())
}

pub(crate) fn write_files(files: Vec<(Utf8PathBuf, String)>) -> Result<()> {
    files
        .into_iter()
        .try_for_each(|(path, contents)| write_contents(&path, contents))
}

pub(crate) fn rendered_file(
    path: Utf8PathBuf,
    contents: std::result::Result<String, askama::Error>,
) -> Result<(Utf8PathBuf, String)> {
    Ok((path, contents?))
}

pub(crate) fn json_string(value: &str) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

#[derive(Template)]
#[template(path = "package/package.json.j2", escape = "none")]
pub(crate) struct PackageJsonTemplate {
    pub(crate) package_name_json: String,
    pub(crate) node_engine_json: String,
}

#[derive(Template)]
#[template(path = "package/index.js.j2", escape = "none")]
pub(crate) struct PackageIndexJsTemplate {
    pub(crate) namespace: String,
    pub(crate) manual_load: bool,
}

#[derive(Template)]
#[template(path = "package/index.d.ts.j2", escape = "none")]
pub(crate) struct PackageIndexDtsTemplate {
    pub(crate) namespace: String,
}

#[derive(Template)]
#[template(path = "component/component.js.j2", escape = "none")]
pub(crate) struct ComponentJsTemplate {
    pub(crate) namespace: String,
    pub(crate) namespace_doc_comment: String,
    pub(crate) namespace_json: String,
    pub(crate) package_name_json: String,
    pub(crate) cdylib_name_json: String,
    pub(crate) node_engine_json: String,
    pub(crate) bundled_prebuilds: bool,
    pub(crate) manual_load: bool,
    pub(crate) needs_koffi: bool,
    pub(crate) ffi_imports: Vec<String>,
    pub(crate) ffi_types_imports: Vec<String>,
    pub(crate) ffi_converter_imports: Vec<String>,
    pub(crate) error_imports: Vec<String>,
    pub(crate) async_rust_call_imports: Vec<String>,
    pub(crate) callback_imports: Vec<String>,
    pub(crate) object_imports: Vec<String>,
    pub(crate) rust_call_imports: Vec<String>,
    pub(crate) public_api_js: String,
}

#[derive(Template)]
#[template(path = "component/component.d.ts.j2", escape = "none")]
pub(crate) struct ComponentDtsTemplate {
    pub(crate) namespace: String,
    pub(crate) namespace_doc_comment: String,
    pub(crate) manual_load: bool,
    pub(crate) needs_uniffi_object_base: bool,
    pub(crate) public_api_dts: String,
}

#[derive(Template)]
#[template(source = "{{ contents }}", ext = "txt", escape = "none")]
pub(crate) struct StringTemplate {
    pub(crate) contents: String,
}
