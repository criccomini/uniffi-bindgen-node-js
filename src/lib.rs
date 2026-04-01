#![forbid(unsafe_code)]

pub mod bindings;
mod cli;
pub(crate) mod node_v2;
pub mod subcommands;

pub use node_v2::{GenerateNodePackageOptions, generate_node_package};

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

pub fn run() -> anyhow::Result<()> {
    cli::run(std::env::args_os())
}
