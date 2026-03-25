use std::ffi::OsString;

use crate::CRATE_NAME;
use crate::cli::CliError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
}

impl Command {
    pub fn from_args<I>(args: I) -> Result<Self, CliError>
    where
        I: IntoIterator<Item = OsString>,
    {
        let mut args = args.into_iter();
        let _program_name = args.next();

        match args.next() {
            None => Ok(Self::Help),
            Some(flag) if flag == "--help" || flag == "-h" => Ok(Self::Help),
            Some(command) => Err(CliError::UnsupportedCommand(format!(
                "unsupported command `{}`\n\n{}",
                command.to_string_lossy(),
                usage()
            ))),
        }
    }
}

pub fn run(command: Command) -> Result<(), CliError> {
    match command {
        Command::Help => {
            println!("{}", usage());
            Ok(())
        }
    }
}

fn usage() -> String {
    format!(
        "{CRATE_NAME}\n\nUsage:\n  {CRATE_NAME} <command>\n\nCommands:\n  generate    Generate Node bindings from a UniFFI component\n\nUse `--help` to show this message."
    )
}
