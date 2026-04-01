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
use uniffi_bindgen_node_js::subcommands::generate::{self, GenerateArgs};

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
    pub namespace: String,
    pub crate_name: String,
    pub library_path: Utf8PathBuf,
}

#[derive(Debug)]
pub struct GeneratedFixturePackage {
    pub built_fixture: BuiltFixtureCdylib,
    pub package_dir: Utf8PathBuf,
    pub sibling_library_path: Utf8PathBuf,
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

    let output = Command::new(env!("CARGO"))
        .args([
            "build",
            "--offline",
            "--locked",
            "--manifest-path",
            manifest_path.as_str(),
            "--message-format=json-render-diagnostics",
        ])
        .env("CARGO_TARGET_DIR", target_dir.as_str())
        .output()
        .with_context(|| format!("failed to run cargo build for fixture {}", spec.dir_name))?;

    if !output.status.success() {
        bail!(
            "failed to build fixture {}\nstdout:\n{}\nstderr:\n{}",
            spec.dir_name,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

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
        namespace: spec.namespace.to_string(),
        crate_name: spec.crate_name.to_string(),
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
    let spec = fixture_spec(kind);

    generate::run(GenerateArgs {
        lib_source: built_fixture.library_path.clone(),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: out_dir.clone(),
        package_name: Some(spec.package_name()),
        cdylib_name: Some(built_fixture.crate_name.clone()),
        node_engine: None,
        lib_path_literal: None,
        bundled_prebuilds: false,
        manual_load,
        config_override: Vec::new(),
    })
    .with_context(|| format!("failed to generate fixture package for {}", spec.dir_name))?;

    let library_filename = built_fixture
        .library_path
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("fixture library path has no filename"))?;
    let sibling_library_path = out_dir.join(library_filename);
    copy_library(
        &built_fixture.library_path,
        &sibling_library_path,
        "leak probe",
    )?;

    if install_npm {
        npm_install(&out_dir)?;
    }

    Ok(GeneratedFixturePackage {
        built_fixture,
        package_dir: out_dir,
        sibling_library_path,
    })
}

pub fn remove_dir_all(path: &Utf8PathBuf) -> Result<()> {
    if path.exists() {
        fs::remove_dir_all(path.as_std_path())
            .with_context(|| format!("failed to remove directory {path}"))?;
    }
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

    for entry in fs::read_dir(src.as_std_path())
        .with_context(|| format!("failed to read directory {src}"))?
    {
        let entry = entry.with_context(|| format!("failed to read entry in {src}"))?;
        let entry_path = Utf8PathBuf::from_path_buf(entry.path())
            .unwrap_or_else(|path| panic!("fixture path should be utf-8: {}", path.display()));
        let target_path = dst.join(entry.file_name().to_string_lossy().as_ref());

        if entry_path.is_dir() {
            copy_dir_all(&entry_path, &target_path)?;
        } else {
            fs::copy(entry_path.as_std_path(), target_path.as_std_path()).with_context(|| {
                format!("failed to copy fixture file {entry_path} to {target_path}")
            })?;
        }
    }

    Ok(())
}

fn copy_library(
    source_path: &Utf8PathBuf,
    destination_path: &Utf8PathBuf,
    context: &str,
) -> Result<()> {
    if let Some(parent) = destination_path.parent() {
        fs::create_dir_all(parent.as_std_path())
            .with_context(|| format!("failed to create {context} directory {parent}"))?;
    }

    fs::copy(source_path.as_std_path(), destination_path.as_std_path()).with_context(|| {
        format!("failed to copy {context} library {source_path} to {destination_path}")
    })?;
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
) -> Result<()> {
    let output = Command::new(env!("CARGO"))
        .args(args)
        .arg("--manifest-path")
        .arg(manifest_path.as_str())
        .env("CARGO_TARGET_DIR", target_dir.as_str())
        .output()
        .with_context(|| format!("failed to run cargo {operation} for fixture {fixture_name}"))?;

    if !output.status.success() {
        bail!(
            "failed to run cargo {operation} for fixture {fixture_name}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }

    Ok(())
}

fn find_cdylib_artifact(stdout: &[u8], crate_name: &str) -> Option<Utf8PathBuf> {
    let extension = std::env::consts::DLL_EXTENSION;

    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find_map(|message| {
            let reason = message.get("reason")?.as_str()?;
            if reason != "compiler-artifact" {
                return None;
            }

            let target = message.get("target")?;
            let target_name = target.get("name")?.as_str()?;
            if target_name != crate_name {
                return None;
            }

            let crate_types = target.get("crate_types")?.as_array()?;
            if !crate_types
                .iter()
                .filter_map(Value::as_str)
                .any(|crate_type| crate_type == "cdylib")
            {
                return None;
            }

            message
                .get("filenames")?
                .as_array()?
                .iter()
                .filter_map(Value::as_str)
                .find(|filename| filename.ends_with(extension))
                .and_then(|filename| Utf8PathBuf::from_path_buf(filename.into()).ok())
        })
}
