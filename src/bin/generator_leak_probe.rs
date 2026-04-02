#[path = "support/mod.rs"]
mod support;

use std::fs;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::{Parser, ValueEnum};
use uniffi_bindgen_node_js::{GenerateNodePackageOptions, generate_node_package};

use self::support::{
    BuiltFixtureCdylib, FixtureKind, build_fixture_cdylib, enter_repo_root, pause_for_enter,
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
    let built_fixtures = build_selected_fixtures(args.fixture)?;
    let output_root = create_output_root()?;

    print_probe_instructions(&output_root);
    run_probe_iterations(&args, &built_fixtures, &output_root)?;
    pause_at_end_if_requested(&args)?;
    cleanup_probe_artifacts(&output_root, &built_fixtures)
}

fn build_selected_fixtures(selection: FixtureSelection) -> Result<Vec<BuiltFixtureCdylib>> {
    selected_fixture_kinds(selection)
        .into_iter()
        .map(build_fixture_cdylib)
        .collect()
}

fn create_output_root() -> Result<Utf8PathBuf> {
    let output_root = temp_dir_path("generator-leak-probe-output");
    fs::create_dir_all(output_root.as_std_path())
        .with_context(|| format!("failed to create output root {output_root}"))?;
    Ok(output_root)
}

fn print_probe_instructions(output_root: &Utf8Path) {
    println!("[leaks] generator leak probe pid={}", std::process::id());
    println!("[leaks] output_root={output_root}");
    println!(
        "[leaks] run `leaks {}` while the process is paused or between progress updates.",
        std::process::id()
    );
}

fn run_probe_iterations(
    args: &Args,
    built_fixtures: &[BuiltFixtureCdylib],
    output_root: &Utf8Path,
) -> Result<()> {
    for iteration in 0..args.iterations {
        run_probe_iteration(iteration, built_fixtures, output_root)?;
        handle_iteration_milestones(args, iteration + 1)?;
    }

    Ok(())
}

fn run_probe_iteration(
    iteration: usize,
    built_fixtures: &[BuiltFixtureCdylib],
    output_root: &Utf8Path,
) -> Result<()> {
    for built_fixture in built_fixtures {
        generate_fixture_iteration(built_fixture, output_root, iteration)?;
    }

    Ok(())
}

fn generate_fixture_iteration(
    built_fixture: &BuiltFixtureCdylib,
    output_root: &Utf8Path,
    iteration: usize,
) -> Result<()> {
    let out_dir = output_root.join(format!("{}-{}", built_fixture.crate_name, iteration));
    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: out_dir.clone(),
        package_name: Some(built_fixture.package_name.clone()),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .with_context(|| {
        format!(
            "failed to generate bindings for crate '{}' on iteration {}",
            built_fixture.crate_name,
            iteration + 1
        )
    })?;

    remove_dir_all(&out_dir)
}

fn handle_iteration_milestones(args: &Args, completed: usize) -> Result<()> {
    if completed == args.warmup_iterations && args.pause_after_warmup {
        pause_for_enter(
            "Warmup complete. Inspect the live generator process with leaks, then press Enter.",
        )?;
    }

    if should_report_progress(completed, args.iterations) {
        println!(
            "[leaks] completed {completed}/{} iterations",
            args.iterations
        );
    }

    Ok(())
}

fn should_report_progress(completed: usize, total_iterations: usize) -> bool {
    completed % 10 == 0 || completed == total_iterations
}

fn pause_at_end_if_requested(args: &Args) -> Result<()> {
    if !args.pause_at_end {
        return Ok(());
    }

    pause_for_enter(
        "Measured iterations complete. Inspect the live generator process with leaks, then press Enter to exit.",
    )
}

fn cleanup_probe_artifacts(
    output_root: &Utf8PathBuf,
    built_fixtures: &[BuiltFixtureCdylib],
) -> Result<()> {
    remove_dir_all(output_root)?;
    for built_fixture in built_fixtures {
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
