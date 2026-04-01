mod component_selection;
pub(crate) mod config;
mod paths;

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use uniffi_bindgen::{BindgenLoader, Component};

use self::component_selection::{normalize_crate_name_selector, select_component};
use self::config::{
    NodeBindingCliOverrides, NodeBindingGeneratorConfig, apply_component_renames,
    finalize_node_binding_config, parse_node_binding_config,
};
use self::paths::build_bindgen_paths;
use crate::bindings::write_generated_package;

#[derive(Debug, Clone)]
pub struct GenerateNodePackageOptions {
    pub lib_source: Utf8PathBuf,
    pub manifest_path: Option<Utf8PathBuf>,
    pub crate_name: Option<String>,
    pub out_dir: Utf8PathBuf,
    pub package_name: Option<String>,
    pub node_engine: Option<String>,
    pub bundled_prebuilds: bool,
    pub manual_load: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct GenerateNodePackageCliOverrides {
    pub cdylib_name: Option<String>,
    pub lib_path_literal: Option<String>,
    pub config_override: Vec<String>,
}

pub fn generate_node_package(options: GenerateNodePackageOptions) -> Result<()> {
    generate_node_package_with_cli_overrides(options, GenerateNodePackageCliOverrides::default())
}

pub(crate) fn generate_node_package_with_cli_overrides(
    options: GenerateNodePackageOptions,
    cli_compat_overrides: GenerateNodePackageCliOverrides,
) -> Result<()> {
    let cli_overrides = NodeBindingCliOverrides::from_parts(
        options.package_name.clone(),
        cli_compat_overrides.cdylib_name,
        options.node_engine.clone(),
        cli_compat_overrides.lib_path_literal,
        options.bundled_prebuilds,
        options.manual_load,
        cli_compat_overrides.config_override,
    )?;
    let paths = build_bindgen_paths(options.manifest_path.as_deref())
        .context("failed to build BindgenPaths for node package generation")?;

    let loader = BindgenLoader::new(paths);
    let metadata = loader.load_metadata(&options.lib_source).with_context(|| {
        format!(
            "failed to load UniFFI metadata from '{}'",
            options.lib_source
        )
    })?;
    let cis = loader.load_cis(metadata).with_context(|| {
        format!(
            "failed to load UniFFI component interfaces from '{}'",
            options.lib_source
        )
    })?;
    let mut components = loader
        .load_components(cis, |_, root_toml| parse_node_binding_config(&root_toml))
        .with_context(|| {
            format!(
                "failed to load UniFFI component configs from '{}'",
                options.lib_source
            )
        })?;

    finalize_component_configs(
        &mut components,
        loader.library_name(&options.lib_source),
        &cli_overrides,
    )?;
    apply_component_renames(&mut components);

    let normalized_crate_name = options
        .crate_name
        .as_deref()
        .map(normalize_crate_name_selector);
    let mut component = select_component(components, normalized_crate_name.as_deref())?;
    component.ci.derive_ffi_funcs().with_context(|| {
        format!(
            "failed to derive FFI functions for crate '{}'",
            component.ci.crate_name()
        )
    })?;

    write_generated_package(&options.out_dir, &component).with_context(|| {
        format!(
            "failed to generate Node bindings for crate '{}' from '{}'",
            component.ci.crate_name(),
            options.lib_source
        )
    })
}

fn finalize_component_configs(
    components: &mut [Component<NodeBindingGeneratorConfig>],
    cdylib_name: Option<&str>,
    cli_overrides: &NodeBindingCliOverrides,
) -> Result<()> {
    for component in components {
        finalize_node_binding_config(
            &component.ci,
            &mut component.config,
            cdylib_name,
            cli_overrides,
        )?;
    }
    Ok(())
}
