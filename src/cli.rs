use std::ffi::OsString;

use clap::Parser;

use crate::subcommands::{self, Cli};

pub fn run<I>(args: I) -> anyhow::Result<()>
where
    I: IntoIterator<Item = OsString>,
{
    let cli = Cli::parse_from(args);
    subcommands::run(cli.command)
}
