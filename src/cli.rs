use std::ffi::OsString;
use std::fmt;

use crate::subcommands::{self, Command};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    InvalidArguments(String),
    UnsupportedCommand(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArguments(message) | Self::UnsupportedCommand(message) => {
                f.write_str(message)
            }
        }
    }
}

impl std::error::Error for CliError {}

pub fn run<I>(args: I) -> Result<(), CliError>
where
    I: IntoIterator<Item = OsString>,
{
    let command = Command::from_args(args)?;
    subcommands::run(command)
}
