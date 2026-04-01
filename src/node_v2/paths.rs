use std::{collections::HashMap, fs};

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::Metadata;
use uniffi_bindgen::{BindgenPaths, BindgenPathsLayer};

pub(crate) fn build_bindgen_paths(manifest_path: Option<&Utf8Path>) -> Result<BindgenPaths> {
    let mut paths = BindgenPaths::default();

    if let Some(manifest_path) = manifest_path {
        paths.add_layer(CargoMetadataPathsLayer::from_manifest_path(manifest_path, false)?);
    }

    paths
        .add_cargo_metadata_layer(false)
        .context("failed to build workspace BindgenPaths layer from cargo metadata")?;

    Ok(paths)
}

#[derive(Debug, Clone, Default)]
struct CargoMetadataPathsLayer {
    crate_roots: HashMap<String, Utf8PathBuf>,
}

impl CargoMetadataPathsLayer {
    fn from_manifest_path(manifest_path: &Utf8Path, no_deps: bool) -> Result<Self> {
        let mut command = cargo_metadata::MetadataCommand::new();
        command.manifest_path(manifest_path.as_std_path());
        if no_deps {
            command.no_deps();
        }
        let metadata = command.exec().with_context(|| {
            format!("error running cargo metadata for manifest '{}'", manifest_path)
        })?;
        Ok(Self::from(metadata))
    }
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
