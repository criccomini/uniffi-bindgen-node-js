use anyhow::{Context, bail};
use camino::Utf8PathBuf;
use clap::Args;
use uniffi_bindgen::cargo_metadata::CrateConfigSupplier;

use crate::bindings::{NodeBindingCliOverrides, NodeBindingGenerator};

#[derive(Debug, Clone, Args)]
pub struct GenerateArgs {
    pub lib_source: Utf8PathBuf,

    #[arg(long)]
    pub crate_name: String,

    #[arg(long)]
    pub out_dir: Utf8PathBuf,

    #[arg(long)]
    pub package_name: Option<String>,

    #[arg(long)]
    pub cdylib_name: Option<String>,

    #[arg(long)]
    pub node_engine: Option<String>,

    #[arg(long)]
    pub lib_path_literal: Option<String>,

    #[arg(long)]
    pub manual_load: bool,

    #[arg(long)]
    pub config_override: Vec<String>,
}

pub fn run(args: GenerateArgs) -> anyhow::Result<()> {
    validate_args(&args)?;

    let metadata = cargo_metadata::MetadataCommand::new()
        .exec()
        .context("failed to run cargo metadata for UniFFI config discovery")?;
    let config_supplier = CrateConfigSupplier::from(metadata);
    let cli_overrides = NodeBindingCliOverrides::from_parts(
        args.package_name.clone(),
        args.cdylib_name.clone(),
        args.node_engine.clone(),
        args.lib_path_literal.clone(),
        args.manual_load,
        args.config_override.clone(),
    )?;
    let generator = NodeBindingGenerator::new(cli_overrides);

    uniffi_bindgen::library_mode::generate_bindings(
        &args.lib_source,
        Some(args.crate_name.clone()),
        &generator,
        &config_supplier,
        None::<&camino::Utf8Path>,
        &args.out_dir,
        false,
    )
    .with_context(|| {
        format!(
            "failed to generate Node bindings for crate '{}' from '{}'",
            args.crate_name, args.lib_source
        )
    })?;

    Ok(())
}

fn validate_args(args: &GenerateArgs) -> anyhow::Result<()> {
    if args.crate_name.trim().is_empty() {
        bail!("--crate-name cannot be empty");
    }
    if args.out_dir.as_str().trim().is_empty() {
        bail!("--out-dir cannot be empty");
    }
    if args.out_dir.exists() && !args.out_dir.is_dir() {
        bail!("--out-dir '{}' exists but is not a directory", args.out_dir);
    }
    if args.lib_source.as_str().trim().is_empty() {
        bail!("lib_source cannot be empty");
    }
    if !args.lib_source.exists() {
        bail!("library source '{}' does not exist", args.lib_source);
    }
    if !args.lib_source.is_file() {
        bail!("library source '{}' is not a file", args.lib_source);
    }
    if !uniffi_bindgen::is_cdylib(&args.lib_source) {
        bail!(
            "library source '{}' is not a supported cdylib (.so, .dylib, or .dll)",
            args.lib_source
        );
    }

    Ok(())
}
