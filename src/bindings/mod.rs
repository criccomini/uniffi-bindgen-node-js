use std::fs;

use anyhow::{Result, anyhow, bail};
use askama::Template;
use camino::Utf8PathBuf;
use serde::Deserialize;
use uniffi_bindgen::{BindingGenerator, Component, GenerationSettings};

mod api;

use self::api::{ComponentModel, RenderedComponentApi};

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
    manual_load: bool,
    config_overrides: Vec<NodeBindingConfigOverride>,
}

impl NodeBindingCliOverrides {
    pub fn from_parts(
        package_name: Option<String>,
        cdylib_name: Option<String>,
        node_engine: Option<String>,
        lib_path_literal: Option<String>,
        manual_load: bool,
        config_overrides: Vec<String>,
    ) -> Result<Self> {
        Ok(Self {
            package_name: normalize_optional_value("--package-name", package_name)?,
            cdylib_name: normalize_optional_value("--cdylib-name", cdylib_name)?,
            node_engine: normalize_optional_value("--node-engine", node_engine)?,
            lib_path_literal: normalize_optional_value("--lib-path-literal", lib_path_literal)?,
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

        if let Some(package_name) = &self.package_name {
            config.package_name = Some(package_name.clone());
        }
        if let Some(cdylib_name) = &self.cdylib_name {
            config.cdylib_name = Some(cdylib_name.clone());
        }
        if let Some(node_engine) = &self.node_engine {
            config.node_engine = node_engine.clone();
        }
        if let Some(lib_path_literal) = &self.lib_path_literal {
            config.lib_path_literal = Some(lib_path_literal.clone());
        }
        if self.manual_load {
            config.manual_load = true;
        }
    }
}

#[derive(Debug, Clone)]
enum NodeBindingConfigOverride {
    PackageName(String),
    CdylibName(String),
    NodeEngine(String),
    LibPathLiteral(String),
    ManualLoad(bool),
    ModuleFormat(String),
    Commonjs(bool),
    LibPathModules(String),
}

impl NodeBindingConfigOverride {
    fn parse(raw: String) -> Result<Self> {
        let (raw_key, raw_value) = raw
            .split_once('=')
            .ok_or_else(|| anyhow!("invalid --config-override '{raw}': expected KEY=VALUE"))?;
        let key = raw_key.trim();
        if key.is_empty() {
            bail!("invalid --config-override '{raw}': missing key before '='");
        }
        let value = normalize_required_value("--config-override", raw_value.trim())?;

        match key {
            "package_name"
            | "package-name"
            | "bindings.node.package_name"
            | "bindings.node.package-name" => Ok(Self::PackageName(value)),
            "cdylib_name"
            | "cdylib-name"
            | "bindings.node.cdylib_name"
            | "bindings.node.cdylib-name" => Ok(Self::CdylibName(value)),
            "node_engine"
            | "node-engine"
            | "bindings.node.node_engine"
            | "bindings.node.node-engine" => Ok(Self::NodeEngine(value)),
            "lib_path_literal"
            | "lib-path-literal"
            | "bindings.node.lib_path_literal"
            | "bindings.node.lib-path-literal" => Ok(Self::LibPathLiteral(value)),
            "manual_load"
            | "manual-load"
            | "bindings.node.manual_load"
            | "bindings.node.manual-load" => {
                Ok(Self::ManualLoad(parse_bool_override(&raw, &value)?))
            }
            "module_format"
            | "module-format"
            | "bindings.node.module_format"
            | "bindings.node.module-format" => Ok(Self::ModuleFormat(value)),
            "commonjs" | "bindings.node.commonjs" => {
                Ok(Self::Commonjs(parse_bool_override(&raw, &value)?))
            }
            "lib_path_module"
            | "lib-path-module"
            | "lib_path_modules"
            | "lib-path-modules"
            | "out_lib_path_module"
            | "out-lib-path-module"
            | "out_lib_path_modules"
            | "out-lib-path-modules"
            | "bindings.node.lib_path_module"
            | "bindings.node.lib-path-module"
            | "bindings.node.lib_path_modules"
            | "bindings.node.lib-path-modules"
            | "bindings.node.out_lib_path_module"
            | "bindings.node.out-lib-path-module"
            | "bindings.node.out_lib_path_modules"
            | "bindings.node.out-lib-path-modules" => Ok(Self::LibPathModules(value)),
            _ => bail!("unsupported --config-override key '{key}'"),
        }
    }

    fn apply_to(&self, config: &mut NodeBindingGeneratorConfig) {
        match self {
            Self::PackageName(value) => config.package_name = Some(value.clone()),
            Self::CdylibName(value) => config.cdylib_name = Some(value.clone()),
            Self::NodeEngine(value) => config.node_engine = value.clone(),
            Self::LibPathLiteral(value) => config.lib_path_literal = Some(value.clone()),
            Self::ManualLoad(value) => config.manual_load = *value,
            Self::ModuleFormat(value) => config.module_format = Some(value.clone()),
            Self::Commonjs(value) => config.commonjs = Some(*value),
            Self::LibPathModules(value) => {
                config.lib_path_modules = Some(toml::Value::String(value.clone()))
            }
        }
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

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NodeBindingGeneratorConfig {
    pub package_name: Option<String>,
    pub cdylib_name: Option<String>,
    pub node_engine: String,
    pub lib_path_literal: Option<String>,
    pub manual_load: bool,
    #[serde(alias = "module-format")]
    pub module_format: Option<String>,
    pub commonjs: Option<bool>,
    #[serde(
        alias = "lib-path-module",
        alias = "lib_path_module",
        alias = "lib-path-modules",
        alias = "out-lib-path-module",
        alias = "out_lib_path_module",
        alias = "out-lib-path-modules",
        alias = "out_lib_path_modules"
    )]
    pub lib_path_modules: Option<toml::Value>,
}

impl Default for NodeBindingGeneratorConfig {
    fn default() -> Self {
        Self {
            package_name: None,
            cdylib_name: None,
            node_engine: ">=16".to_string(),
            lib_path_literal: None,
            manual_load: false,
            module_format: None,
            commonjs: None,
            lib_path_modules: None,
        }
    }
}

impl NodeBindingGeneratorConfig {
    fn validate(&self) -> Result<()> {
        if self
            .package_name
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            bail!("node binding package_name cannot be empty");
        }
        if self
            .cdylib_name
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            bail!("node binding cdylib_name cannot be empty");
        }
        if self.node_engine.trim().is_empty() {
            bail!("node binding node_engine cannot be empty");
        }
        if self
            .lib_path_literal
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            bail!("node binding lib_path_literal cannot be empty");
        }
        if let Some(module_format) = self.module_format.as_deref() {
            let normalized = module_format.trim();
            if normalized.is_empty() {
                bail!("node binding module_format cannot be empty");
            }
            if !normalized.eq_ignore_ascii_case("esm") {
                if normalized.eq_ignore_ascii_case("commonjs")
                    || normalized.eq_ignore_ascii_case("cjs")
                {
                    bail!("node bindings v1 are ESM-only; CommonJS output is not supported");
                }
                bail!(
                    "unsupported node binding module_format '{normalized}': v1 only supports 'esm'"
                );
            }
        }
        if self.commonjs == Some(true) {
            bail!("node bindings v1 are ESM-only; CommonJS output is not supported");
        }
        if self.lib_path_modules.is_some() {
            bail!(
                "node bindings v1 do not support multi-package platform-switch packaging; use lib_path_literal or the default sibling-library lookup"
            );
        }
        Ok(())
    }
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
    manual_load: bool,
    public_api: RenderedComponentApi,
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

        Ok(Self {
            layout,
            cdylib_name: cdylib_name.to_string(),
            node_engine: component.config.node_engine.trim().to_string(),
            lib_path_literal: component.config.lib_path_literal.clone(),
            manual_load: component.config.manual_load,
            public_api,
        })
    }

    fn ensure_root_dir(&self) -> Result<()> {
        self.layout.ensure_root_dir()
    }

    fn write_package_files(&self) -> Result<()> {
        let template_context = TemplateContext::from_package(self)?;

        write_template(
            &self.layout.package_json_path(),
            &PackageJsonTemplate {
                package_name_json: template_context.package_name_json.clone(),
                node_engine_json: template_context.node_engine_json.clone(),
            },
        )?;
        write_template(
            &self.layout.index_js_path(),
            &PackageIndexJsTemplate {
                namespace: self.layout.namespace.clone(),
            },
        )?;
        write_template(
            &self.layout.index_dts_path(),
            &PackageIndexDtsTemplate {
                namespace: self.layout.namespace.clone(),
            },
        )?;
        write_template(
            &self.layout.component_js_path(),
            &ComponentJsTemplate {
                namespace: self.layout.namespace.clone(),
                namespace_json: template_context.namespace_json.clone(),
                package_name_json: template_context.package_name_json.clone(),
                cdylib_name_json: template_context.cdylib_name_json.clone(),
                node_engine_json: template_context.node_engine_json.clone(),
                lib_path_literal_json: template_context.lib_path_literal_json.clone(),
                manual_load: self.manual_load,
                public_api_js: self.public_api.js.clone(),
            },
        )?;
        write_template(
            &self.layout.component_dts_path(),
            &ComponentDtsTemplate {
                namespace: self.layout.namespace.clone(),
                public_api_dts: self.public_api.dts.clone(),
            },
        )?;
        write_template(
            &self.layout.component_ffi_js_path(),
            &ComponentFfiJsTemplate {
                namespace_json: template_context.namespace_json,
                cdylib_name_json: template_context.cdylib_name_json,
                lib_path_literal_json: template_context.lib_path_literal_json,
                manual_load: self.manual_load,
            },
        )?;
        write_template(
            &self.layout.component_ffi_dts_path(),
            &ComponentFfiDtsTemplate {},
        )?;
        self.write_runtime_files()?;

        Ok(())
    }

    fn write_runtime_files(&self) -> Result<()> {
        write_template(
            &self.layout.runtime_path("errors.js"),
            &RuntimeErrorsJsTemplate {},
        )?;
        write_template(
            &self.layout.runtime_path("errors.d.ts"),
            &RuntimeErrorsDtsTemplate {},
        )?;
        write_template(
            &self.layout.runtime_path("ffi-types.js"),
            &RuntimeFfiTypesJsTemplate {},
        )?;
        write_template(
            &self.layout.runtime_path("ffi-types.d.ts"),
            &RuntimeFfiTypesDtsTemplate {},
        )?;
        write_template(
            &self.layout.runtime_path("ffi-converters.js"),
            &RuntimeFfiConvertersJsTemplate {},
        )?;
        write_template(
            &self.layout.runtime_path("ffi-converters.d.ts"),
            &RuntimeFfiConvertersDtsTemplate {},
        )?;
        write_template(
            &self.layout.runtime_path("rust-call.js"),
            &RuntimeRustCallJsTemplate {},
        )?;
        write_template(
            &self.layout.runtime_path("rust-call.d.ts"),
            &RuntimeRustCallDtsTemplate {},
        )?;
        write_template(
            &self.layout.runtime_path("async-rust-call.js"),
            &RuntimeAsyncRustCallJsTemplate {},
        )?;
        write_template(
            &self.layout.runtime_path("async-rust-call.d.ts"),
            &RuntimeAsyncRustCallDtsTemplate {},
        )?;
        write_template(
            &self.layout.runtime_path("handle-map.js"),
            &RuntimeHandleMapJsTemplate {},
        )?;
        write_template(
            &self.layout.runtime_path("handle-map.d.ts"),
            &RuntimeHandleMapDtsTemplate {},
        )?;
        write_template(
            &self.layout.runtime_path("callbacks.js"),
            &RuntimeCallbacksJsTemplate {},
        )?;
        write_template(
            &self.layout.runtime_path("callbacks.d.ts"),
            &RuntimeCallbacksDtsTemplate {},
        )?;
        write_template(
            &self.layout.runtime_path("objects.js"),
            &RuntimeObjectsJsTemplate {},
        )?;
        write_template(
            &self.layout.runtime_path("objects.d.ts"),
            &RuntimeObjectsDtsTemplate {},
        )?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct TemplateContext {
    namespace_json: String,
    package_name_json: String,
    cdylib_name_json: String,
    node_engine_json: String,
    lib_path_literal_json: String,
}

impl TemplateContext {
    fn from_package(package: &GeneratedPackage) -> Result<Self> {
        Ok(Self {
            namespace_json: json_string(&package.layout.namespace)?,
            package_name_json: json_string(&package.layout.package_name)?,
            cdylib_name_json: json_string(&package.cdylib_name)?,
            node_engine_json: json_string(&package.node_engine)?,
            lib_path_literal_json: json_optional_string(package.lib_path_literal.as_deref())?,
        })
    }
}

fn write_template<T: Template>(path: &Utf8PathBuf, template: &T) -> Result<()> {
    let contents = template.render()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent.as_std_path())?;
    }
    fs::write(path.as_std_path(), contents)?;
    Ok(())
}

fn json_string(value: &str) -> Result<String> {
    Ok(serde_json::to_string(value)?)
}

fn json_optional_string(value: Option<&str>) -> Result<String> {
    Ok(serde_json::to_string(&value)?)
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
    namespace_json: String,
    package_name_json: String,
    cdylib_name_json: String,
    node_engine_json: String,
    lib_path_literal_json: String,
    manual_load: bool,
    public_api_js: String,
}

#[derive(Template)]
#[template(path = "component/component.d.ts.j2", escape = "none")]
struct ComponentDtsTemplate {
    namespace: String,
    public_api_dts: String,
}

#[derive(Template)]
#[template(path = "component/component-ffi.js.j2", escape = "none")]
struct ComponentFfiJsTemplate {
    namespace_json: String,
    cdylib_name_json: String,
    lib_path_literal_json: String,
    manual_load: bool,
}

#[derive(Template)]
#[template(path = "component/component-ffi.d.ts.j2", escape = "none")]
struct ComponentFfiDtsTemplate {}

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
    fn config_validation_rejects_platform_switch_packaging() {
        let config = parse_node_config(
            r#"
            [bindings.node]
            out_lib_path_module = ["@scope/example-darwin", "@scope/example-linux"]
            "#,
        );

        let error = config
            .validate()
            .expect_err("platform-switch packaging should be rejected");

        assert!(
            error
                .to_string()
                .contains("multi-package platform-switch packaging"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn config_override_validation_rejects_platform_switch_packaging() {
        let overrides = NodeBindingCliOverrides::from_parts(
            None,
            None,
            None,
            None,
            false,
            vec!["out_lib_path_module=@scope/example-darwin".to_string()],
        )
        .expect("override should parse");
        let mut config = NodeBindingGeneratorConfig::default();

        overrides.apply_to(&mut config);
        let error = config
            .validate()
            .expect_err("platform-switch override should be rejected");

        assert!(
            error
                .to_string()
                .contains("multi-package platform-switch packaging"),
            "unexpected error: {error}"
        );
    }
}
