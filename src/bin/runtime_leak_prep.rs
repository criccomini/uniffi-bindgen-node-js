#[path = "support/mod.rs"]
mod support;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;

use self::support::{FixtureKind, enter_repo_root, generate_fixture_package};

#[derive(Debug, Parser)]
struct Args {
    #[arg(value_enum)]
    fixture: FixtureKind,

    #[arg(long)]
    out_dir: Utf8PathBuf,

    #[arg(long)]
    manual_load: bool,

    #[arg(long)]
    skip_npm_install: bool,
}

fn main() -> Result<()> {
    enter_repo_root()?;

    let args = Args::parse();
    let generated = generate_fixture_package(
        args.fixture,
        args.out_dir,
        args.manual_load,
        !args.skip_npm_install,
    )?;

    println!("fixture: {}", generated.built_fixture.crate_name);
    println!("package_dir: {}", generated.package_dir);
    println!("library_path: {}", generated.sibling_library_path);
    println!("workspace_dir: {}", generated.built_fixture.workspace_dir);

    Ok(())
}
