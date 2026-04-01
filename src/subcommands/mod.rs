pub mod generate;

use clap::{Parser, Subcommand};

use crate::CRATE_NAME;

#[derive(Debug, Parser)]
#[command(
    name = CRATE_NAME,
    about = "Generate a self-contained ESM Node package for a built UniFFI cdylib"
)]
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
    use clap::{CommandFactory, Parser, error::ErrorKind};

    use super::Cli;

    #[test]
    fn generate_cli_help_describes_v2_surface() {
        let mut command = Cli::command();
        let generate = command
            .find_subcommand_mut("generate")
            .expect("generate subcommand should exist");
        let help = generate.render_long_help().to_string();

        assert!(
            help.contains("Generate a self-contained ESM Node package from a built UniFFI cdylib"),
            "unexpected help output: {help}"
        );
        assert!(
            help.contains("--manifest-path <Cargo.toml>"),
            "unexpected help output: {help}"
        );
        assert!(
            help.contains("prebuilds/<host-target>/"),
            "unexpected help output: {help}"
        );
        assert!(
            help.contains("--manual-load"),
            "unexpected help output: {help}"
        );
        assert!(
            !help.contains("--config-override")
                && !help.contains("--cdylib-name")
                && !help.contains("--lib-path-literal"),
            "unexpected help output: {help}"
        );
    }

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

    #[test]
    fn generate_cli_rejects_removed_lib_path_literal_flag() {
        let error = Cli::try_parse_from([
            "uniffi-bindgen-node-js",
            "generate",
            "/tmp/libfixture.dylib",
            "--out-dir",
            "/tmp/out",
            "--lib-path-literal",
            "./native/libfixture.dylib",
        ])
        .expect_err("removed --lib-path-literal flag should not parse");

        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
        assert!(
            error.to_string().contains("--lib-path-literal"),
            "unexpected clap error: {error}"
        );
    }
}
