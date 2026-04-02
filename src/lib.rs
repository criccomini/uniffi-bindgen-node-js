#![forbid(unsafe_code)]

pub mod bindings;
mod cli;
pub(crate) mod node;
pub mod subcommands;

pub use node::{GenerateNodePackageOptions, generate_node_package};

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

pub fn run() -> anyhow::Result<()> {
    cli::run(std::env::args_os())
}
