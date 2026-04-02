use crate::{GenerateNodePackageOptions, generate_node_package};
use anyhow::bail;
use camino::Utf8PathBuf;
use clap::Args;

#[derive(Debug, Clone, Args)]
#[command(
    about = "Generate a self-contained ESM Node package from a built UniFFI cdylib",
    long_about = "Generate a self-contained ESM Node package from a built UniFFI cdylib.\n\nThe generator loads UniFFI component metadata from the native library, selects one component, renders the Node package files, and stages the native library into the generated package layout."
)]
pub struct GenerateArgs {
    /// Path to the built UniFFI cdylib (.so, .dylib, or .dll) to package.
    #[arg(value_name = "LIB_SOURCE")]
    pub lib_source: Utf8PathBuf,

    /// Cargo.toml hint used to resolve UDL and uniffi.toml inputs when needed.
    #[arg(long, value_name = "Cargo.toml")]
    pub manifest_path: Option<Utf8PathBuf>,

    /// Select a component when the library exposes more than one UniFFI component.
    #[arg(long, value_name = "CRATE_NAME")]
    pub crate_name: Option<String>,

    /// Output directory for the generated ESM package.
    #[arg(long, value_name = "OUT_DIR")]
    pub out_dir: Utf8PathBuf,

    /// Override the generated npm package name.
    #[arg(long, value_name = "PACKAGE_NAME")]
    pub package_name: Option<String>,

    /// Override the package.json engines.node range.
    #[arg(long, value_name = "NODE_ENGINE")]
    pub node_engine: Option<String>,

    /// Stage the native library into prebuilds/<host-target>/ instead of the package root.
    #[arg(long)]
    pub bundled_prebuilds: bool,

    /// Emit manual load and unload helpers instead of auto-loading on import.
    #[arg(long)]
    pub manual_load: bool,
}

pub fn run(args: GenerateArgs) -> anyhow::Result<()> {
    validate_args(&args)?;
    generate_node_package(GenerateNodePackageOptions {
        lib_source: args.lib_source,
        manifest_path: args.manifest_path,
        crate_name: args.crate_name,
        out_dir: args.out_dir,
        package_name: args.package_name,
        node_engine: args.node_engine,
        bundled_prebuilds: args.bundled_prebuilds,
        manual_load: args.manual_load,
    })
}

fn validate_args(args: &GenerateArgs) -> anyhow::Result<()> {
    if let Some(crate_name) = args.crate_name.as_deref()
        && crate_name.trim().is_empty()
    {
        bail!(
            "--crate-name cannot be empty; omit it to infer the only UniFFI component in the library"
        );
    }
    if args.out_dir.as_str().trim().is_empty() {
        bail!("--out-dir cannot be empty");
    }
    if args.out_dir.exists() && !args.out_dir.is_dir() {
        bail!("--out-dir '{}' exists but is not a directory", args.out_dir);
    }
    if args.lib_source.as_str().trim().is_empty() {
        bail!("<LIB_SOURCE> cannot be empty");
    }
    if !args.lib_source.exists() {
        bail!("built UniFFI cdylib '{}' does not exist", args.lib_source);
    }
    if !args.lib_source.is_file() {
        bail!("built UniFFI cdylib '{}' is not a file", args.lib_source);
    }
    if !uniffi_bindgen::is_cdylib(&args.lib_source) {
        bail!(
            "built UniFFI cdylib '{}' must end in .so, .dylib, or .dll",
            args.lib_source
        );
    }

    Ok(())
}
