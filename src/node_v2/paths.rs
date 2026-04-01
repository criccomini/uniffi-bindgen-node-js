use std::{collections::HashMap, fs};

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::Metadata;
use uniffi_bindgen::{BindgenPaths, BindgenPathsLayer};

pub(crate) fn build_bindgen_paths(manifest_path: Option<&Utf8Path>) -> Result<BindgenPaths> {
    let manifest_layer = manifest_path
        .map(|path| CargoMetadataPathsLayer::from_manifest_path(path, false))
        .transpose()?;
    let workspace_layer = CargoMetadataPathsLayer::from_workspace(false)
        .context("failed to build workspace BindgenPaths layer from cargo metadata")?;

    Ok(layer_v2_bindgen_paths(manifest_layer, workspace_layer))
}

#[derive(Debug, Clone, Default)]
struct CargoMetadataPathsLayer {
    crate_roots: HashMap<String, Utf8PathBuf>,
}

impl CargoMetadataPathsLayer {
    fn from_workspace(no_deps: bool) -> Result<Self> {
        let mut command = cargo_metadata::MetadataCommand::new();
        if no_deps {
            command.no_deps();
        }
        let metadata = command.exec().context("error running cargo metadata")?;
        Ok(Self::from(metadata))
    }

    fn from_manifest_path(manifest_path: &Utf8Path, no_deps: bool) -> Result<Self> {
        let mut command = cargo_metadata::MetadataCommand::new();
        command.manifest_path(manifest_path.as_std_path());
        if no_deps {
            command.no_deps();
        }
        let metadata = command.exec().with_context(|| {
            format!(
                "error running cargo metadata for manifest '{}'",
                manifest_path
            )
        })?;
        Ok(Self::from(metadata))
    }
}

fn layer_v2_bindgen_paths<M, W>(manifest_layer: Option<M>, workspace_layer: W) -> BindgenPaths
where
    M: BindgenPathsLayer + 'static,
    W: BindgenPathsLayer + 'static,
{
    let mut paths = BindgenPaths::default();

    // UniFFI resolves BindgenPaths layers in insertion order, so the explicit
    // --manifest-path hint must win before falling back to workspace discovery.
    if let Some(manifest_layer) = manifest_layer {
        paths.add_layer(manifest_layer);
    }
    paths.add_layer(workspace_layer);

    paths
}

impl From<Metadata> for CargoMetadataPathsLayer {
    fn from(metadata: Metadata) -> Self {
        let crate_roots = metadata
            .packages
            .iter()
            .flat_map(|package| {
                package
                    .targets
                    .iter()
                    .filter(|target| {
                        !target.is_bin()
                            && !target.is_example()
                            && !target.is_test()
                            && !target.is_bench()
                            && !target.is_custom_build()
                    })
                    .filter_map(|target| {
                        package
                            .manifest_path
                            .parent()
                            .map(|root| (target.name.replace('-', "_"), root.to_owned()))
                    })
            })
            .collect();

        Self { crate_roots }
    }
}

impl BindgenPathsLayer for CargoMetadataPathsLayer {
    fn get_config(&self, crate_name: &str) -> Result<Option<toml::value::Table>> {
        let Some(crate_root) = self.crate_roots.get(crate_name) else {
            return Ok(None);
        };

        let config_path = crate_root.join("uniffi.toml");
        if !config_path.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(&config_path)
            .with_context(|| format!("read file: {:?}", config_path))?;
        let toml = toml::de::from_str(&contents)
            .with_context(|| format!("parse toml: {:?}", config_path))?;
        Ok(Some(toml))
    }

    fn get_udl_path(&self, crate_name: &str, udl_name: &str) -> Option<Utf8PathBuf> {
        self.crate_roots
            .get(crate_name)
            .map(|root| root.join("src").join(format!("{udl_name}.udl")))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use anyhow::Result;

    use super::*;

    #[derive(Clone, Default)]
    struct StaticConfigLayer {
        configs: HashMap<&'static str, toml::value::Table>,
    }

    impl StaticConfigLayer {
        fn new(crate_name: &'static str, package_name: &'static str) -> Self {
            Self::from_configs([(crate_name, package_name)])
        }

        fn from_configs(configs: impl IntoIterator<Item = (&'static str, &'static str)>) -> Self {
            Self {
                configs: configs
                    .into_iter()
                    .map(|(crate_name, package_name)| {
                        (
                            crate_name,
                            toml::from_str(&format!(
                                "[bindings.node]\npackage_name = \"{package_name}\"\n"
                            ))
                            .expect("static TOML should parse"),
                        )
                    })
                    .collect(),
            }
        }
    }

    impl BindgenPathsLayer for StaticConfigLayer {
        fn get_config(&self, crate_name: &str) -> Result<Option<toml::value::Table>> {
            Ok(self.configs.get(crate_name).cloned())
        }
    }

    #[test]
    fn manifest_layer_precedes_workspace_discovery() {
        let paths = layer_v2_bindgen_paths(
            Some(StaticConfigLayer::new(
                "fixture_callbacks",
                "manifest-layer-package",
            )),
            StaticConfigLayer::new("fixture_callbacks", "workspace-layer-package"),
        );

        let config = paths
            .get_config("fixture_callbacks")
            .expect("config lookup should succeed");
        let package_name = config
            .get("bindings")
            .and_then(|bindings| bindings.get("node"))
            .and_then(|node| node.get("package_name"))
            .and_then(|value| value.as_str());

        assert_eq!(package_name, Some("manifest-layer-package"));
    }

    #[test]
    fn config_lookup_is_deterministic_with_manifest_and_workspace_layers() {
        let paths = layer_v2_bindgen_paths(
            Some(StaticConfigLayer::from_configs([(
                "fixture_callbacks",
                "manifest-layer-package",
            )])),
            StaticConfigLayer::from_configs([
                ("fixture_callbacks", "workspace-layer-package"),
                ("fixture_basic", "workspace-fallback-package"),
            ]),
        );

        let overlapping_config = paths
            .get_config("fixture_callbacks")
            .expect("overlapping config lookup should succeed");
        let fallback_config = paths
            .get_config("fixture_basic")
            .expect("workspace fallback config lookup should succeed");
        let repeated_config = paths
            .get_config("fixture_callbacks")
            .expect("repeated config lookup should succeed");

        let overlapping_crate = extract_package_name(&overlapping_config);
        let fallback_crate = extract_package_name(&fallback_config);
        let repeated_lookup = extract_package_name(&repeated_config);

        assert_eq!(overlapping_crate, Some("manifest-layer-package"));
        assert_eq!(fallback_crate, Some("workspace-fallback-package"));
        assert_eq!(repeated_lookup, Some("manifest-layer-package"));
    }

    fn extract_package_name(config: &toml::value::Table) -> Option<&str> {
        config
            .get("bindings")
            .and_then(|bindings| bindings.get("node"))
            .and_then(|node| node.get("package_name"))
            .and_then(|value| value.as_str())
    }
}
