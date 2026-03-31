use std::fs;

use anyhow::{Result, anyhow, bail};
use askama::Template;
use camino::Utf8PathBuf;
use serde::Deserialize;
use uniffi_bindgen::{BindingGenerator, Component, GenerationSettings};

mod api;
mod ffi;

use self::{
    api::{ComponentModel, RenderedComponentApi},
    ffi::{RenderedComponentFfi, render_component_ffi},
};

#[derive(Debug, Clone)]
pub struct NodeBindingGenerator {
    cli_overrides: NodeBindingCliOverrides,
}

impl NodeBindingGenerator {
    pub fn new(cli_overrides: NodeBindingCliOverrides) -> Self {
        Self { cli_overrides }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NodeBindingCliOverrides {
    package_name: Option<String>,
    cdylib_name: Option<String>,
    node_engine: Option<String>,
    lib_path_literal: Option<String>,
    bundled_prebuilds: bool,
    manual_load: bool,
    config_overrides: Vec<NodeBindingConfigOverride>,
}

impl NodeBindingCliOverrides {
    pub fn from_parts(
        package_name: Option<String>,
        cdylib_name: Option<String>,
        node_engine: Option<String>,
        lib_path_literal: Option<String>,
        bundled_prebuilds: bool,
        manual_load: bool,
        config_overrides: Vec<String>,
    ) -> Result<Self> {
        Ok(Self {
            package_name: normalize_optional_value("--package-name", package_name)?,
            cdylib_name: normalize_optional_value("--cdylib-name", cdylib_name)?,
            node_engine: normalize_optional_value("--node-engine", node_engine)?,
            lib_path_literal: normalize_optional_value("--lib-path-literal", lib_path_literal)?,
            bundled_prebuilds,
            manual_load,
            config_overrides: config_overrides
                .into_iter()
                .map(NodeBindingConfigOverride::parse)
                .collect::<Result<_>>()?,
        })
    }

    fn apply_to(&self, config: &mut NodeBindingGeneratorConfig) {
        for override_entry in &self.config_overrides {
            override_entry.apply_to(config);
        }

        apply_optional_string_override(&mut config.package_name, self.package_name.as_ref());
        apply_optional_string_override(&mut config.cdylib_name, self.cdylib_name.as_ref());
        apply_optional_value_override(&mut config.node_engine, self.node_engine.as_ref());
        apply_optional_string_override(
            &mut config.lib_path_literal,
            self.lib_path_literal.as_ref(),
        );
        enable_override(&mut config.bundled_prebuilds, self.bundled_prebuilds);
        enable_override(&mut config.manual_load, self.manual_load);
    }
}

#[derive(Debug, Clone)]
enum NodeBindingConfigOverride {
    PackageName(String),
    CdylibName(String),
    NodeEngine(String),
    LibPathLiteral(String),
    BundledPrebuilds(bool),
    ManualLoad(bool),
    ModuleFormat(String),
    Commonjs(bool),
}

impl NodeBindingConfigOverride {
    fn parse(raw: String) -> Result<Self> {
        let (key, value) = parse_override_parts(&raw)?;
        let normalized_key = normalize_override_key(key);

        if let Some(override_entry) = Self::parse_string_override(&normalized_key, &value) {
            return Ok(override_entry);
        }
        if let Some(override_entry) = Self::parse_boolean_override(&normalized_key, &raw, &value)? {
            return Ok(override_entry);
        }

        bail!("unsupported --config-override key '{key}'");
    }

    fn apply_to(&self, config: &mut NodeBindingGeneratorConfig) {
        match self {
            Self::PackageName(value) => config.package_name = Some(value.clone()),
            Self::CdylibName(value) => config.cdylib_name = Some(value.clone()),
            Self::NodeEngine(value) => config.node_engine = value.clone(),
            Self::LibPathLiteral(value) => config.lib_path_literal = Some(value.clone()),
            Self::BundledPrebuilds(value) => config.bundled_prebuilds = *value,
            Self::ManualLoad(value) => config.manual_load = *value,
            Self::ModuleFormat(value) => config.module_format = Some(value.clone()),
            Self::Commonjs(value) => config.commonjs = Some(*value),
        }
    }

    fn parse_string_override(normalized_key: &str, value: &str) -> Option<Self> {
        match normalized_key {
            "package_name" => Some(Self::PackageName(value.to_string())),
            "cdylib_name" => Some(Self::CdylibName(value.to_string())),
            "node_engine" => Some(Self::NodeEngine(value.to_string())),
            "lib_path_literal" => Some(Self::LibPathLiteral(value.to_string())),
            "module_format" => Some(Self::ModuleFormat(value.to_string())),
            _ => None,
        }
    }

    fn parse_boolean_override(
        normalized_key: &str,
        raw: &str,
        value: &str,
    ) -> Result<Option<Self>> {
        let parsed = match normalized_key {
            "bundled_prebuilds" => Some(Self::BundledPrebuilds(parse_bool_override(raw, value)?)),
            "manual_load" => Some(Self::ManualLoad(parse_bool_override(raw, value)?)),
            "commonjs" => Some(Self::Commonjs(parse_bool_override(raw, value)?)),
            _ => None,
        };

        Ok(parsed)
    }
}

fn parse_bool_override(raw: &str, value: &str) -> Result<bool> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => bail!("invalid boolean override '{raw}': expected true or false"),
    }
}

fn normalize_optional_value(flag: &str, value: Option<String>) -> Result<Option<String>> {
    value
        .map(|value| normalize_required_value(flag, &value))
        .transpose()
}

fn normalize_required_value(flag: &str, value: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        bail!("{flag} cannot be empty");
    }
    Ok(trimmed.to_string())
}

fn parse_override_parts(raw: &str) -> Result<(&str, String)> {
    let (raw_key, raw_value) = raw
        .split_once('=')
        .ok_or_else(|| anyhow!("invalid --config-override '{raw}': expected KEY=VALUE"))?;
    let key = raw_key.trim();
    if key.is_empty() {
        bail!("invalid --config-override '{raw}': missing key before '='");
    }

    Ok((
        key,
        normalize_required_value("--config-override", raw_value.trim())?,
    ))
}

fn normalize_override_key(key: &str) -> String {
    key.strip_prefix("bindings.node.")
        .unwrap_or(key)
        .replace('-', "_")
}

fn apply_optional_string_override(target: &mut Option<String>, value: Option<&String>) {
    if let Some(value) = value {
        *target = Some(value.clone());
    }
}

fn apply_optional_value_override(target: &mut String, value: Option<&String>) {
    if let Some(value) = value {
        *target = value.clone();
    }
}

fn enable_override(target: &mut bool, enabled: bool) {
    *target |= enabled;
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NodeBindingGeneratorConfig {
    pub package_name: Option<String>,
    pub cdylib_name: Option<String>,
    pub node_engine: String,
    pub lib_path_literal: Option<String>,
    pub bundled_prebuilds: bool,
    pub manual_load: bool,
    #[serde(alias = "module-format")]
    pub module_format: Option<String>,
    pub commonjs: Option<bool>,
}

impl Default for NodeBindingGeneratorConfig {
    fn default() -> Self {
        Self {
            package_name: None,
            cdylib_name: None,
            node_engine: ">=16".to_string(),
            lib_path_literal: None,
            bundled_prebuilds: false,
            manual_load: false,
            module_format: None,
            commonjs: None,
        }
    }
}

impl NodeBindingGeneratorConfig {
    fn validate(&self) -> Result<()> {
        self.validate_required_fields()?;
        self.validate_library_loading()?;
        self.validate_module_settings()?;
        Ok(())
    }

    fn validate_required_fields(&self) -> Result<()> {
        validate_optional_non_empty("package_name", self.package_name.as_deref())?;
        validate_optional_non_empty("cdylib_name", self.cdylib_name.as_deref())?;
        validate_non_empty("node_engine", &self.node_engine)?;
        validate_optional_non_empty("lib_path_literal", self.lib_path_literal.as_deref())?;
        Ok(())
    }

    fn validate_library_loading(&self) -> Result<()> {
        if self.bundled_prebuilds && self.lib_path_literal.is_some() {
            bail!(
                "node binding bundled_prebuilds cannot be enabled together with lib_path_literal"
            );
        }
        Ok(())
    }

    fn validate_module_settings(&self) -> Result<()> {
        validate_module_format(self.module_format.as_deref())?;
        reject_commonjs(self.commonjs)
    }
}

fn validate_optional_non_empty(field_name: &str, value: Option<&str>) -> Result<()> {
    value
        .map(|value| validate_non_empty(field_name, value))
        .transpose()?;
    Ok(())
}

fn validate_non_empty(field_name: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("node binding {field_name} cannot be empty");
    }

    Ok(())
}

fn validate_module_format(module_format: Option<&str>) -> Result<()> {
    let Some(module_format) = module_format else {
        return Ok(());
    };

    let normalized = module_format.trim();
    if normalized.is_empty() {
        bail!("node binding module_format cannot be empty");
    }
    if normalized.eq_ignore_ascii_case("esm") {
        return Ok(());
    }
    if normalized.eq_ignore_ascii_case("commonjs") || normalized.eq_ignore_ascii_case("cjs") {
        bail!("node bindings v1 are ESM-only; CommonJS output is not supported");
    }

    bail!("unsupported node binding module_format '{normalized}': v1 only supports 'esm'");
}

fn reject_commonjs(commonjs: Option<bool>) -> Result<()> {
    if commonjs == Some(true) {
        bail!("node bindings v1 are ESM-only; CommonJS output is not supported");
    }

    Ok(())
}

impl BindingGenerator for NodeBindingGenerator {
    type Config = NodeBindingGeneratorConfig;

    fn new_config(&self, _root_toml: &toml::value::Value) -> Result<Self::Config> {
        Ok(
            match _root_toml
                .get("bindings")
                .and_then(|bindings| bindings.get("node"))
            {
                Some(value) => value.clone().try_into()?,
                None => Self::Config::default(),
            },
        )
    }

    fn update_component_configs(
        &self,
        settings: &GenerationSettings,
        components: &mut Vec<Component<Self::Config>>,
    ) -> Result<()> {
        for component in components {
            if component.config.package_name.is_none() {
                component.config.package_name = Some(component.ci.namespace().to_string());
            }
            if component.config.cdylib_name.is_none() {
                component.config.cdylib_name = settings.cdylib.clone();
            }
            self.cli_overrides.apply_to(&mut component.config);
            component.config.validate()?;
        }
        Ok(())
    }

    fn write_bindings(
        &self,
        settings: &GenerationSettings,
        components: &[Component<Self::Config>],
    ) -> Result<()> {
        let component = match components {
            [component] => component,
            [] => bail!("node bindings generation did not receive a UniFFI component"),
            _ => bail!(
                "node bindings generation emits one npm package per invocation; re-run with --crate-name to select a single crate"
            ),
        };

        let package = GeneratedPackage::from_component(settings, component)?;
        package.ensure_root_dir()?;
        package.write_package_files()?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GeneratedPackageLayout {
    root_dir: Utf8PathBuf,
    namespace: String,
    package_name: String,
}

impl GeneratedPackageLayout {
    fn from_component(
        settings: &GenerationSettings,
        component: &Component<NodeBindingGeneratorConfig>,
    ) -> Result<Self> {
        let namespace = component.ci.namespace().trim();
        if namespace.is_empty() {
            bail!("node bindings generation requires a non-empty UniFFI namespace");
        }

        let package_name = component
            .config
            .package_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("node bindings generation requires a package_name"))?;

        Ok(Self {
            root_dir: settings.out_dir.clone(),
            namespace: namespace.to_string(),
            package_name: package_name.to_string(),
        })
    }

    fn ensure_root_dir(&self) -> Result<()> {
        fs::create_dir_all(self.root_dir.as_std_path())?;
        Ok(())
    }

    fn package_json_path(&self) -> Utf8PathBuf {
        self.root_dir.join("package.json")
    }

    fn index_js_path(&self) -> Utf8PathBuf {
        self.root_dir.join("index.js")
    }

    fn index_dts_path(&self) -> Utf8PathBuf {
        self.root_dir.join("index.d.ts")
    }

    fn component_js_path(&self) -> Utf8PathBuf {
        self.root_dir.join(format!("{}.js", self.namespace))
    }

    fn component_dts_path(&self) -> Utf8PathBuf {
        self.root_dir.join(format!("{}.d.ts", self.namespace))
    }

    fn component_ffi_js_path(&self) -> Utf8PathBuf {
        self.root_dir.join(format!("{}-ffi.js", self.namespace))
    }

    fn component_ffi_dts_path(&self) -> Utf8PathBuf {
        self.root_dir.join(format!("{}-ffi.d.ts", self.namespace))
    }

    fn runtime_path(&self, file_name: &str) -> Utf8PathBuf {
        self.root_dir.join("runtime").join(file_name)
    }
}

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
        settings: &GenerationSettings,
        component: &Component<NodeBindingGeneratorConfig>,
    ) -> Result<Self> {
        let public_api = ComponentModel::from_ci(&component.ci)?.render_public_api()?;
        let layout = GeneratedPackageLayout::from_component(settings, component)?;
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
        write_files(runtime_files(&self.layout)?)
    }
}

struct ComponentJsImports {
    ffi_types_imports: Vec<String>,
    ffi_converter_imports: Vec<String>,
    error_imports: Vec<String>,
    async_rust_call_imports: Vec<String>,
    callback_imports: Vec<String>,
    object_imports: Vec<String>,
    rust_call_imports: Vec<String>,
}

impl ComponentJsImports {
    fn from_public_api(public_api_js: &str) -> Self {
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

fn write_contents(path: &Utf8PathBuf, contents: String) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent.as_std_path())?;
    }
    fs::write(path.as_std_path(), contents)?;
    Ok(())
}

fn write_files(files: Vec<(Utf8PathBuf, String)>) -> Result<()> {
    files
        .into_iter()
        .try_for_each(|(path, contents)| write_contents(&path, contents))
}

fn rendered_file(
    path: Utf8PathBuf,
    contents: std::result::Result<String, askama::Error>,
) -> Result<(Utf8PathBuf, String)> {
    Ok((path, contents?))
}

type TemplateRenderResult = std::result::Result<String, askama::Error>;
type RuntimeTemplateRenderer = fn() -> TemplateRenderResult;

struct RuntimeModuleTemplateSet {
    stem: &'static str,
    js: RuntimeTemplateRenderer,
    dts: RuntimeTemplateRenderer,
}

fn runtime_files(layout: &GeneratedPackageLayout) -> Result<Vec<(Utf8PathBuf, String)>> {
    runtime_file_contents()?
        .into_iter()
        .map(|(file_name, contents)| Ok((layout.runtime_path(&file_name), contents)))
        .collect::<Result<Vec<_>>>()
}

fn rendered_runtime_file(
    file_name: String,
    contents: TemplateRenderResult,
) -> Result<(String, String)> {
    Ok((file_name, contents?))
}

fn runtime_file_contents() -> Result<Vec<(String, String)>> {
    let mut files = Vec::with_capacity(RUNTIME_MODULE_TEMPLATES.len() * 2);
    for templates in RUNTIME_MODULE_TEMPLATES {
        files.extend(render_runtime_module_files(templates)?);
    }
    Ok(files)
}

fn render_runtime_module_files(
    templates: &RuntimeModuleTemplateSet,
) -> Result<[(String, String); 2]> {
    Ok([
        rendered_runtime_file(format!("{}.js", templates.stem), (templates.js)())?,
        rendered_runtime_file(format!("{}.d.ts", templates.stem), (templates.dts)())?,
    ])
}

fn json_string(value: &str) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

fn json_optional_string(value: Option<&str>) -> Result<String> {
    Ok(serde_json::to_string(&value)?)
}

const RUNTIME_MODULE_TEMPLATES: &[RuntimeModuleTemplateSet] = &[
    RuntimeModuleTemplateSet {
        stem: "errors",
        js: render_runtime_errors_js,
        dts: render_runtime_errors_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "ffi-types",
        js: render_runtime_ffi_types_js,
        dts: render_runtime_ffi_types_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "ffi-converters",
        js: render_runtime_ffi_converters_js,
        dts: render_runtime_ffi_converters_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "rust-call",
        js: render_runtime_rust_call_js,
        dts: render_runtime_rust_call_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "async-rust-call",
        js: render_runtime_async_rust_call_js,
        dts: render_runtime_async_rust_call_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "handle-map",
        js: render_runtime_handle_map_js,
        dts: render_runtime_handle_map_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "callbacks",
        js: render_runtime_callbacks_js,
        dts: render_runtime_callbacks_dts,
    },
    RuntimeModuleTemplateSet {
        stem: "objects",
        js: render_runtime_objects_js,
        dts: render_runtime_objects_dts,
    },
];

fn render_runtime_errors_js() -> TemplateRenderResult {
    RuntimeErrorsJsTemplate {}.render()
}

fn render_runtime_errors_dts() -> TemplateRenderResult {
    RuntimeErrorsDtsTemplate {}.render()
}

fn render_runtime_ffi_types_js() -> TemplateRenderResult {
    RuntimeFfiTypesJsTemplate {}.render()
}

fn render_runtime_ffi_types_dts() -> TemplateRenderResult {
    RuntimeFfiTypesDtsTemplate {}.render()
}

fn render_runtime_ffi_converters_js() -> TemplateRenderResult {
    RuntimeFfiConvertersJsTemplate {}.render()
}

fn render_runtime_ffi_converters_dts() -> TemplateRenderResult {
    RuntimeFfiConvertersDtsTemplate {}.render()
}

fn render_runtime_rust_call_js() -> TemplateRenderResult {
    RuntimeRustCallJsTemplate {}.render()
}

fn render_runtime_rust_call_dts() -> TemplateRenderResult {
    RuntimeRustCallDtsTemplate {}.render()
}

fn render_runtime_async_rust_call_js() -> TemplateRenderResult {
    RuntimeAsyncRustCallJsTemplate {}.render()
}

fn render_runtime_async_rust_call_dts() -> TemplateRenderResult {
    RuntimeAsyncRustCallDtsTemplate {}.render()
}

fn render_runtime_handle_map_js() -> TemplateRenderResult {
    RuntimeHandleMapJsTemplate {}.render()
}

fn render_runtime_handle_map_dts() -> TemplateRenderResult {
    RuntimeHandleMapDtsTemplate {}.render()
}

fn render_runtime_callbacks_js() -> TemplateRenderResult {
    RuntimeCallbacksJsTemplate {}.render()
}

fn render_runtime_callbacks_dts() -> TemplateRenderResult {
    RuntimeCallbacksDtsTemplate {}.render()
}

fn render_runtime_objects_js() -> TemplateRenderResult {
    RuntimeObjectsJsTemplate {}.render()
}

fn render_runtime_objects_dts() -> TemplateRenderResult {
    RuntimeObjectsDtsTemplate {}.render()
}

#[derive(Template)]
#[template(path = "package/package.json.j2", escape = "none")]
struct PackageJsonTemplate {
    package_name_json: String,
    node_engine_json: String,
}

#[derive(Template)]
#[template(path = "package/index.js.j2", escape = "none")]
struct PackageIndexJsTemplate {
    namespace: String,
}

#[derive(Template)]
#[template(path = "package/index.d.ts.j2", escape = "none")]
struct PackageIndexDtsTemplate {
    namespace: String,
}

#[derive(Template)]
#[template(path = "component/component.js.j2", escape = "none")]
struct ComponentJsTemplate {
    namespace: String,
    namespace_doc_comment: String,
    namespace_json: String,
    package_name_json: String,
    cdylib_name_json: String,
    node_engine_json: String,
    lib_path_literal_json: String,
    bundled_prebuilds: bool,
    manual_load: bool,
    ffi_types_imports: Vec<String>,
    ffi_converter_imports: Vec<String>,
    error_imports: Vec<String>,
    async_rust_call_imports: Vec<String>,
    callback_imports: Vec<String>,
    object_imports: Vec<String>,
    rust_call_imports: Vec<String>,
    public_api_js: String,
}

#[derive(Template)]
#[template(path = "component/component.d.ts.j2", escape = "none")]
struct ComponentDtsTemplate {
    namespace: String,
    namespace_doc_comment: String,
    manual_load: bool,
    public_api_dts: String,
}

#[derive(Template)]
#[template(source = "{{ contents }}", ext = "txt", escape = "none")]
struct StringTemplate {
    contents: String,
}

#[derive(Template)]
#[template(path = "runtime/errors.js.j2", escape = "none")]
struct RuntimeErrorsJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/errors.d.ts.j2", escape = "none")]
struct RuntimeErrorsDtsTemplate {}

#[derive(Template)]
#[template(path = "runtime/ffi-types.js.j2", escape = "none")]
struct RuntimeFfiTypesJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/ffi-types.d.ts.j2", escape = "none")]
struct RuntimeFfiTypesDtsTemplate {}

#[derive(Template)]
#[template(path = "runtime/ffi-converters.js.j2", escape = "none")]
struct RuntimeFfiConvertersJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/ffi-converters.d.ts.j2", escape = "none")]
struct RuntimeFfiConvertersDtsTemplate {}

#[derive(Template)]
#[template(path = "runtime/rust-call.js.j2", escape = "none")]
struct RuntimeRustCallJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/rust-call.d.ts.j2", escape = "none")]
struct RuntimeRustCallDtsTemplate {}

#[derive(Template)]
#[template(path = "runtime/async-rust-call.js.j2", escape = "none")]
struct RuntimeAsyncRustCallJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/async-rust-call.d.ts.j2", escape = "none")]
struct RuntimeAsyncRustCallDtsTemplate {}

#[derive(Template)]
#[template(path = "runtime/handle-map.js.j2", escape = "none")]
struct RuntimeHandleMapJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/handle-map.d.ts.j2", escape = "none")]
struct RuntimeHandleMapDtsTemplate {}

#[derive(Template)]
#[template(path = "runtime/callbacks.js.j2", escape = "none")]
struct RuntimeCallbacksJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/callbacks.d.ts.j2", escape = "none")]
struct RuntimeCallbacksDtsTemplate {}

#[derive(Template)]
#[template(path = "runtime/objects.js.j2", escape = "none")]
struct RuntimeObjectsJsTemplate {}

#[derive(Template)]
#[template(path = "runtime/objects.d.ts.j2", escape = "none")]
struct RuntimeObjectsDtsTemplate {}

// GENERATED CODE
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        env, process,
        time::{SystemTime, UNIX_EPOCH},
    };

    use uniffi_bindgen::interface::ComponentInterface;

    fn component_with_namespace(namespace: &str) -> Component<NodeBindingGeneratorConfig> {
        Component {
            ci: ComponentInterface::from_webidl(
                &format!("namespace {namespace} {{}};"),
                "fixture_crate",
            )
            .expect("valid test UDL"),
            config: NodeBindingGeneratorConfig {
                package_name: Some(format!("{namespace}-package")),
                cdylib_name: Some("fixture".to_string()),
                ..NodeBindingGeneratorConfig::default()
            },
        }
    }

    fn component_with_manual_load(namespace: &str) -> Component<NodeBindingGeneratorConfig> {
        let mut component = component_with_namespace(namespace);
        component.config.manual_load = true;
        component
    }

    fn component_from_webidl(source: &str) -> Component<NodeBindingGeneratorConfig> {
        Component {
            ci: ComponentInterface::from_webidl(source, "fixture_crate").expect("valid test UDL"),
            config: NodeBindingGeneratorConfig {
                package_name: Some("fixture-package".to_string()),
                cdylib_name: Some("fixture".to_string()),
                ..NodeBindingGeneratorConfig::default()
            },
        }
    }

    fn temp_dir_path(name: &str) -> Utf8PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        Utf8PathBuf::from_path_buf(env::temp_dir().join(format!(
            "uniffi-bindgen-node-js-{name}-{}-{unique}",
            process::id()
        )))
        .expect("temp dir path should be utf-8")
    }

    fn parse_node_config(source: &str) -> NodeBindingGeneratorConfig {
        let root = source
            .parse::<toml::Value>()
            .expect("test TOML should deserialize");
        NodeBindingGenerator::new(NodeBindingCliOverrides::default())
            .new_config(&root)
            .expect("node config should deserialize")
    }

    #[test]
    fn write_bindings_creates_output_package_directory() {
        let generator = NodeBindingGenerator::new(NodeBindingCliOverrides::default());
        let output_dir = temp_dir_path("package-root");
        let settings = GenerationSettings {
            out_dir: output_dir.clone(),
            try_format_code: false,
            cdylib: Some("fixture".to_string()),
        };

        generator
            .write_bindings(&settings, &[component_with_namespace("example")])
            .expect("write_bindings should succeed");

        assert!(output_dir.is_dir(), "expected {output_dir} to be created");

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_rejects_multiple_components() {
        let generator = NodeBindingGenerator::new(NodeBindingCliOverrides::default());
        let settings = GenerationSettings {
            out_dir: temp_dir_path("multiple-components"),
            try_format_code: false,
            cdylib: Some("fixture".to_string()),
        };

        let error = generator
            .write_bindings(
                &settings,
                &[
                    component_with_namespace("first"),
                    component_with_namespace("second"),
                ],
            )
            .expect_err("multiple components should be rejected");

        assert!(
            error.to_string().contains("one npm package per invocation"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn write_bindings_emits_package_and_component_files() {
        let generator = NodeBindingGenerator::new(NodeBindingCliOverrides::default());
        let output_dir = temp_dir_path("package-files");
        let settings = GenerationSettings {
            out_dir: output_dir.clone(),
            try_format_code: false,
            cdylib: Some("fixture".to_string()),
        };

        generator
            .write_bindings(&settings, &[component_with_namespace("example")])
            .expect("write_bindings should succeed");

        for expected in [
            "package.json",
            "index.js",
            "index.d.ts",
            "example.js",
            "example.d.ts",
            "example-ffi.js",
            "example-ffi.d.ts",
            "runtime/errors.js",
            "runtime/errors.d.ts",
            "runtime/ffi-types.js",
            "runtime/ffi-types.d.ts",
            "runtime/ffi-converters.js",
            "runtime/ffi-converters.d.ts",
            "runtime/rust-call.js",
            "runtime/rust-call.d.ts",
            "runtime/async-rust-call.js",
            "runtime/async-rust-call.d.ts",
            "runtime/handle-map.js",
            "runtime/handle-map.d.ts",
            "runtime/callbacks.js",
            "runtime/callbacks.d.ts",
            "runtime/objects.js",
            "runtime/objects.d.ts",
        ] {
            let path = output_dir.join(expected);
            assert!(path.is_file(), "expected generated file {path}");
        }

        let package_json = fs::read_to_string(output_dir.join("package.json").as_std_path())
            .expect("package.json should be readable");
        assert!(
            package_json.contains("\"name\": \"example-package\""),
            "unexpected package.json contents: {package_json}"
        );
        assert!(
            package_json.contains("\"koffi\": \"^2.0.0\""),
            "unexpected package.json contents: {package_json}"
        );

        let component_js = fs::read_to_string(output_dir.join("example.js").as_std_path())
            .expect("component JS should be readable");
        assert!(
            component_js.contains("componentMetadata"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export { ffiMetadata }"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("loadFfi()"),
            "unexpected component JS contents: {component_js}"
        );

        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        assert!(
            component_ffi_js.contains("import koffi from \"koffi\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("from \"./runtime/ffi-types.js\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("koffi.load"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("uniffi_contract_version"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        let ffi_types_js =
            fs::read_to_string(output_dir.join("runtime/ffi-types.js").as_std_path())
                .expect("runtime FFI types JS should be readable");
        let errors_js = fs::read_to_string(output_dir.join("runtime/errors.js").as_std_path())
            .expect("runtime errors JS should be readable");
        assert!(
            errors_js.contains("export class UniffiError"),
            "unexpected runtime errors JS contents: {errors_js}"
        );
        assert!(
            errors_js.contains("export const UniffiInternalError"),
            "unexpected runtime errors JS contents: {errors_js}"
        );
        assert!(
            ffi_types_js.contains("export const RustBuffer"),
            "unexpected runtime FFI types JS contents: {ffi_types_js}"
        );
        assert!(
            ffi_types_js.contains("export function defineCallbackVtable"),
            "unexpected runtime FFI types JS contents: {ffi_types_js}"
        );
        assert!(
            ffi_types_js.contains("export function normalizeUInt64"),
            "unexpected runtime FFI types JS contents: {ffi_types_js}"
        );
        assert!(
            ffi_types_js.contains("export class RustBufferValue"),
            "unexpected runtime FFI types JS contents: {ffi_types_js}"
        );
        assert!(
            ffi_types_js.contains("export function readRustBufferBytes"),
            "unexpected runtime FFI types JS contents: {ffi_types_js}"
        );
        let ffi_converters_js =
            fs::read_to_string(output_dir.join("runtime/ffi-converters.js").as_std_path())
                .expect("runtime FFI converters JS should be readable");
        assert!(
            ffi_converters_js.contains("export class AbstractFfiConverterByteArray"),
            "unexpected runtime FFI converters JS contents: {ffi_converters_js}"
        );
        assert!(
            ffi_converters_js.contains("export const FfiConverterString"),
            "unexpected runtime FFI converters JS contents: {ffi_converters_js}"
        );
        let rust_call_js =
            fs::read_to_string(output_dir.join("runtime/rust-call.js").as_std_path())
                .expect("runtime rust-call JS should be readable");
        assert!(
            rust_call_js.contains("export function checkRustCallStatus"),
            "unexpected runtime rust-call JS contents: {rust_call_js}"
        );
        assert!(
            rust_call_js.contains("export class UniffiRustCaller"),
            "unexpected runtime rust-call JS contents: {rust_call_js}"
        );
        let handle_map_js =
            fs::read_to_string(output_dir.join("runtime/handle-map.js").as_std_path())
                .expect("runtime handle-map JS should be readable");
        assert!(
            handle_map_js.contains("export class UniffiHandleMap"),
            "unexpected runtime handle-map JS contents: {handle_map_js}"
        );
        assert!(
            handle_map_js.contains("export const FIRST_FOREIGN_HANDLE"),
            "unexpected runtime handle-map JS contents: {handle_map_js}"
        );
        let async_rust_call_js =
            fs::read_to_string(output_dir.join("runtime/async-rust-call.js").as_std_path())
                .expect("runtime async rust-call JS should be readable");
        let callbacks_js =
            fs::read_to_string(output_dir.join("runtime/callbacks.js").as_std_path())
                .expect("runtime callbacks JS should be readable");
        let objects_js = fs::read_to_string(output_dir.join("runtime/objects.js").as_std_path())
            .expect("runtime objects JS should be readable");
        assert!(
            async_rust_call_js.contains("export async function rustCallAsync"),
            "unexpected runtime async rust-call JS contents: {async_rust_call_js}"
        );
        assert!(
            async_rust_call_js.contains("export const rustFutureContinuationCallback"),
            "unexpected runtime async rust-call JS contents: {async_rust_call_js}"
        );
        assert!(
            callbacks_js.contains("export class UniffiCallbackRegistry"),
            "unexpected runtime callbacks JS contents: {callbacks_js}"
        );
        assert!(
            callbacks_js.contains("export function invokeCallbackMethod"),
            "unexpected runtime callbacks JS contents: {callbacks_js}"
        );
        assert!(
            callbacks_js.contains("export function writeCallbackError"),
            "unexpected runtime callbacks JS contents: {callbacks_js}"
        );
        assert!(
            objects_js.contains("export class UniffiObjectFactory"),
            "unexpected runtime objects JS contents: {objects_js}"
        );
        assert!(
            objects_js.contains("export class FfiConverterObject"),
            "unexpected runtime objects JS contents: {objects_js}"
        );
        assert!(
            objects_js.contains("UNIFFI_OBJECT_HANDLE_SIZE"),
            "unexpected runtime objects JS contents: {objects_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_emits_koffi_callback_and_function_declarations() {
        let generator = NodeBindingGenerator::new(NodeBindingCliOverrides::default());
        let output_dir = temp_dir_path("ffi-bindings");
        let settings = GenerationSettings {
            out_dir: output_dir.clone(),
            try_format_code: false,
            cdylib: Some("fixture".to_string()),
        };
        let component = component_from_webidl(
            r#"
            namespace example {
                u64 current_generation();
                void init_logging(LogCallback callback);
            };

            callback interface LogCallback {
                void log(string message);
            };
            "#,
        );

        generator
            .write_bindings(&settings, &[component])
            .expect("write_bindings should succeed");

        let component_js = fs::read_to_string(output_dir.join("example.js").as_std_path())
            .expect("component JS should be readable");
        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        assert!(
            component_js.contains("createCallbackRegistry"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("configureRuntimeHooks({"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("loadFfi();"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_ffi_js
                .contains("defineCallbackPrototype(\"CallbackInterfaceLogCallbackMethod0\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("export function configureRuntimeHooks"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            !component_ffi_js.contains("if (!ffiMetadata.manualLoad) {\n  load();\n}"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("defineCallbackPrototype(\"RustFutureContinuationCallback\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js
                .contains("defineCallbackVtable(\"VTableCallbackInterfaceLogCallback\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("defineStructType(\"ForeignFuture\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("defineCallbackPrototype(\"ForeignFutureCompleteVoid\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("current_generation"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("init_callback_vtable_logcallback"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("ffi_fixture_crate_uniffi_contract_version"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("uniffi_fixture_crate_checksum_func_current_generation"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("normalizeUInt64"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_emits_typed_object_handle_round_trip_support() {
        let generator = NodeBindingGenerator::new(NodeBindingCliOverrides::default());
        let output_dir = temp_dir_path("object-handle-round-trip");
        let settings = GenerationSettings {
            out_dir: output_dir.clone(),
            try_format_code: false,
            cdylib: Some("fixture".to_string()),
        };
        let component = component_from_webidl(
            r#"
            namespace example {};

            interface Store {
                constructor();
                Store? maybe_clone(Store? value);
            };
            "#,
        );

        generator
            .write_bindings(&settings, &[component])
            .expect("write_bindings should succeed");

        let component_js = fs::read_to_string(output_dir.join("example.js").as_std_path())
            .expect("component JS should be readable");
        let objects_js = fs::read_to_string(output_dir.join("runtime/objects.js").as_std_path())
            .expect("runtime objects JS should be readable");
        let ffi_types_js =
            fs::read_to_string(output_dir.join("runtime/ffi-types.js").as_std_path())
                .expect("runtime FFI types JS should be readable");

        assert!(
            component_js.contains("getFfiBindings"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("handleType: () => getFfiBindings().ffiTypes.RustArcPtrStore,"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            objects_js.contains("import koffi from \"koffi\";"),
            "unexpected runtime objects JS contents: {objects_js}"
        );
        assert!(
            objects_js.contains(
                "return koffi.decode(new BigUint64Array([normalizeUInt64(normalizedHandle)]), handleType);",
            ),
            "unexpected runtime objects JS contents: {objects_js}"
        );
        assert!(
            objects_js.contains("return this.factory.createRetyped(handle);"),
            "unexpected runtime objects JS contents: {objects_js}"
        );
        assert!(
            objects_js.contains("const rawHandle = requireHandle("),
            "unexpected runtime objects JS contents: {objects_js}"
        );
        assert!(
            ffi_types_js.contains("return normalizeUInt64(pointer);"),
            "unexpected runtime FFI types JS contents: {ffi_types_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_emits_slatedb_callback_interface_paths() {
        let generator = NodeBindingGenerator::new(NodeBindingCliOverrides::default());
        let output_dir = temp_dir_path("slatedb-callbacks");
        let settings = GenerationSettings {
            out_dir: output_dir.clone(),
            try_format_code: false,
            cdylib: Some("fixture".to_string()),
        };
        let component = component_from_webidl(
            r#"
            namespace example {
                void init_logging(LogLevel level, LogCallback? callback);
            };

            enum LogLevel {
                "off",
                "info"
            };

            dictionary LogRecord {
                LogLevel level;
                string target;
                string message;
            };

            [Error]
            interface MergeOperatorCallbackError {
                Callback(string message);
            };

            callback interface LogCallback {
                void log(LogRecord record);
            };

            callback interface MergeOperator {
                [Throws=MergeOperatorCallbackError]
                bytes merge(bytes key, bytes? existing_value, bytes operand);
            };

            interface DbBuilder {
                constructor();
                void with_merge_operator(MergeOperator merge_operator);
            };
            "#,
        );

        generator
            .write_bindings(&settings, &[component])
            .expect("write_bindings should succeed");

        let component_js = fs::read_to_string(output_dir.join("example.js").as_std_path())
            .expect("component JS should be readable");

        assert!(
            component_js.contains("export function init_logging(level, callback)"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "const loweredCallback = uniffiLowerIntoRustBuffer(uniffiOptionalConverter(FfiConverterLogCallback), callback);"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js
                .contains(
                    "function uniffiRegisterLogCallbackVtable(bindings, registrations, vtableReferences) {"
                ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js
                .contains(
                    "function uniffiRegisterMergeOperatorVtable(bindings, registrations, vtableReferences) {"
                ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("args: [\n          uniffiLiftFromRustBuffer(FfiConverterLogRecord, record),\n        ],"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "const loweredMergeOperator = FfiConverterMergeOperator.lower(merge_operator);"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "args: [\n          uniffiLiftFromRustBuffer(FfiConverterBytes, key),\n          uniffiLiftFromRustBuffer(uniffiOptionalConverter(FfiConverterBytes), existing_value),\n          uniffiLiftFromRustBuffer(FfiConverterBytes, operand),\n        ],"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "lowerError: (error) => error instanceof MergeOperatorCallbackError ? uniffiLowerIntoRustBuffer(FfiConverterMergeOperatorCallbackError, error) : null,"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "const loweredReturn = uniffiLowerIntoRustBuffer(FfiConverterBytes, uniffiResult);"
            ),
            "unexpected component JS contents: {component_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_emits_slatedb_async_api_paths() {
        let generator = NodeBindingGenerator::new(NodeBindingCliOverrides::default());
        let output_dir = temp_dir_path("slatedb-async-apis");
        let settings = GenerationSettings {
            out_dir: output_dir.clone(),
            try_format_code: false,
            cdylib: Some("fixture".to_string()),
        };
        let component = component_from_webidl(
            r#"
            namespace example {};

            enum IsolationLevel {
                "read_committed",
                "serializable"
            };

            dictionary KeyRange {
                bytes start;
                bytes end;
            };

            dictionary KeyValue {
                bytes key;
                bytes value;
            };

            dictionary WriteHandle {
                u64 seq;
            };

            dictionary WalFileMetadata {
                i64 last_modified_seconds;
                u32 last_modified_nanos;
                u64 size_bytes;
                string location;
            };

            dictionary RowEntry {
                bytes key;
                bytes value;
            };

            interface WriteBatch {
                constructor();
            };

            interface DbIterator {
                constructor();
                [Async] KeyValue? next();
                [Async] void seek(bytes key);
            };

            interface DbSnapshot {
                constructor();
                [Async] bytes? get(bytes key);
                [Async] KeyValue? get_key_value(bytes key);
                [Async] DbIterator scan(KeyRange range);
                [Async] DbIterator scan_prefix(bytes prefix);
            };

            interface DbReader {
                constructor();
                [Async] bytes? get(bytes key);
                [Async] DbIterator scan(KeyRange range);
                [Async] DbIterator scan_prefix(bytes prefix);
                [Async] void shutdown();
            };

            interface DbTransaction {
                constructor();
                [Async] void put(bytes key, bytes value);
                [Async] bytes? get(bytes key);
                [Async] KeyValue? get_key_value(bytes key);
                [Async] DbIterator scan(KeyRange range);
                [Async] DbIterator scan_prefix(bytes prefix);
                [Async] WriteHandle? commit();
            };

            interface Db {
                constructor();
                [Async] void shutdown();
                [Async] bytes? get(bytes key);
                [Async] KeyValue? get_key_value(bytes key);
                [Async] DbIterator scan(KeyRange range);
                [Async] DbIterator scan_prefix(bytes prefix);
                [Async] WriteHandle put(bytes key, bytes value);
                [Async] void flush();
                [Async] DbSnapshot snapshot();
                [Async] DbTransaction begin(IsolationLevel isolation_level);
                [Async] void write(WriteBatch batch);
            };

            interface WalFile {
                constructor();
                [Async] WalFileMetadata metadata();
                [Async] WalFileIterator iterator();
            };

            interface WalFileIterator {
                constructor();
                [Async] RowEntry? next();
            };

            interface WalReader {
                constructor();
                WalFile get(u64 id);
                [Async] sequence<WalFile> list(u64? start_id, u64? end_id);
            };
            "#,
        );

        generator
            .write_bindings(&settings, &[component])
            .expect("write_bindings should succeed");

        let component_js = fs::read_to_string(output_dir.join("example.js").as_std_path())
            .expect("component JS should be readable");
        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");

        assert!(
            component_js.contains("export class Db extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export class DbReader extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export class DbIterator extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export class DbSnapshot extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export class DbTransaction extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export class WalReader extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export class WalFile extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("export class WalFileIterator extends UniffiObjectBase {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiLiftFromRustBuffer(uniffiOptionalConverter(FfiConverterBytes), uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiLiftFromRustBuffer(uniffiOptionalConverter(FfiConverterKeyValue), uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiDbIteratorObjectFactory.createRawExternal(uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiDbSnapshotObjectFactory.createRawExternal(uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiDbTransactionObjectFactory.createRawExternal(uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiWalFileIteratorObjectFactory.createRawExternal(uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiLiftFromRustBuffer(uniffiOptionalConverter(FfiConverterWriteHandle), uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiLiftFromRustBuffer(FfiConverterWalFileMetadata, uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiLiftFromRustBuffer(uniffiOptionalConverter(FfiConverterRowEntry), uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "liftFunc: (uniffiResult) => uniffiLiftFromRustBuffer(uniffiArrayConverter(FfiConverterWalFile), uniffiResult),"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js
                .contains("const loweredBatch = uniffiWriteBatchObjectFactory.cloneHandle(batch);"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "const loweredIsolationLevel = uniffiLowerIntoRustBuffer(FfiConverterIsolationLevel, isolation_level);"
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("rustFutureContinuationCallback"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("let uniffiRustFutureContinuationPointer = null;"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("const library = bindings.library;"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("let libraryCache = uniffiLibraryFunctionCache.get(library);"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("uniffiLibraryFunctionCache.set(library, libraryCache);"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("function uniffiGetRustFutureContinuationPointer() {"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains(
                "pollFunc: (rustFuture, _continuationCallback, continuationHandle) => ffiFunctions."
            ),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("uniffiGetRustFutureContinuationPointer(), continuationHandle)"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("configureRuntimeHooks({"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_js.contains("koffi.unregister(uniffiRustFutureContinuationPointer);"),
            "unexpected component JS contents: {component_js}"
        );
        assert!(
            component_ffi_js.contains("export function configureRuntimeHooks"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            !component_ffi_js.contains("if (!ffiMetadata.manualLoad) {\n  load();\n}"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_makes_ffi_load_idempotent() {
        let generator = NodeBindingGenerator::new(NodeBindingCliOverrides::default());
        let output_dir = temp_dir_path("ffi-idempotent-load");
        let settings = GenerationSettings {
            out_dir: output_dir.clone(),
            try_format_code: false,
            cdylib: Some("fixture".to_string()),
        };

        generator
            .write_bindings(&settings, &[component_with_namespace("example")])
            .expect("write_bindings should succeed");

        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        assert!(
            component_ffi_js.contains("loadedBindings.libraryPath === canonicalLibraryPath"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("return loadedBindings;"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("Call unload() before loading a different library path."),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_validates_contract_version_during_load() {
        let generator = NodeBindingGenerator::new(NodeBindingCliOverrides::default());
        let output_dir = temp_dir_path("ffi-contract-version");
        let settings = GenerationSettings {
            out_dir: output_dir.clone(),
            try_format_code: false,
            cdylib: Some("fixture".to_string()),
        };

        generator
            .write_bindings(&settings, &[component_with_namespace("example")])
            .expect("write_bindings should succeed");

        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        assert!(
            component_ffi_js.contains("expectedContractVersion: 29"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("validateContractVersion(bindings);"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("throw new ContractVersionMismatchError(expected, actual);"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("bindings.library.unload();"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        let component_ffi_dts =
            fs::read_to_string(output_dir.join("example-ffi.d.ts").as_std_path())
                .expect("component FFI DTS should be readable");
        assert!(
            component_ffi_dts.contains("export declare function validateContractVersion"),
            "unexpected component FFI DTS contents: {component_ffi_dts}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_validates_checksums_during_load() {
        let generator = NodeBindingGenerator::new(NodeBindingCliOverrides::default());
        let output_dir = temp_dir_path("ffi-checksums");
        let settings = GenerationSettings {
            out_dir: output_dir.clone(),
            try_format_code: false,
            cdylib: Some("fixture".to_string()),
        };
        let component = component_from_webidl(
            r#"
            namespace example {
                u64 current_generation();
            };
            "#,
        );

        generator
            .write_bindings(&settings, &[component])
            .expect("write_bindings should succeed");

        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        assert!(
            component_ffi_js.contains("validateChecksums(bindings);"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("checksums: Object.freeze({"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("\"uniffi_fixture_crate_checksum_func_current_generation\":"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains(
                "throw new ChecksumMismatchError(\"uniffi_fixture_crate_checksum_func_current_generation\", expected, actual);"
            ),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        let component_ffi_dts =
            fs::read_to_string(output_dir.join("example-ffi.d.ts").as_std_path())
                .expect("component FFI DTS should be readable");
        assert!(
            component_ffi_dts.contains("export declare function validateChecksums"),
            "unexpected component FFI DTS contents: {component_ffi_dts}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_resolves_sibling_and_literal_library_paths() {
        let generator = NodeBindingGenerator::new(NodeBindingCliOverrides::default());
        let output_dir = temp_dir_path("ffi-library-paths");
        let settings = GenerationSettings {
            out_dir: output_dir.clone(),
            try_format_code: false,
            cdylib: Some("fixture".to_string()),
        };

        generator
            .write_bindings(&settings, &[component_with_namespace("example")])
            .expect("write_bindings should succeed");

        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        assert!(
            component_ffi_js.contains("import { existsSync, realpathSync } from \"node:fs\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("import { dirname, isAbsolute, join } from \"node:path\""),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("const moduleFilename = fileURLToPath(import.meta.url);"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("function defaultSiblingLibraryPath()"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js
                .contains("const rawLibraryPath = libraryPath ?? ffiMetadata.libPathLiteral;"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("function canonicalizeExistingLibraryPath(libraryPath)"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("libraryPath: isAbsolute(rawLibraryPath)"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("bundledPrebuild: null,"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_auto_loads_by_default() {
        let generator = NodeBindingGenerator::new(NodeBindingCliOverrides::default());
        let output_dir = temp_dir_path("ffi-auto-load");
        let settings = GenerationSettings {
            out_dir: output_dir.clone(),
            try_format_code: false,
            cdylib: Some("fixture".to_string()),
        };

        generator
            .write_bindings(&settings, &[component_with_namespace("example")])
            .expect("write_bindings should succeed");

        let component_ffi_js = fs::read_to_string(output_dir.join("example-ffi.js").as_std_path())
            .expect("component FFI JS should be readable");
        assert!(
            component_ffi_js.contains("if (!ffiMetadata.manualLoad) {"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );
        assert!(
            component_ffi_js.contains("  load();"),
            "unexpected component FFI JS contents: {component_ffi_js}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn write_bindings_exports_manual_load_helpers() {
        let generator = NodeBindingGenerator::new(NodeBindingCliOverrides::default());
        let output_dir = temp_dir_path("manual-load-exports");
        let settings = GenerationSettings {
            out_dir: output_dir.clone(),
            try_format_code: false,
            cdylib: Some("fixture".to_string()),
        };

        generator
            .write_bindings(&settings, &[component_with_manual_load("example")])
            .expect("write_bindings should succeed");

        let component_js = fs::read_to_string(output_dir.join("example.js").as_std_path())
            .expect("component JS should be readable");
        assert!(
            component_js.contains("export { load, unload } from \"./example-ffi.js\";"),
            "unexpected component JS contents: {component_js}"
        );

        let component_dts = fs::read_to_string(output_dir.join("example.d.ts").as_std_path())
            .expect("component DTS should be readable");
        assert!(
            component_dts.contains("export { load, unload } from \"./example-ffi.js\";"),
            "unexpected component DTS contents: {component_dts}"
        );

        fs::remove_dir_all(output_dir.as_std_path()).expect("cleanup temp dir");
    }

    #[test]
    fn config_validation_rejects_commonjs_output() {
        let config = parse_node_config(
            r#"
            [bindings.node]
            module_format = "commonjs"
            "#,
        );

        let error = config
            .validate()
            .expect_err("CommonJS output should be rejected");

        assert!(
            error
                .to_string()
                .contains("CommonJS output is not supported"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn config_override_validation_rejects_commonjs_output() {
        let overrides = NodeBindingCliOverrides::from_parts(
            None,
            None,
            None,
            None,
            false,
            false,
            vec!["commonjs=true".to_string()],
        )
        .expect("override should parse");
        let mut config = NodeBindingGeneratorConfig::default();

        overrides.apply_to(&mut config);
        let error = config
            .validate()
            .expect_err("CommonJS override should be rejected");

        assert!(
            error
                .to_string()
                .contains("CommonJS output is not supported"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn new_config_rejects_removed_legacy_library_path_keys() {
        for key in [
            "lib_path_module",
            "lib_path_modules",
            "out_lib_path_module",
            "out_lib_path_modules",
        ] {
            let root = format!(
                r#"
            [bindings.node]
            {key} = "./native/example.node"
            "#
            )
            .parse::<toml::Value>()
            .expect("test TOML should deserialize");
            let error = NodeBindingGenerator::new(NodeBindingCliOverrides::default())
                .new_config(&root)
                .unwrap_err();

            assert!(
                error
                    .to_string()
                    .contains(&format!("unknown field `{key}`")),
                "unexpected error for {key}: {error}"
            );
        }
    }

    #[test]
    fn config_override_rejects_removed_legacy_library_path_keys() {
        for key in [
            "lib_path_module",
            "lib_path_modules",
            "out_lib_path_module",
            "out_lib_path_modules",
        ] {
            let error = NodeBindingCliOverrides::from_parts(
                None,
                None,
                None,
                None,
                false,
                false,
                vec![format!("{key}=./native/example.node")],
            )
            .unwrap_err();

            assert!(
                error
                    .to_string()
                    .contains(&format!("unsupported --config-override key '{key}'")),
                "unexpected error for {key}: {error}"
            );
        }
    }

    #[test]
    fn new_config_parses_bindings_node_settings_and_defaults() {
        let explicit = parse_node_config(
            r#"
            [bindings.node]
            package_name = "fixture-package"
            cdylib_name = "fixture_cdylib"
            node_engine = ">=20"
            lib_path_literal = "./native/libfixture.node"
            bundled_prebuilds = false
            manual_load = true
            "#,
        );

        assert_eq!(explicit.package_name.as_deref(), Some("fixture-package"));
        assert_eq!(explicit.cdylib_name.as_deref(), Some("fixture_cdylib"));
        assert_eq!(explicit.node_engine, ">=20");
        assert_eq!(
            explicit.lib_path_literal.as_deref(),
            Some("./native/libfixture.node")
        );
        assert!(!explicit.bundled_prebuilds);
        assert!(explicit.manual_load);

        let defaulted = parse_node_config(
            r#"
            [bindings.node]
            "#,
        );

        assert_eq!(defaulted.package_name, None);
        assert_eq!(defaulted.cdylib_name, None);
        assert_eq!(defaulted.node_engine, ">=16");
        assert_eq!(defaulted.lib_path_literal, None);
        assert!(!defaulted.bundled_prebuilds);
        assert!(!defaulted.manual_load);
    }

    #[test]
    fn new_config_accepts_bundled_prebuilds() {
        let config = parse_node_config(
            r#"
            [bindings.node]
            bundled_prebuilds = true
            "#,
        );

        assert!(config.bundled_prebuilds);
    }

    #[test]
    fn config_validation_rejects_bundled_prebuilds_with_lib_path_literal() {
        let config = parse_node_config(
            r#"
            [bindings.node]
            bundled_prebuilds = true
            lib_path_literal = "./native/libfixture.node"
            "#,
        );

        let error = config
            .validate()
            .expect_err("bundled_prebuilds with lib_path_literal should be rejected");

        assert!(
            error
                .to_string()
                .contains("bundled_prebuilds cannot be enabled together with lib_path_literal"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn generated_package_layout_resolves_output_paths_from_out_dir_and_namespace() {
        let out_dir = temp_dir_path("layout-paths");
        let settings = GenerationSettings {
            out_dir: out_dir.clone(),
            try_format_code: false,
            cdylib: Some("fixture".to_string()),
        };
        let component = component_with_namespace("example");

        let layout = GeneratedPackageLayout::from_component(&settings, &component).expect("layout");

        assert_eq!(layout.root_dir, out_dir);
        assert_eq!(
            layout.package_json_path(),
            layout.root_dir.join("package.json")
        );
        assert_eq!(layout.index_js_path(), layout.root_dir.join("index.js"));
        assert_eq!(layout.index_dts_path(), layout.root_dir.join("index.d.ts"));
        assert_eq!(
            layout.component_js_path(),
            layout.root_dir.join("example.js")
        );
        assert_eq!(
            layout.component_dts_path(),
            layout.root_dir.join("example.d.ts")
        );
        assert_eq!(
            layout.component_ffi_js_path(),
            layout.root_dir.join("example-ffi.js")
        );
        assert_eq!(
            layout.component_ffi_dts_path(),
            layout.root_dir.join("example-ffi.d.ts")
        );
        assert_eq!(
            layout.runtime_path("errors.js"),
            layout.root_dir.join("runtime/errors.js")
        );
    }
}
