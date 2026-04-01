use anyhow::{Result, anyhow};
use askama::Template;
use camino::{Utf8Path, Utf8PathBuf};
use uniffi_bindgen::Component;

use super::{
    api::{RenderedComponentApi, build_public_api_ir, render_public_api},
    ffi::{RenderedComponentFfi, render_component_ffi},
    layout::GeneratedPackageLayout,
    runtime::emit_runtime_files,
    templates::{
        ComponentDtsTemplate, ComponentJsTemplate, PackageIndexDtsTemplate, PackageIndexJsTemplate,
        PackageJsonTemplate, StringTemplate, json_optional_string, json_string, rendered_file,
        write_files,
    },
};
use crate::node_v2::config::NodeBindingGeneratorConfig;

#[derive(Debug, Clone)]
struct GeneratedPackage {
    layout: GeneratedPackageLayout,
    cdylib_name: String,
    node_engine: String,
    lib_path_literal: Option<String>,
    bundled_prebuilds: bool,
    manual_load: bool,
    public_api: RenderedComponentApi,
    ffi_api: RenderedComponentFfi,
}

impl GeneratedPackage {
    fn from_component(
        out_dir: &Utf8Path,
        component: &Component<NodeBindingGeneratorConfig>,
    ) -> Result<Self> {
        let public_api = render_public_api(&build_public_api_ir(&component.ci)?)?;
        let layout = GeneratedPackageLayout::from_component(out_dir, component)?;
        let cdylib_name = component
            .config
            .cdylib_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("node bindings generation requires a cdylib_name"))?;
        let ffi_api = render_component_ffi(
            &component.ci,
            cdylib_name,
            component.config.lib_path_literal.as_deref(),
            component.config.bundled_prebuilds,
            component.config.manual_load,
        )?;

        Ok(Self {
            layout,
            cdylib_name: cdylib_name.to_string(),
            node_engine: component.config.node_engine.trim().to_string(),
            lib_path_literal: component.config.lib_path_literal.clone(),
            bundled_prebuilds: component.config.bundled_prebuilds,
            manual_load: component.config.manual_load,
            public_api,
            ffi_api,
        })
    }

    fn ensure_root_dir(&self) -> Result<()> {
        self.layout.ensure_root_dir()
    }

    fn write_package_files(&self) -> Result<()> {
        let template_context = TemplateContext::from_package(self)?;
        write_files(self.package_files(&template_context)?)?;
        self.write_runtime_files()?;

        Ok(())
    }

    fn package_files(
        &self,
        template_context: &TemplateContext,
    ) -> Result<Vec<(Utf8PathBuf, String)>> {
        let mut files = self.package_metadata_files(template_context)?;
        files.extend(self.component_api_files(template_context)?);
        files.extend(self.component_ffi_files()?);
        Ok(files)
    }

    fn package_metadata_files(
        &self,
        template_context: &TemplateContext,
    ) -> Result<Vec<(Utf8PathBuf, String)>> {
        Ok(vec![
            rendered_file(
                self.layout.package_json_path(),
                PackageJsonTemplate {
                    package_name_json: template_context.package_name_json.clone(),
                    node_engine_json: template_context.node_engine_json.clone(),
                }
                .render(),
            )?,
            rendered_file(
                self.layout.index_js_path(),
                PackageIndexJsTemplate {
                    namespace: self.layout.namespace.clone(),
                }
                .render(),
            )?,
            rendered_file(
                self.layout.index_dts_path(),
                PackageIndexDtsTemplate {
                    namespace: self.layout.namespace.clone(),
                }
                .render(),
            )?,
        ])
    }

    fn component_api_files(
        &self,
        template_context: &TemplateContext,
    ) -> Result<Vec<(Utf8PathBuf, String)>> {
        let component_js_imports = ComponentJsImports::from_public_api(&self.public_api.js);
        Ok(vec![
            rendered_file(
                self.layout.component_js_path(),
                ComponentJsTemplate {
                    namespace: self.layout.namespace.clone(),
                    namespace_doc_comment: self.public_api.namespace_doc_comment.clone(),
                    namespace_json: template_context.namespace_json.clone(),
                    package_name_json: template_context.package_name_json.clone(),
                    cdylib_name_json: template_context.cdylib_name_json.clone(),
                    node_engine_json: template_context.node_engine_json.clone(),
                    lib_path_literal_json: template_context.lib_path_literal_json.clone(),
                    bundled_prebuilds: template_context.bundled_prebuilds,
                    manual_load: self.manual_load,
                    ffi_types_imports: component_js_imports.ffi_types_imports,
                    ffi_converter_imports: component_js_imports.ffi_converter_imports,
                    error_imports: component_js_imports.error_imports,
                    async_rust_call_imports: component_js_imports.async_rust_call_imports,
                    callback_imports: component_js_imports.callback_imports,
                    object_imports: component_js_imports.object_imports,
                    rust_call_imports: component_js_imports.rust_call_imports,
                    public_api_js: self.public_api.js.clone(),
                }
                .render(),
            )?,
            rendered_file(
                self.layout.component_dts_path(),
                ComponentDtsTemplate {
                    namespace: self.layout.namespace.clone(),
                    namespace_doc_comment: self.public_api.namespace_doc_comment.clone(),
                    manual_load: self.manual_load,
                    public_api_dts: self.public_api.dts.clone(),
                }
                .render(),
            )?,
        ])
    }

    fn component_ffi_files(&self) -> Result<Vec<(Utf8PathBuf, String)>> {
        Ok(vec![
            rendered_file(
                self.layout.component_ffi_js_path(),
                StringTemplate {
                    contents: self.ffi_api.js.clone(),
                }
                .render(),
            )?,
            rendered_file(
                self.layout.component_ffi_dts_path(),
                StringTemplate {
                    contents: self.ffi_api.dts.clone(),
                }
                .render(),
            )?,
        ])
    }

    fn write_runtime_files(&self) -> Result<()> {
        emit_runtime_files(&self.layout)
    }
}

pub(crate) fn write_generated_package(
    out_dir: &Utf8Path,
    component: &Component<NodeBindingGeneratorConfig>,
) -> Result<()> {
    let package = GeneratedPackage::from_component(out_dir, component)?;
    package.ensure_root_dir()?;
    package.write_package_files()
}

pub(crate) struct ComponentJsImports {
    pub(crate) ffi_types_imports: Vec<String>,
    pub(crate) ffi_converter_imports: Vec<String>,
    pub(crate) error_imports: Vec<String>,
    pub(crate) async_rust_call_imports: Vec<String>,
    pub(crate) callback_imports: Vec<String>,
    pub(crate) object_imports: Vec<String>,
    pub(crate) rust_call_imports: Vec<String>,
}

impl ComponentJsImports {
    pub(crate) fn from_public_api(public_api_js: &str) -> Self {
        Self {
            ffi_types_imports: collect_used_js_imports(
                public_api_js,
                &["createForeignBytes", "EMPTY_RUST_BUFFER", "RustBufferValue"],
            ),
            ffi_converter_imports: collect_used_js_imports(
                public_api_js,
                &[
                    "AbstractFfiConverterByteArray",
                    "FfiConverterArray",
                    "FfiConverterBool",
                    "FfiConverterBytes",
                    "FfiConverterDuration",
                    "FfiConverterFloat32",
                    "FfiConverterFloat64",
                    "FfiConverterInt8",
                    "FfiConverterInt16",
                    "FfiConverterInt32",
                    "FfiConverterInt64",
                    "FfiConverterMap",
                    "FfiConverterOptional",
                    "FfiConverterString",
                    "FfiConverterTimestamp",
                    "FfiConverterUInt8",
                    "FfiConverterUInt16",
                    "FfiConverterUInt32",
                    "FfiConverterUInt64",
                ],
            ),
            error_imports: collect_used_js_imports(public_api_js, &["UnexpectedEnumCase"]),
            async_rust_call_imports: collect_used_js_imports(
                public_api_js,
                &["rustCallAsync", "rustFutureContinuationCallback"],
            ),
            callback_imports: collect_used_js_imports(
                public_api_js,
                &[
                    "clearPendingForeignFutures",
                    "createCallbackRegistry",
                    "freePendingForeignFuture",
                    "invokeAsyncCallbackMethod",
                    "invokeCallbackMethod",
                ],
            ),
            object_imports: collect_used_js_imports(
                public_api_js,
                &[
                    "createObjectConverter",
                    "createObjectFactory",
                    "UniffiObjectBase",
                    "UNIFFI_OBJECT_HANDLE_SIZE",
                ],
            ),
            rust_call_imports: collect_used_js_imports(
                public_api_js,
                &["CALL_SUCCESS", "UniffiRustCaller", "createRustCallStatus"],
            ),
        }
    }
}

fn collect_used_js_imports(source: &str, identifiers: &[&str]) -> Vec<String> {
    identifiers
        .iter()
        .filter(|identifier| source.contains(**identifier))
        .map(|identifier| (*identifier).to_string())
        .collect()
}

#[derive(Debug, Clone)]
struct TemplateContext {
    namespace_json: String,
    package_name_json: String,
    cdylib_name_json: String,
    node_engine_json: String,
    lib_path_literal_json: String,
    bundled_prebuilds: bool,
}

impl TemplateContext {
    fn from_package(package: &GeneratedPackage) -> Result<Self> {
        Ok(Self {
            namespace_json: json_string(&package.layout.namespace)?,
            package_name_json: json_string(&package.layout.package_name)?,
            cdylib_name_json: json_string(&package.cdylib_name)?,
            node_engine_json: json_string(&package.node_engine)?,
            lib_path_literal_json: json_optional_string(package.lib_path_literal.as_deref())?,
            bundled_prebuilds: package.bundled_prebuilds,
        })
    }
}
