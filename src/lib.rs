#![forbid(unsafe_code)]

mod cli;
pub mod subcommands;

pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

pub fn run() -> Result<(), cli::CliError> {
    cli::run(std::env::args_os())
}
