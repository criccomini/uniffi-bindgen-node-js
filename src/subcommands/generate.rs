use anyhow::bail;
use camino::Utf8PathBuf;
use clap::Args;

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

pub fn run(_args: GenerateArgs) -> anyhow::Result<()> {
    bail!("the `generate` subcommand is not implemented yet")
}
