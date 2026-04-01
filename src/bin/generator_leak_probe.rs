#[path = "support/mod.rs"]
mod support;

use std::fs;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use uniffi_bindgen_node_js::subcommands::generate::{self, GenerateArgs};

use self::support::{
    FixtureKind, build_fixture_cdylib, enter_repo_root, fixture_spec, pause_for_enter,
    remove_dir_all, temp_dir_path,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum FixtureSelection {
    Basic,
    Callbacks,
    Both,
}

#[derive(Debug, Parser)]
struct Args {
    #[arg(value_enum, default_value = "both")]
    fixture: FixtureSelection,

    #[arg(long, default_value_t = 200)]
    iterations: usize,

    #[arg(long, default_value_t = 10)]
    warmup_iterations: usize,

    #[arg(long)]
    pause_after_warmup: bool,

    #[arg(long)]
    pause_at_end: bool,
}

fn main() -> Result<()> {
    enter_repo_root()?;

    let args = Args::parse();
    let selected_fixtures = selected_fixture_kinds(args.fixture);
    let built_fixtures = selected_fixtures
        .into_iter()
        .map(build_fixture_cdylib)
        .collect::<Result<Vec<_>>>()?;
    let output_root = temp_dir_path("generator-leak-probe-output");

    fs::create_dir_all(output_root.as_std_path())
        .with_context(|| format!("failed to create output root {output_root}"))?;

    println!("[leaks] generator leak probe pid={}", std::process::id());
    println!("[leaks] output_root={output_root}");
    println!(
        "[leaks] run `leaks {}` while the process is paused or between progress updates.",
        std::process::id()
    );

    for iteration in 0..args.iterations {
        for built_fixture in &built_fixtures {
            let out_dir = output_root.join(format!("{}-{}", built_fixture.crate_name, iteration));
            let spec = fixture_spec(fixture_kind_for_crate(&built_fixture.crate_name));

            generate::run(GenerateArgs {
                lib_source: built_fixture.library_path.clone(),
                manifest_path: Some(built_fixture.manifest_path.clone()),
                crate_name: Some(built_fixture.crate_name.clone()),
                out_dir: out_dir.clone(),
                package_name: Some(spec.package_name()),
                cdylib_name: Some(built_fixture.crate_name.clone()),
                node_engine: None,
                lib_path_literal: None,
                bundled_prebuilds: false,
                manual_load: false,
                config_override: Vec::new(),
            })
            .with_context(|| {
                format!(
                    "failed to generate bindings for crate '{}' on iteration {}",
                    built_fixture.crate_name,
                    iteration + 1
                )
            })?;

            remove_dir_all(&out_dir)?;
        }

        let completed = iteration + 1;
        if completed == args.warmup_iterations && args.pause_after_warmup {
            pause_for_enter(
                "Warmup complete. Inspect the live generator process with leaks, then press Enter.",
            )?;
        }
        if completed % 10 == 0 || completed == args.iterations {
            println!(
                "[leaks] completed {completed}/{} iterations",
                args.iterations
            );
        }
    }

    if args.pause_at_end {
        pause_for_enter(
            "Measured iterations complete. Inspect the live generator process with leaks, then press Enter to exit.",
        )?;
    }

    remove_dir_all(&output_root)?;
    for built_fixture in &built_fixtures {
        remove_dir_all(&built_fixture.workspace_dir)?;
    }

    Ok(())
}

fn selected_fixture_kinds(selection: FixtureSelection) -> Vec<FixtureKind> {
    match selection {
        FixtureSelection::Basic => vec![FixtureKind::Basic],
        FixtureSelection::Callbacks => vec![FixtureKind::Callbacks],
        FixtureSelection::Both => vec![FixtureKind::Basic, FixtureKind::Callbacks],
    }
}

fn fixture_kind_for_crate(crate_name: &str) -> FixtureKind {
    match crate_name {
        "fixture_basic" => FixtureKind::Basic,
        "fixture_callbacks" => FixtureKind::Callbacks,
        other => panic!("unknown fixture crate '{other}'"),
    }
}
