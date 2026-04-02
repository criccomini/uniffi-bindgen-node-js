use std::collections::HashMap;

use anyhow::{Result, bail};
use serde::Deserialize;
use uniffi_bindgen::{Component, ComponentInterface, interface::rename as apply_ci_rename};

const REMOVED_NODE_CONFIG_KEYS: &[RemovedNodeConfigKey] = &[
    RemovedNodeConfigKey {
        key: "cdylib_name",
        guidance: "the generator now derives the expected packaged native library name from the input cdylib, so delete this setting",
    },
    RemovedNodeConfigKey {
        key: "lib_path_literal",
        guidance: "generated loaders now derive the default packaged native library path, so delete this setting and either place the library there during packaging or call load(path)",
    },
    RemovedNodeConfigKey {
        key: "module_format",
        guidance: "generated Node packages are ESM-only, so delete this setting instead of selecting a module format",
    },
    RemovedNodeConfigKey {
        key: "module-format",
        guidance: "generated Node packages are ESM-only, so delete this setting instead of selecting a module format",
    },
    RemovedNodeConfigKey {
        key: "commonjs",
        guidance: "generated Node packages are ESM-only, so delete this setting instead of requesting CommonJS output",
    },
    RemovedNodeConfigKey {
        key: "lib_path_module",
        guidance: "native library path modules are no longer configurable, so delete this setting",
    },
    RemovedNodeConfigKey {
        key: "lib_path_modules",
        guidance: "native library path modules are no longer configurable, so delete this setting",
    },
    RemovedNodeConfigKey {
        key: "out_lib_path_module",
        guidance: "native library path modules are no longer configurable, so delete this setting",
    },
    RemovedNodeConfigKey {
        key: "out_lib_path_modules",
        guidance: "native library path modules are no longer configurable, so delete this setting",
    },
];

#[derive(Debug, Clone, Copy)]
struct RemovedNodeConfigKey {
    key: &'static str,
    guidance: &'static str,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct NodePackageCliOverrides {
    package_name: Option<String>,
    node_engine: Option<String>,
    bundled_prebuilds: bool,
    manual_load: bool,
}

impl NodePackageCliOverrides {
    pub fn from_parts(
        package_name: Option<String>,
        node_engine: Option<String>,
        bundled_prebuilds: bool,
        manual_load: bool,
    ) -> Result<Self> {
        Ok(Self {
            package_name: normalize_optional_value("--package-name", package_name)?,
            node_engine: normalize_optional_value("--node-engine", node_engine)?,
            bundled_prebuilds,
            manual_load,
        })
    }

    pub(crate) fn apply_to(&self, config: &mut NodePackageConfig) {
        apply_optional_string_override(&mut config.package_name, self.package_name.as_ref());
        apply_optional_value_override(&mut config.node_engine, self.node_engine.as_ref());
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
pub(crate) struct NodePackageConfig {
    pub package_name: Option<String>,
    pub node_engine: String,
    pub bundled_prebuilds: bool,
    pub manual_load: bool,
    #[serde(default)]
    pub rename: toml::Table,
}

impl Default for NodePackageConfig {
    fn default() -> Self {
        Self {
            package_name: None,
            node_engine: ">=16".to_string(),
            bundled_prebuilds: false,
            manual_load: false,
            rename: toml::Table::new(),
        }
    }
}

impl NodePackageConfig {
    pub(crate) fn validate(&self) -> Result<()> {
        self.validate_required_fields()
    }

    fn validate_required_fields(&self) -> Result<()> {
        validate_optional_non_empty("package_name", self.package_name.as_deref())?;
        validate_non_empty("node_engine", &self.node_engine)
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
        bail!("bindings.node.{field_name} cannot be empty");
    }

    Ok(())
}

fn reject_removed_node_config_keys(node_toml: &toml::Value) -> Result<()> {
    let Some(node_table) = node_toml.as_table() else {
        return Ok(());
    };

    let diagnostics = REMOVED_NODE_CONFIG_KEYS
        .iter()
        .filter(|removed_key| node_table.contains_key(removed_key.key))
        .map(|removed_key| {
            format!(
                "- bindings.node.{} is no longer supported; {}",
                removed_key.key, removed_key.guidance
            )
        })
        .collect::<Vec<_>>();

    if diagnostics.is_empty() {
        return Ok(());
    }

    bail!(
        "unsupported legacy [bindings.node] settings:\n{}",
        diagnostics.join("\n")
    );
}

pub(crate) fn parse_node_package_config(root_toml: &toml::Value) -> Result<NodePackageConfig> {
    let Some(node_toml) = root_toml
        .get("bindings")
        .and_then(|bindings| bindings.get("node"))
    else {
        return Ok(NodePackageConfig::default());
    };

    reject_removed_node_config_keys(node_toml)?;
    Ok(node_toml.clone().try_into()?)
}

pub(crate) fn finalize_node_package_config(
    ci: &ComponentInterface,
    config: &mut NodePackageConfig,
    cli_overrides: &NodePackageCliOverrides,
) -> Result<()> {
    if config.package_name.is_none() {
        config.package_name = Some(default_package_name(ci)?);
    }
    cli_overrides.apply_to(config);
    config.validate()
}

fn default_package_name(ci: &ComponentInterface) -> Result<String> {
    let namespace = ci.namespace().trim();
    if namespace.is_empty() {
        bail!("selected UniFFI component namespace cannot be empty");
    }

    Ok(namespace.to_string())
}

pub(crate) fn apply_component_renames(components: &mut [Component<NodePackageConfig>]) {
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
        NodePackageCliOverrides, NodePackageConfig, apply_component_renames,
        finalize_node_package_config, parse_node_package_config,
    };

    fn parse_node_config(source: &str) -> NodePackageConfig {
        let root = toml::from_str::<toml::Value>(source).expect("test TOML should deserialize");
        parse_node_package_config(&root).expect("node config should deserialize")
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
    fn finalize_config_defaults_package_name_from_component_namespace() -> Result<()> {
        let ci = uniffi_bindgen::ComponentInterface::from_webidl(
            r#"
            namespace example {
                u32 add(u32 lhs, u32 rhs);
            };
            "#,
            "fixture_crate",
        )?;
        let mut config = NodePackageConfig::default();

        finalize_node_package_config(&ci, &mut config, &NodePackageCliOverrides::default())?;

        assert_eq!(config.package_name.as_deref(), Some("example"));
        Ok(())
    }

    #[test]
    fn parse_config_rejects_removed_legacy_keys_with_current_diagnostics() {
        let error = toml::from_str::<toml::Value>(
            r#"
            [bindings.node]
            cdylib_name = "fixture_cdylib"
            lib_path_literal = "./native/libfixture.node"
            module_format = "commonjs"
            commonjs = true
            "#,
        )
        .map(|root| parse_node_package_config(&root))
        .expect("test TOML should deserialize")
        .expect_err("legacy config keys should be rejected");

        let message = error.to_string();
        assert!(
            message.contains("unsupported legacy [bindings.node] settings"),
            "unexpected error: {error}"
        );
        assert!(
            message.contains("bindings.node.cdylib_name is no longer supported"),
            "unexpected error: {error}"
        );
        assert!(
            message.contains("bindings.node.lib_path_literal is no longer supported"),
            "unexpected error: {error}"
        );
        assert!(
            message.contains("bindings.node.module_format is no longer supported"),
            "unexpected error: {error}"
        );
        assert!(
            message.contains("generated Node packages are ESM-only"),
            "unexpected error: {error}"
        );
        assert!(
            message.contains("bindings.node.commonjs is no longer supported"),
            "unexpected error: {error}"
        );
        assert!(
            !message.contains("v1"),
            "diagnostics should not mention v1: {error}"
        );
    }
}
