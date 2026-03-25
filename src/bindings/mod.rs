use anyhow::{Result, anyhow, bail};
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
            package_name,
            cdylib_name,
            node_engine,
            lib_path_literal,
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
        let value = raw_value.trim().to_string();

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
        }
        Ok(())
    }

    fn write_bindings(
        &self,
        _settings: &GenerationSettings,
        _components: &[Component<Self::Config>],
    ) -> Result<()> {
        Ok(())
    }
}
