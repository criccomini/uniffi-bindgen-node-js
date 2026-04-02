use std::collections::BTreeSet;
use std::fs;

use anyhow::{Context, Result, bail};
use askama::Template;
use camino::{Utf8Path, Utf8PathBuf};
use uniffi_bindgen::interface::ComponentInterface;

use super::{
    api::{RenderedComponentApi, build_public_api_ir, render_public_api},
    ffi::{RenderedComponentFfi, render_component_ffi},
    layout::GeneratedPackageLayout,
    runtime::emit_runtime_files,
    spec::NodePackageSpec,
    templates::{
        ComponentDtsTemplate, ComponentJsTemplate, PackageIndexDtsTemplate, PackageIndexJsTemplate,
        PackageJsonTemplate, StringTemplate, json_string, rendered_file, write_files,
    },
};

#[derive(Debug, Clone)]
struct GeneratedPackage {
    layout: GeneratedPackageLayout,
    spec: NodePackageSpec,
    public_api: RenderedComponentApi,
    ffi_api: RenderedComponentFfi,
}

impl GeneratedPackage {
    fn from_parts(
        out_dir: &Utf8Path,
        lib_source: &Utf8Path,
        ci: &ComponentInterface,
        package_spec: &NodePackageSpec,
    ) -> Result<Self> {
        let public_api = render_public_api(&build_public_api_ir(ci)?)?;
        let layout = GeneratedPackageLayout::from_parts(out_dir, lib_source, ci, package_spec)?;
        let ffi_api = render_component_ffi(
            ci,
            &package_spec.library_name,
            &layout.native_library.file_name,
            layout.native_library.package_relative_path.as_str(),
            package_spec.bundled_prebuilds,
            package_spec.manual_load,
        )?;

        Ok(Self {
            layout,
            spec: package_spec.clone(),
            public_api,
            ffi_api,
        })
    }

    fn ensure_output_dirs(&self) -> Result<()> {
        self.layout.ensure_output_dirs()
    }

    fn write_package_files(&self) -> Result<()> {
        let template_context = TemplateContext::from_package(self)?;
        self.write_package_metadata_files(&template_context)?;
        let mut files = self.component_api_files(&template_context)?;
        files.extend(self.component_ffi_files()?);
        write_files(files)?;
        self.write_runtime_files()?;
        self.stage_native_library()?;

        Ok(())
    }

    fn write_package_metadata_files(&self, template_context: &TemplateContext) -> Result<()> {
        write_files(self.package_metadata_files(template_context)?)
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
                    manual_load: self.spec.manual_load,
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
        let component_dts_imports = ComponentDtsImports::from_public_api(&self.public_api.dts);
        Ok(vec![
            rendered_file(
                self.layout.component_js_path(),
                ComponentJsTemplate {
                    namespace: self.layout.namespace.clone(),
                    namespace_doc_comment: self.public_api.namespace_doc_comment.clone(),
                    namespace_json: template_context.namespace_json.clone(),
                    package_name_json: template_context.package_name_json.clone(),
                    library_name_json: template_context.library_name_json.clone(),
                    node_engine_json: template_context.node_engine_json.clone(),
                    bundled_prebuilds: template_context.bundled_prebuilds,
                    manual_load: self.spec.manual_load,
                    needs_koffi: component_js_imports.needs_koffi,
                    ffi_imports: component_js_imports.ffi_imports,
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
                    manual_load: self.spec.manual_load,
                    needs_uniffi_object_base: component_dts_imports.needs_uniffi_object_base,
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
        emit_runtime_files(&self.layout, &self.direct_runtime_modules())
    }

    fn stage_native_library(&self) -> Result<()> {
        let source_path = self.layout.native_library.source_path.as_std_path();
        let output_path = self.layout.native_library.output_path.as_std_path();

        if existing_staged_library_matches_source(source_path, output_path)? {
            return Ok(());
        }

        if output_path.exists() {
            if output_path.is_dir() {
                bail!(
                    "failed to stage native library '{}' into '{}': destination is an existing directory",
                    self.layout.native_library.source_path,
                    self.layout.native_library.output_path
                );
            }

            fs::remove_file(output_path).with_context(|| {
                format!(
                    "failed to replace existing staged native library '{}'",
                    self.layout.native_library.output_path
                )
            })?;
        }

        fs::copy(source_path, output_path).with_context(|| {
            format!(
                "failed to stage native library '{}' into '{}'",
                self.layout.native_library.source_path, self.layout.native_library.output_path
            )
        })?;

        Ok(())
    }
}

impl GeneratedPackage {
    fn direct_runtime_modules(&self) -> BTreeSet<&'static str> {
        let component_js_imports = ComponentJsImports::from_public_api(&self.public_api.js);
        let component_dts_imports = ComponentDtsImports::from_public_api(&self.public_api.dts);
        let mut modules = BTreeSet::from(["errors", "ffi-types"]);

        if !component_js_imports.ffi_converter_imports.is_empty() {
            modules.insert("ffi-converters");
        }
        if !component_js_imports.async_rust_call_imports.is_empty() {
            modules.insert("async-rust-call");
        }
        if !component_js_imports.callback_imports.is_empty() {
            modules.insert("callbacks");
        }
        if !component_js_imports.object_imports.is_empty()
            || component_dts_imports.needs_uniffi_object_base
        {
            modules.insert("objects");
        }
        if !component_js_imports.rust_call_imports.is_empty() {
            modules.insert("rust-call");
        }

        modules
    }
}

fn existing_staged_library_matches_source(
    source_path: &std::path::Path,
    output_path: &std::path::Path,
) -> Result<bool> {
    if !output_path.exists() {
        return Ok(false);
    }

    let canonical_source = fs::canonicalize(source_path).with_context(|| {
        format!(
            "failed to canonicalize native library source '{}'",
            source_path.display()
        )
    })?;
    let canonical_output = fs::canonicalize(output_path).with_context(|| {
        format!(
            "failed to canonicalize staged native library '{}'",
            output_path.display()
        )
    })?;

    Ok(canonical_source == canonical_output)
}

pub(crate) fn write_generated_package(
    out_dir: &Utf8Path,
    lib_source: &Utf8Path,
    ci: &ComponentInterface,
    package_spec: &NodePackageSpec,
) -> Result<()> {
    let package = GeneratedPackage::from_parts(out_dir, lib_source, ci, package_spec)?;
    package.ensure_output_dirs()?;
    package.write_package_files()
}

pub(crate) struct ComponentJsImports {
    pub(crate) needs_koffi: bool,
    pub(crate) ffi_imports: Vec<String>,
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
            needs_koffi: public_api_js.contains("koffi."),
            ffi_imports: collect_used_js_imports(
                public_api_js,
                &[
                    "configureRuntimeHooks",
                    "ffiFunctions",
                    "getFfiBindings",
                    "getFfiTypes",
                ],
            ),
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

struct ComponentDtsImports {
    needs_uniffi_object_base: bool,
}

impl ComponentDtsImports {
    fn from_public_api(public_api_dts: &str) -> Self {
        Self {
            needs_uniffi_object_base: public_api_dts.contains("UniffiObjectBase"),
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
    library_name_json: String,
    node_engine_json: String,
    bundled_prebuilds: bool,
}

impl TemplateContext {
    fn from_package(package: &GeneratedPackage) -> Result<Self> {
        Ok(Self {
            namespace_json: json_string(&package.layout.namespace)?,
            package_name_json: json_string(&package.layout.package_name)?,
            library_name_json: json_string(&package.spec.library_name)?,
            node_engine_json: json_string(&package.spec.node_engine)?,
            bundled_prebuilds: package.spec.bundled_prebuilds,
        })
    }
}
