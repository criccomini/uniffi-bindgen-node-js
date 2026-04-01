use anyhow::{Result, anyhow, bail};
use serde::Deserialize;
use uniffi_bindgen::ComponentInterface;

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

    pub(crate) fn apply_to(&self, config: &mut NodeBindingGeneratorConfig) {
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
    pub(crate) fn validate(&self) -> Result<()> {
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

pub(crate) fn parse_node_binding_config(
    root_toml: &toml::Value,
) -> Result<NodeBindingGeneratorConfig> {
    Ok(
        match root_toml
            .get("bindings")
            .and_then(|bindings| bindings.get("node"))
        {
            Some(value) => value.clone().try_into()?,
            None => NodeBindingGeneratorConfig::default(),
        },
    )
}

pub(crate) fn finalize_node_binding_config(
    ci: &ComponentInterface,
    config: &mut NodeBindingGeneratorConfig,
    cdylib_name: Option<&str>,
    cli_overrides: &NodeBindingCliOverrides,
) -> Result<()> {
    if config.package_name.is_none() {
        config.package_name = Some(ci.namespace().to_string());
    }
    if config.cdylib_name.is_none() {
        config.cdylib_name = cdylib_name.map(str::to_string);
    }
    cli_overrides.apply_to(config);
    config.validate()
}
