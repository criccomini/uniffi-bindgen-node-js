#![allow(dead_code)]

use std::{
    env, fs,
    io::{self, Write},
    process::{self, Command},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use camino::Utf8PathBuf;
use clap::ValueEnum;
use serde_json::Value;
use uniffi_bindgen_node_js::{GenerateNodePackageOptions, generate_node_package};

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum FixtureKind {
    Basic,
    Callbacks,
}

#[derive(Clone, Copy, Debug)]
pub struct FixtureSpec {
    pub dir_name: &'static str,
    pub namespace: &'static str,
    pub crate_name: &'static str,
}

impl FixtureSpec {
    pub fn package_name(self) -> String {
        format!("{}-package", self.namespace)
    }
}

#[derive(Debug)]
pub struct BuiltFixtureCdylib {
    pub workspace_dir: Utf8PathBuf,
    pub manifest_path: Utf8PathBuf,
    pub namespace: String,
    pub crate_name: String,
    pub package_name: String,
    pub library_path: Utf8PathBuf,
}

#[derive(Debug)]
pub struct GeneratedFixturePackage {
    pub built_fixture: BuiltFixtureCdylib,
    pub package_dir: Utf8PathBuf,
    pub staged_library_path: Utf8PathBuf,
}

pub fn repo_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn enter_repo_root() -> Result<()> {
    env::set_current_dir(repo_root().as_std_path())
        .context("failed to enter repository root for leak helpers")
}

pub fn fixture_spec(kind: FixtureKind) -> FixtureSpec {
    match kind {
        FixtureKind::Basic => FixtureSpec {
            dir_name: "basic-fixture",
            namespace: "fixture",
            crate_name: "fixture_basic",
        },
        FixtureKind::Callbacks => FixtureSpec {
            dir_name: "callback-fixture",
            namespace: "callbacks_fixture",
            crate_name: "fixture_callbacks",
        },
    }
}

pub fn build_fixture_cdylib(kind: FixtureKind) -> Result<BuiltFixtureCdylib> {
    let spec = fixture_spec(kind);
    let workspace_dir = temp_dir_path(&format!("leaks-{}-cdylib", spec.dir_name));
    let fixture_dir = workspace_dir.join(spec.dir_name);
    let manifest_path = fixture_dir.join("Cargo.toml");
    let target_dir = workspace_dir.join("target");
    let source_dir = repo_root().join("fixtures").join(spec.dir_name);

    copy_dir_all(&source_dir, &fixture_dir)?;

    run_cargo_command(
        spec.dir_name,
        "generate-lockfile",
        &manifest_path,
        &target_dir,
        &["generate-lockfile", "--offline"],
    )?;

    let output = run_cargo_command(
        spec.dir_name,
        "build",
        &manifest_path,
        &target_dir,
        &[
            "build",
            "--offline",
            "--locked",
            "--message-format=json-render-diagnostics",
        ],
    )?;

    let library_path = find_cdylib_artifact(&output.stdout, spec.crate_name).ok_or_else(|| {
        anyhow::anyhow!(
            "failed to locate cdylib artifact for fixture {}\nstdout:\n{}\nstderr:\n{}",
            spec.dir_name,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        )
    })?;

    Ok(BuiltFixtureCdylib {
        workspace_dir,
        manifest_path,
        namespace: spec.namespace.to_string(),
        crate_name: spec.crate_name.to_string(),
        package_name: spec.package_name(),
        library_path,
    })
}

pub fn generate_fixture_package(
    kind: FixtureKind,
    out_dir: Utf8PathBuf,
    manual_load: bool,
    install_npm: bool,
) -> Result<GeneratedFixturePackage> {
    ensure_output_dir_is_empty(&out_dir)?;

    let built_fixture = build_fixture_cdylib(kind)?;
    let staged_library_path =
        populate_fixture_package(kind, &out_dir, &built_fixture, manual_load)?;
    maybe_install_fixture_dependencies(&out_dir, install_npm)?;

    Ok(GeneratedFixturePackage {
        built_fixture,
        package_dir: out_dir,
        staged_library_path,
    })
}

fn populate_fixture_package(
    kind: FixtureKind,
    out_dir: &Utf8PathBuf,
    built_fixture: &BuiltFixtureCdylib,
    manual_load: bool,
) -> Result<Utf8PathBuf> {
    generate_fixture_bindings(kind, out_dir, built_fixture, manual_load)?;
    stage_fixture_cdylib(out_dir, built_fixture)
}

fn generate_fixture_bindings(
    kind: FixtureKind,
    out_dir: &Utf8PathBuf,
    built_fixture: &BuiltFixtureCdylib,
    manual_load: bool,
) -> Result<()> {
    let spec = fixture_spec(kind);
    generate_node_package(fixture_generate_options(
        out_dir,
        built_fixture,
        manual_load,
    ))
    .with_context(|| format!("failed to generate fixture package for {}", spec.dir_name))
}

fn fixture_generate_options(
    out_dir: &Utf8PathBuf,
    built_fixture: &BuiltFixtureCdylib,
    manual_load: bool,
) -> GenerateNodePackageOptions {
    GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: out_dir.clone(),
        package_name: Some(built_fixture.package_name.clone()),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load,
    }
}

fn stage_fixture_cdylib(
    out_dir: &Utf8PathBuf,
    built_fixture: &BuiltFixtureCdylib,
) -> Result<Utf8PathBuf> {
    let staged_library_package_relative_path =
        read_staged_library_package_relative_path(out_dir, &built_fixture.namespace)?;
    let staged_library_path = out_dir.join(&staged_library_package_relative_path);
    copy_fixture_cdylib_into_package(&built_fixture.library_path, &staged_library_path)?;
    Ok(staged_library_path)
}

fn maybe_install_fixture_dependencies(out_dir: &Utf8PathBuf, install_npm: bool) -> Result<()> {
    if install_npm {
        npm_install(out_dir)?;
    }
    Ok(())
}

pub fn remove_dir_all(path: &Utf8PathBuf) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path.as_std_path())
            .with_context(|| format!("failed to remove directory {path}"))?;
    }
    Ok(())
}

fn copy_fixture_cdylib_into_package(
    source_path: &Utf8PathBuf,
    target_path: &Utf8PathBuf,
) -> Result<()> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent.as_std_path())
            .with_context(|| format!("failed to create native library directory {parent}"))?;
    }

    fs::copy(source_path.as_std_path(), target_path.as_std_path()).with_context(|| {
        format!("failed to copy fixture cdylib {source_path} into {target_path}")
    })?;

    Ok(())
}

pub fn pause_for_enter(label: &str) -> Result<()> {
    let mut stdout = io::stdout().lock();
    writeln!(
        stdout,
        "[leaks] {label}\n[leaks] pid={}\n[leaks] Press Enter to continue.",
        process::id()
    )
    .context("failed to write pause prompt")?;
    stdout.flush().context("failed to flush pause prompt")?;

    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .context("failed to read confirmation from stdin")?;
    Ok(())
}

pub fn temp_dir_path(name: &str) -> Utf8PathBuf {
    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    Utf8PathBuf::from_path_buf(env::temp_dir().join(format!(
        "uniffi-bindgen-node-js-{name}-{}-{unique}-{counter}",
        process::id()
    )))
    .expect("temp dir path should be utf-8")
}

fn ensure_output_dir_is_empty(out_dir: &Utf8PathBuf) -> Result<()> {
    if out_dir.exists() {
        if !out_dir.is_dir() {
            bail!(
                "output directory '{}' exists but is not a directory",
                out_dir
            );
        }
        if fs::read_dir(out_dir.as_std_path())?
            .next()
            .transpose()?
            .is_some()
        {
            bail!(
                "output directory '{}' already exists and is not empty",
                out_dir
            );
        }
        return Ok(());
    }

    fs::create_dir_all(out_dir.as_std_path())
        .with_context(|| format!("failed to create output directory {out_dir}"))?;
    Ok(())
}

fn copy_dir_all(src: &Utf8PathBuf, dst: &Utf8PathBuf) -> Result<()> {
    fs::create_dir_all(dst.as_std_path())
        .with_context(|| format!("failed to create directory {dst}"))?;

    for entry in read_dir_entries(src)? {
        let entry = entry.with_context(|| format!("failed to read entry in {src}"))?;
        copy_dir_entry(dst, entry)?;
    }

    Ok(())
}

fn read_dir_entries(src: &Utf8PathBuf) -> Result<fs::ReadDir> {
    fs::read_dir(src.as_std_path()).with_context(|| format!("failed to read directory {src}"))
}

fn copy_dir_entry(dst: &Utf8PathBuf, entry: fs::DirEntry) -> Result<()> {
    let entry_path = utf8_entry_path(entry.path());
    let target_path = dst.join(entry.file_name().to_string_lossy().as_ref());

    if entry_path.is_dir() {
        copy_dir_all(&entry_path, &target_path)
    } else {
        copy_file(&entry_path, &target_path)
    }
}

fn utf8_entry_path(path: std::path::PathBuf) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(path)
        .unwrap_or_else(|path| panic!("fixture path should be utf-8: {}", path.display()))
}

fn copy_file(source_path: &Utf8PathBuf, target_path: &Utf8PathBuf) -> Result<()> {
    fs::copy(source_path.as_std_path(), target_path.as_std_path())
        .with_context(|| format!("failed to copy fixture file {source_path} to {target_path}"))?;
    Ok(())
}

fn npm_install(package_dir: &Utf8PathBuf) -> Result<()> {
    let output = Command::new(npm_command())
        .args(["install", "--no-package-lock"])
        .current_dir(package_dir.as_std_path())
        .output()
        .with_context(|| format!("failed to run npm install in {package_dir}"))?;

    if !output.status.success() {
        bail!(
            "failed to run npm install in {package_dir}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    Ok(())
}

fn npm_command() -> &'static str {
    if cfg!(windows) { "npm.cmd" } else { "npm" }
}

fn run_cargo_command(
    fixture_name: &str,
    operation: &str,
    manifest_path: &Utf8PathBuf,
    target_dir: &Utf8PathBuf,
    args: &[&str],
) -> Result<process::Output> {
    let output = cargo_command_output(manifest_path, target_dir, args)
        .with_context(|| format!("failed to run cargo {operation} for fixture {fixture_name}"))?;

    if output.status.success() {
        return Ok(output);
    }

    if should_retry_cargo_without_offline(args, &output.stderr) {
        let retry_args = strip_offline_flag(args);
        let retry_output = cargo_command_output(manifest_path, target_dir, &retry_args)
            .with_context(|| {
                format!(
                    "failed to rerun cargo {operation} for fixture {fixture_name} without --offline"
                )
            })?;
        if retry_output.status.success() {
            return Ok(retry_output);
        }

        bail!(
            "failed to run cargo {operation} for fixture {fixture_name}\n\
offline stdout:\n{}\n\
offline stderr:\n{}\n\
online retry stdout:\n{}\n\
online retry stderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&retry_output.stdout),
            String::from_utf8_lossy(&retry_output.stderr),
        );
    }

    bail!(
        "failed to run cargo {operation} for fixture {fixture_name}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}

fn cargo_command_output(
    manifest_path: &Utf8PathBuf,
    target_dir: &Utf8PathBuf,
    args: &[&str],
) -> io::Result<process::Output> {
    Command::new(env!("CARGO"))
        .args(args)
        .arg("--manifest-path")
        .arg(manifest_path.as_str())
        .env("CARGO_TARGET_DIR", target_dir.as_str())
        .output()
}

fn should_retry_cargo_without_offline(args: &[&str], stderr: &[u8]) -> bool {
    args.contains(&"--offline") && {
        let stderr = String::from_utf8_lossy(stderr);
        stderr.contains("you're using offline mode (--offline)")
            || stderr.contains("attempting to make an HTTP request, but --offline was specified")
    }
}

fn strip_offline_flag<'a>(args: &'a [&'a str]) -> Vec<&'a str> {
    args.iter()
        .copied()
        .filter(|arg| *arg != "--offline")
        .collect()
}

fn find_cdylib_artifact(stdout: &[u8], crate_name: &str) -> Option<Utf8PathBuf> {
    let extension = std::env::consts::DLL_EXTENSION;

    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(parse_cargo_build_message)
        .find_map(|message| artifact_path_from_message(&message, crate_name, extension))
}

fn parse_cargo_build_message(line: &str) -> Option<Value> {
    serde_json::from_str::<Value>(line).ok()
}

fn artifact_path_from_message(
    message: &Value,
    crate_name: &str,
    extension: &str,
) -> Option<Utf8PathBuf> {
    let target = compiler_artifact_target(message)?;
    if !target_matches_cdylib(target, crate_name) {
        return None;
    }

    artifact_filename(message, extension)
}

fn compiler_artifact_target(message: &Value) -> Option<&Value> {
    if message.get("reason")?.as_str()? != "compiler-artifact" {
        return None;
    }

    message.get("target")
}

fn target_matches_cdylib(target: &Value, crate_name: &str) -> bool {
    target_name(target) == Some(crate_name) && target_has_cdylib_type(target)
}

fn target_name(target: &Value) -> Option<&str> {
    target.get("name")?.as_str()
}

fn target_has_cdylib_type(target: &Value) -> bool {
    target
        .get("crate_types")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .any(|crate_type| crate_type == "cdylib")
}

fn artifact_filename(message: &Value, extension: &str) -> Option<Utf8PathBuf> {
    message
        .get("filenames")?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find(|filename| filename.ends_with(extension))
        .and_then(|filename| Utf8PathBuf::from_path_buf(filename.into()).ok())
}

fn read_staged_library_package_relative_path(
    package_dir: &Utf8PathBuf,
    namespace: &str,
) -> Result<Utf8PathBuf> {
    let ffi_js_path = package_dir.join(format!("{namespace}-ffi.js"));
    let ffi_js = fs::read_to_string(ffi_js_path.as_std_path())
        .with_context(|| format!("failed to read generated ffi file {ffi_js_path}"))?;
    let raw_relative_path = ffi_js
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("stagedLibraryPackageRelativePath: ")
                .map(|value| value.trim_end_matches(','))
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "generated ffi file '{}' does not include stagedLibraryPackageRelativePath metadata",
                ffi_js_path
            )
        })?;
    let relative_path: String = serde_json::from_str(raw_relative_path).with_context(|| {
        format!(
            "generated ffi metadata in '{}' should serialize stagedLibraryPackageRelativePath as JSON",
            ffi_js_path
        )
    })?;

    Ok(Utf8PathBuf::from(relative_path))
}
