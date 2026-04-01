use std::collections::HashMap;

use anyhow::{Result, bail};
use serde::Deserialize;
use uniffi_bindgen::{Component, ComponentInterface, interface::rename as apply_ci_rename};

#[derive(Debug, Clone, Default)]
pub(crate) struct NodeBindingCliOverrides {
    package_name: Option<String>,
    node_engine: Option<String>,
    lib_path_literal: Option<String>,
    bundled_prebuilds: bool,
    manual_load: bool,
}

impl NodeBindingCliOverrides {
    pub fn from_parts(
        package_name: Option<String>,
        node_engine: Option<String>,
        lib_path_literal: Option<String>,
        bundled_prebuilds: bool,
        manual_load: bool,
    ) -> Result<Self> {
        Ok(Self {
            package_name: normalize_optional_value("--package-name", package_name)?,
            node_engine: normalize_optional_value("--node-engine", node_engine)?,
            lib_path_literal: normalize_optional_value("--lib-path-literal", lib_path_literal)?,
            bundled_prebuilds,
            manual_load,
        })
    }

    pub(crate) fn apply_to(&self, config: &mut NodeBindingGeneratorConfig) {
        apply_optional_string_override(&mut config.package_name, self.package_name.as_ref());
        apply_optional_value_override(&mut config.node_engine, self.node_engine.as_ref());
        apply_optional_string_override(
            &mut config.lib_path_literal,
            self.lib_path_literal.as_ref(),
        );
        enable_override(&mut config.bundled_prebuilds, self.bundled_prebuilds);
        enable_override(&mut config.manual_load, self.manual_load);
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
pub(crate) struct NodeBindingGeneratorConfig {
    pub package_name: Option<String>,
    pub cdylib_name: Option<String>,
    pub node_engine: String,
    pub lib_path_literal: Option<String>,
    pub bundled_prebuilds: bool,
    pub manual_load: bool,
    #[serde(alias = "module-format")]
    pub module_format: Option<String>,
    pub commonjs: Option<bool>,
    #[serde(default)]
    pub rename: toml::Table,
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
            rename: toml::Table::new(),
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

pub(crate) fn apply_component_renames(components: &mut [Component<NodeBindingGeneratorConfig>]) {
    let mut module_renames = HashMap::new();
    for component in components.iter() {
        if !component.config.rename.is_empty() {
            module_renames.insert(
                component.ci.crate_name().to_string(),
                component.config.rename.clone(),
            );
        }
    }

    if module_renames.is_empty() {
        return;
    }

    for component in components.iter_mut() {
        apply_ci_rename(&mut component.ci, &module_renames);
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use uniffi_bindgen::Component;

    use super::{
        NodeBindingCliOverrides, NodeBindingGeneratorConfig, apply_component_renames,
        finalize_node_binding_config, parse_node_binding_config,
    };

    fn parse_node_config(source: &str) -> NodeBindingGeneratorConfig {
        let root = toml::from_str::<toml::Value>(source).expect("test TOML should deserialize");
        parse_node_binding_config(&root).expect("node config should deserialize")
    }

    #[test]
    fn rename_config_is_applied_before_deriving_ffi_functions() -> Result<()> {
        let mut components = vec![Component {
            ci: uniffi_bindgen::ComponentInterface::from_webidl(
                r#"
                namespace example {
                    u32 add(u32 first_value, u32 second_value);
                };
                "#,
                "fixture_crate",
            )?,
            config: parse_node_config(
                r#"
                [bindings.node.rename]
                "add.first_value" = "lhs"
                "add.second_value" = "rhs"
                "#,
            ),
        }];

        apply_component_renames(&mut components);
        components[0].ci.derive_ffi_funcs()?;

        let ffi_function = components[0]
            .ci
            .iter_user_ffi_function_definitions()
            .next()
            .expect("expected derived FFI function");
        let argument_names = ffi_function
            .arguments()
            .into_iter()
            .map(|argument| argument.name().to_string())
            .collect::<Vec<_>>();

        assert_eq!(argument_names, vec!["lhs", "rhs"]);
        Ok(())
    }

    #[test]
    fn finalize_config_defaults_cdylib_name_from_loaded_library_name() -> Result<()> {
        let ci = uniffi_bindgen::ComponentInterface::from_webidl(
            r#"
            namespace example {
                u32 add(u32 lhs, u32 rhs);
            };
            "#,
            "fixture_crate",
        )?;
        let mut config = NodeBindingGeneratorConfig::default();

        finalize_node_binding_config(
            &ci,
            &mut config,
            Some("fixture_from_loader"),
            &NodeBindingCliOverrides::default(),
        )?;

        assert_eq!(config.cdylib_name.as_deref(), Some("fixture_from_loader"));
        Ok(())
    }
}
