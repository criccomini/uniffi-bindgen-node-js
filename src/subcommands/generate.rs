use crate::GenerateNodePackageOptions;
use crate::node_v2::{GenerateNodePackageCliOverrides, generate_node_package_with_cli_overrides};
use anyhow::bail;
use camino::Utf8PathBuf;
use clap::Args;

#[derive(Debug, Clone, Args)]
pub struct GenerateArgs {
    pub lib_source: Utf8PathBuf,

    #[arg(long)]
    pub manifest_path: Option<Utf8PathBuf>,

    #[arg(long)]
    pub crate_name: Option<String>,

    #[arg(long)]
    pub out_dir: Utf8PathBuf,

    #[arg(long)]
    pub package_name: Option<String>,

    #[arg(long)]
    pub node_engine: Option<String>,

    #[arg(long)]
    pub lib_path_literal: Option<String>,

    #[arg(long)]
    pub bundled_prebuilds: bool,

    #[arg(long)]
    pub manual_load: bool,
}

pub fn run(args: GenerateArgs) -> anyhow::Result<()> {
    validate_args(&args)?;
    generate_node_package_with_cli_overrides(
        GenerateNodePackageOptions {
            lib_source: args.lib_source,
            manifest_path: args.manifest_path,
            crate_name: args.crate_name,
            out_dir: args.out_dir,
            package_name: args.package_name,
            node_engine: args.node_engine,
            bundled_prebuilds: args.bundled_prebuilds,
            manual_load: args.manual_load,
        },
        GenerateNodePackageCliOverrides {
            lib_path_literal: args.lib_path_literal,
        },
    )
}

fn validate_args(args: &GenerateArgs) -> anyhow::Result<()> {
    if let Some(crate_name) = args.crate_name.as_deref() {
        if crate_name.trim().is_empty() {
            bail!("--crate-name cannot be empty");
        }
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
