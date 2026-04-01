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

#[cfg(test)]
mod tests {
    use clap::{Parser, error::ErrorKind};

    use super::Cli;

    #[test]
    fn generate_cli_rejects_removed_config_override_flag() {
        let error = Cli::try_parse_from([
            "uniffi-bindgen-node-js",
            "generate",
            "/tmp/libfixture.dylib",
            "--out-dir",
            "/tmp/out",
            "--config-override",
            "commonjs=true",
        ])
        .expect_err("removed --config-override flag should not parse");

        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
        assert!(
            error.to_string().contains("--config-override"),
            "unexpected clap error: {error}"
        );
    }

    #[test]
    fn generate_cli_rejects_removed_cdylib_name_flag() {
        let error = Cli::try_parse_from([
            "uniffi-bindgen-node-js",
            "generate",
            "/tmp/libfixture.dylib",
            "--out-dir",
            "/tmp/out",
            "--cdylib-name",
            "fixture_override",
        ])
        .expect_err("removed --cdylib-name flag should not parse");

        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
        assert!(
            error.to_string().contains("--cdylib-name"),
            "unexpected clap error: {error}"
        );
    }
}
