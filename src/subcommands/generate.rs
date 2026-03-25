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
}

pub fn run(_args: GenerateArgs) -> anyhow::Result<()> {
    bail!("the `generate` subcommand is not implemented yet")
}
