use std::fs;

use anyhow::{Result, anyhow, bail};
use camino::Utf8PathBuf;
use serde::Deserialize;
use uniffi_bindgen::{BindingGenerator, Component, GenerationSettings};

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
}

impl Default for NodeBindingGeneratorConfig {
    fn default() -> Self {
        Self {
            package_name: None,
            cdylib_name: None,
            node_engine: ">=16".to_string(),
            lib_path_literal: None,
            manual_load: false,
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

        let package = GeneratedPackageLayout::from_component(settings, component)?;
        package.ensure_root_dir()?;

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
}

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
}
