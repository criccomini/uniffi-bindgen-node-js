use anyhow::Context;
use camino::Utf8PathBuf;
use clap::Args;
use uniffi_bindgen::cargo_metadata::CrateConfigSupplier;

use crate::bindings::NodeBindingGenerator;

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
    let metadata = cargo_metadata::MetadataCommand::new()
        .exec()
        .context("failed to run cargo metadata for UniFFI config discovery")?;
    let config_supplier = CrateConfigSupplier::from(metadata);
    let generator = NodeBindingGenerator::new();

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
