use std::{
    collections::{HashMap, HashSet},
    fs,
};

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::{Metadata, Package, PackageId, Resolve, Target};
use uniffi_bindgen::{BindgenPaths, BindgenPathsLayer};

pub(crate) fn build_bindgen_paths(manifest_path: Option<&Utf8Path>) -> Result<BindgenPaths> {
    let manifest_layer = manifest_path
        .map(|path| CargoMetadataPathsLayer::from_manifest_path(path, false))
        .transpose()?;
    let workspace_layer = CargoMetadataPathsLayer::from_workspace(false)
        .context("failed to build workspace BindgenPaths layer from cargo metadata")?;

    Ok(layer_v2_bindgen_paths(manifest_layer, workspace_layer))
}

pub(crate) fn resolve_cdylib_target_name(
    manifest_path: Option<&Utf8Path>,
    selected_component_crate_name: &str,
) -> Result<Option<String>> {
    let metadata = load_cargo_metadata(manifest_path, false)?;
    Ok(resolve_cdylib_target_name_from_metadata(
        &metadata,
        selected_component_crate_name,
    ))
}

#[derive(Debug, Clone, Default)]
struct CargoMetadataPathsLayer {
    crate_roots: HashMap<String, Utf8PathBuf>,
}

impl CargoMetadataPathsLayer {
    fn from_workspace(no_deps: bool) -> Result<Self> {
        Ok(Self::from(load_cargo_metadata(None, no_deps)?))
    }

    fn from_manifest_path(manifest_path: &Utf8Path, no_deps: bool) -> Result<Self> {
        Ok(Self::from(load_cargo_metadata(
            Some(manifest_path),
            no_deps,
        )?))
    }
}

fn load_cargo_metadata(manifest_path: Option<&Utf8Path>, no_deps: bool) -> Result<Metadata> {
    let mut command = cargo_metadata::MetadataCommand::new();
    if let Some(manifest_path) = manifest_path {
        command.manifest_path(manifest_path.as_std_path());
    }
    if no_deps {
        command.no_deps();
    }
    command.exec().with_context(|| match manifest_path {
        Some(manifest_path) => {
            format!(
                "error running cargo metadata for manifest '{}'",
                manifest_path
            )
        }
        None => "error running cargo metadata".to_string(),
    })
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

fn resolve_cdylib_target_name_from_metadata(
    metadata: &Metadata,
    selected_component_crate_name: &str,
) -> Option<String> {
    if let Some(root_package) = metadata.root_package()
        && let Some(root_target_name) = exactly_one_cdylib_target_name(root_package)
    {
        return Some(root_target_name);
    }

    let selected_package_ids =
        selected_component_package_ids(metadata, selected_component_crate_name);
    if let Some(target_name) = exactly_one_cdylib_target_name_from_packages(
        metadata
            .packages
            .iter()
            .filter(|package| selected_package_ids.contains(&package.id)),
    ) {
        return Some(target_name);
    }

    if let Some(target_name) = metadata.resolve.as_ref().and_then(|resolve| {
        exactly_one_cdylib_target_name_from_packages(metadata.packages.iter().filter(|package| {
            selected_package_ids.contains(&package.id)
                || package_depends_on_any(resolve, &package.id, &selected_package_ids)
        }))
    }) {
        return Some(target_name);
    }

    exactly_one_cdylib_target_name_from_packages(metadata.packages.iter())
}

fn selected_component_package_ids(
    metadata: &Metadata,
    selected_component_crate_name: &str,
) -> HashSet<PackageId> {
    let normalized_crate_name = normalize_cargo_name(selected_component_crate_name);
    metadata
        .packages
        .iter()
        .filter(|package| package_matches_component(package, &normalized_crate_name))
        .map(|package| package.id.clone())
        .collect()
}

fn package_matches_component(package: &Package, normalized_crate_name: &str) -> bool {
    normalize_cargo_name(&package.name) == normalized_crate_name
        || package
            .targets
            .iter()
            .any(|target| normalize_cargo_name(&target.name) == normalized_crate_name)
}

fn package_depends_on_any(
    resolve: &Resolve,
    package_id: &PackageId,
    selected_package_ids: &HashSet<PackageId>,
) -> bool {
    if selected_package_ids.is_empty() {
        return false;
    }

    let mut pending = vec![package_id.clone()];
    let mut visited = HashSet::new();
    while let Some(current_id) = pending.pop() {
        if !visited.insert(current_id.clone()) {
            continue;
        }
        if selected_package_ids.contains(&current_id) {
            return true;
        }
        let current_node = &resolve[&current_id];
        pending.extend(current_node.dependencies.iter().cloned());
    }
    false
}

fn exactly_one_cdylib_target_name_from_packages<'a>(
    packages: impl IntoIterator<Item = &'a Package>,
) -> Option<String> {
    let mut target_names = packages
        .into_iter()
        .flat_map(package_cdylib_targets)
        .map(|target| target.name.clone())
        .collect::<HashSet<_>>()
        .into_iter();
    let target_name = target_names.next()?;
    target_names.next().is_none().then_some(target_name)
}

fn exactly_one_cdylib_target_name(package: &Package) -> Option<String> {
    exactly_one_cdylib_target_name_from_packages([package])
}

fn package_cdylib_targets(package: &Package) -> impl Iterator<Item = &Target> {
    package.targets.iter().filter(|target| target.is_cdylib())
}

fn normalize_cargo_name(name: &str) -> String {
    name.replace('-', "_")
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

// GENERATED CODE
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
