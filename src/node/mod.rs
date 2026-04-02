mod component_selection;
pub(crate) mod config;
mod paths;
mod validation;

use anyhow::{Context, Result, anyhow};
use camino::Utf8PathBuf;
use uniffi_bindgen::{BindgenLoader, Component};

use self::component_selection::{normalize_crate_name_selector, select_component};
use self::config::{
    NodePackageCliOverrides, NodePackageConfig, apply_component_renames,
    finalize_node_package_config, parse_node_package_config,
};
use self::paths::build_bindgen_paths;
use self::validation::validate_generate_options;
use crate::bindings::{NodePackageSpec, write_generated_package};

type NodeComponent = Component<NodePackageConfig>;

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

struct LoadedNodePackageInputs {
    library_name: String,
    components: Vec<NodeComponent>,
}

pub fn generate_node_package(options: GenerateNodePackageOptions) -> Result<()> {
    validate_generate_options(&options)?;

    let cli_overrides = build_cli_overrides(&options)?;
    let LoadedNodePackageInputs {
        library_name,
        components,
    } = load_node_package_inputs(&options)?;
    let component = prepare_selected_component(components, &options, &cli_overrides)?;
    let package_spec = build_package_spec(&component, &library_name)?;

    write_selected_component_package(&options, &component, &package_spec)
}

fn build_cli_overrides(options: &GenerateNodePackageOptions) -> Result<NodePackageCliOverrides> {
    NodePackageCliOverrides::from_parts(
        options.package_name.clone(),
        options.node_engine.clone(),
        options.bundled_prebuilds,
        options.manual_load,
    )
}

fn load_node_package_inputs(
    options: &GenerateNodePackageOptions,
) -> Result<LoadedNodePackageInputs> {
    Ok(LoadedNodePackageInputs {
        library_name: resolve_library_name(options)?,
        components: load_node_components(options)?,
    })
}

fn prepare_selected_component(
    components: Vec<NodeComponent>,
    options: &GenerateNodePackageOptions,
    cli_overrides: &NodePackageCliOverrides,
) -> Result<NodeComponent> {
    let mut component = select_configured_component(components, options, cli_overrides)?;
    derive_component_ffi(&mut component)?;
    Ok(component)
}

fn build_bindgen_loader(options: &GenerateNodePackageOptions) -> Result<BindgenLoader> {
    let paths = build_bindgen_paths(options.manifest_path.as_deref())
        .context("failed to build BindgenPaths for node package generation")?;
    Ok(BindgenLoader::new(paths))
}

fn resolve_library_name(options: &GenerateNodePackageOptions) -> Result<String> {
    let loader = build_bindgen_loader(options)?;
    loader
        .library_name(&options.lib_source)
        .map(str::to_string)
        .ok_or_else(|| {
            anyhow!(
                "failed to determine the native library name from '{}'",
                options.lib_source
            )
        })
}

fn load_node_components(options: &GenerateNodePackageOptions) -> Result<Vec<NodeComponent>> {
    let loader = build_bindgen_loader(options)?;
    let metadata = loader.load_metadata(&options.lib_source).with_context(|| {
        format!(
            "failed to load UniFFI metadata from '{}'",
            options.lib_source
        )
    })?;
    let component_interfaces = loader.load_cis(metadata).with_context(|| {
        format!(
            "failed to load UniFFI component interfaces from '{}'",
            options.lib_source
        )
    })?;

    loader
        .load_components(component_interfaces, |_, root_toml| {
            parse_node_package_config(&root_toml)
        })
        .with_context(|| {
            format!(
                "failed to load UniFFI component configs from '{}'",
                options.lib_source
            )
        })
}

fn select_configured_component(
    mut components: Vec<NodeComponent>,
    options: &GenerateNodePackageOptions,
    cli_overrides: &NodePackageCliOverrides,
) -> Result<NodeComponent> {
    finalize_component_configs(&mut components, cli_overrides)?;
    apply_component_renames(&mut components);

    let normalized_crate_name = options
        .crate_name
        .as_deref()
        .map(normalize_crate_name_selector);
    select_component(components, normalized_crate_name.as_deref())
}

fn derive_component_ffi(component: &mut NodeComponent) -> Result<()> {
    component.ci.derive_ffi_funcs().with_context(|| {
        format!(
            "failed to derive FFI functions for crate '{}'",
            component.ci.crate_name()
        )
    })
}

fn write_selected_component_package(
    options: &GenerateNodePackageOptions,
    component: &NodeComponent,
    package_spec: &NodePackageSpec,
) -> Result<()> {
    write_generated_package(
        &options.out_dir,
        &options.lib_source,
        &component.ci,
        package_spec,
    )
    .with_context(|| {
        format!(
            "failed to generate Node bindings for crate '{}' from '{}'",
            component.ci.crate_name(),
            options.lib_source
        )
    })
}

fn finalize_component_configs(
    components: &mut [NodeComponent],
    cli_overrides: &NodePackageCliOverrides,
) -> Result<()> {
    for component in components {
        finalize_node_package_config(&component.ci, &mut component.config, cli_overrides)?;
    }
    Ok(())
}

fn build_package_spec(component: &NodeComponent, library_name: &str) -> Result<NodePackageSpec> {
    let package_name = component.config.package_name.clone().ok_or_else(|| {
        anyhow!(
            "node package generation requires a resolved package name for crate '{}'",
            component.ci.crate_name()
        )
    })?;

    Ok(NodePackageSpec {
        package_name,
        library_name: library_name.to_string(),
        node_engine: component.config.node_engine.clone(),
        bundled_prebuilds: component.config.bundled_prebuilds,
        manual_load: component.config.manual_load,
    })
}
