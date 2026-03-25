#![forbid(unsafe_code)]

pub mod bindings;
mod cli;
pub mod subcommands;

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

pub fn run() -> anyhow::Result<()> {
    cli::run(std::env::args_os())
}
