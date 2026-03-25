pub mod generate;

use clap::{Parser, Subcommand};

use crate::CRATE_NAME;

#[derive(Debug, Parser)]
#[command(name = CRATE_NAME, about = "Generate Node.js bindings for UniFFI components")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Subcommand)]
pub enum Command {
    Generate(generate::GenerateArgs),
}

pub fn run(command: Command) -> anyhow::Result<()> {
    match command {
        Command::Generate(args) => generate::run(args),
    }
}
