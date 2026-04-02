#![allow(dead_code)]

pub mod fixtures;

use std::{
    collections::BTreeMap,
    env, fs, process,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use camino::Utf8PathBuf;
use serde_json::Value;
use uniffi_bindgen::{BindgenLoader, BindgenPaths, ComponentInterface};
use uniffi_bindgen_node_js::{GenerateNodePackageOptions, generate_node_package};

use self::fixtures::fixture_spec;

pub struct BuiltFixtureCdylib {
    pub workspace_dir: Utf8PathBuf,
    pub manifest_path: Utf8PathBuf,
    pub namespace: String,
    pub crate_name: String,
    pub library_path: Utf8PathBuf,
}

pub struct BuiltMultiComponentFixtureCdylib {
    pub workspace_dir: Utf8PathBuf,
    pub manifest_path: Utf8PathBuf,
    pub library_path: Utf8PathBuf,
    pub available_crate_names: Vec<String>,
}

pub struct GeneratedFixturePackage {
    pub built_fixture: BuiltFixtureCdylib,
    pub package_dir: Utf8PathBuf,
    pub staged_library_package_relative_path: Utf8PathBuf,
    pub sibling_library_path: Option<Utf8PathBuf>,
    pub bundled_prebuild_target: Option<String>,
    pub bundled_prebuild_path: Option<Utf8PathBuf>,
}

#[derive(Clone, Copy, Debug)]
pub struct FixturePackageOptions {
    pub bundled_prebuilds: bool,
    pub manual_load: bool,
}

impl Default for FixturePackageOptions {
    fn default() -> Self {
        Self {
            bundled_prebuilds: false,
            manual_load: false,
        }
    }
}

pub fn build_fixture_cdylib(name: &str) -> BuiltFixtureCdylib {
    let spec = fixture_spec(name);
    let workspace_dir = temp_dir_path(&format!("fixture-{name}-cdylib"));
    let fixture_dir = workspace_dir.join(spec.dir_name);
    let manifest_path = fixture_dir.join("Cargo.toml");
    let target_dir = workspace_dir.join("target");
    let source_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(spec.dir_name);

    copy_dir_all(&source_dir, &fixture_dir);

    run_cargo_command(
        name,
        "generate-lockfile",
        &manifest_path,
        &target_dir,
        &["generate-lockfile", "--offline"],
    );

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
        .unwrap_or_else(|error| panic!("failed to run cargo build for fixture {name}: {error}"));

    if !output.status.success() {
        panic!(
            "failed to build fixture {name}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let library_path = find_cdylib_artifact(&output.stdout, spec.crate_name).unwrap_or_else(|| {
        panic!(
            "failed to locate cdylib artifact for fixture {name}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });

    BuiltFixtureCdylib {
        workspace_dir,
        manifest_path,
        namespace: spec.namespace.to_string(),
        crate_name: spec.crate_name.to_string(),
        library_path,
    }
}

pub fn build_proc_macro_multi_component_cdylib() -> BuiltMultiComponentFixtureCdylib {
    let fixture_name = "proc-macro-multi-component";
    let workspace_dir = temp_dir_path(fixture_name);
    let manifest_path = workspace_dir.join("megazord").join("Cargo.toml");
    let target_dir = workspace_dir.join("target");

    for relative_dir in [
        "component-alpha/src",
        "component-beta/src",
        "megazord/src",
    ] {
        let path = workspace_dir.join(relative_dir);
        fs::create_dir_all(path.as_std_path())
            .unwrap_or_else(|error| panic!("failed to create temp fixture dir {path}: {error}"));
    }

    write_temp_fixture_file(
        &workspace_dir.join("Cargo.toml"),
        r#"
[workspace]
members = ["component-alpha", "component-beta", "megazord"]
resolver = "2"
"#,
    );
    write_temp_fixture_file(
        &workspace_dir.join("component-alpha").join("Cargo.toml"),
        r#"
[package]
name = "component-alpha"
version = "0.1.0"
edition = "2021"
publish = false

[lib]
name = "component_alpha"

[dependencies]
uniffi = { version = "=0.31.0" }
"#,
    );
    write_temp_fixture_file(
        &workspace_dir.join("component-alpha").join("src").join("lib.rs"),
        r#"
#[uniffi::export]
pub fn alpha_value() -> u32 {
    1
}

uniffi::setup_scaffolding!();
"#,
    );
    write_temp_fixture_file(
        &workspace_dir.join("component-beta").join("Cargo.toml"),
        r#"
[package]
name = "component-beta"
version = "0.1.0"
edition = "2021"
publish = false

[lib]
name = "component_beta"

[dependencies]
uniffi = { version = "=0.31.0" }
"#,
    );
    write_temp_fixture_file(
        &workspace_dir.join("component-beta").join("src").join("lib.rs"),
        r#"
#[uniffi::export]
pub fn beta_value() -> u32 {
    2
}

uniffi::setup_scaffolding!();
"#,
    );
    write_temp_fixture_file(
        &workspace_dir.join("megazord").join("Cargo.toml"),
        r#"
[package]
name = "megazord-fixture"
version = "0.1.0"
edition = "2021"
publish = false

[lib]
name = "megazord_fixture"
crate-type = ["cdylib"]

[dependencies]
component-alpha = { path = "../component-alpha" }
component-beta = { path = "../component-beta" }
"#,
    );
    write_temp_fixture_file(
        &workspace_dir.join("megazord").join("src").join("lib.rs"),
        r#"
component_alpha::uniffi_reexport_scaffolding!();
component_beta::uniffi_reexport_scaffolding!();

#[unsafe(no_mangle)]
pub extern "C" fn megazord_fixture_ping() -> u32 {
    component_alpha::alpha_value() + component_beta::beta_value()
}
"#,
    );

    run_cargo_command(
        fixture_name,
        "generate-lockfile",
        &manifest_path,
        &target_dir,
        &["generate-lockfile", "--offline"],
    );

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
        .unwrap_or_else(|error| {
            panic!("failed to run cargo build for fixture {fixture_name}: {error}")
        });

    if !output.status.success() {
        panic!(
            "failed to build fixture {fixture_name}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let library_path =
        find_cdylib_artifact(&output.stdout, "megazord_fixture").unwrap_or_else(|| {
            panic!(
                "failed to locate cdylib artifact for fixture {fixture_name}\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });

    BuiltMultiComponentFixtureCdylib {
        workspace_dir,
        manifest_path,
        library_path,
        available_crate_names: vec!["component_alpha".to_string(), "component_beta".to_string()],
    }
}

pub fn build_off_workspace_udl_fixture_cdylib() -> BuiltFixtureCdylib {
    let fixture_name = "off-workspace-udl-fixture";
    let workspace_dir = temp_dir_path(fixture_name);
    let fixture_dir = workspace_dir.join("fixture");
    let manifest_path = fixture_dir.join("Cargo.toml");
    let target_dir = workspace_dir.join("target");
    let namespace = "temp_udl_missing_context";
    let crate_name = "temp_udl_missing_context_fixture";

    fs::create_dir_all(fixture_dir.join("src").as_std_path())
        .unwrap_or_else(|error| panic!("failed to create temp fixture dir {fixture_dir}: {error}"));

    write_temp_fixture_file(
        &manifest_path,
        &format!(
            r#"
[package]
name = "temp-udl-missing-context-fixture"
version = "0.1.0"
edition = "2021"
publish = false

[lib]
name = "{crate_name}"
crate-type = ["cdylib"]

[dependencies]
uniffi = {{ version = "=0.31.0" }}

[build-dependencies]
uniffi = {{ version = "=0.31.0", features = ["build"] }}
"#
        ),
    );
    write_temp_fixture_file(
        &fixture_dir.join("build.rs"),
        &format!(
            r#"
fn main() {{
    uniffi::generate_scaffolding("src/{namespace}.udl").expect("UDL scaffolding should generate");
}}
"#
        ),
    );
    write_temp_fixture_file(
        &fixture_dir.join("src").join("lib.rs"),
        &format!(
            r#"
pub fn meaning_of_life() -> u32 {{
    42
}}

uniffi::include_scaffolding!("{namespace}");
"#
        ),
    );
    write_temp_fixture_file(
        &fixture_dir.join("src").join(format!("{namespace}.udl")),
        &format!(
            r#"
namespace {namespace} {{
    u32 meaning_of_life();
}};
"#
        ),
    );

    run_cargo_command(
        fixture_name,
        "generate-lockfile",
        &manifest_path,
        &target_dir,
        &["generate-lockfile", "--offline"],
    );

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
        .unwrap_or_else(|error| {
            panic!("failed to run cargo build for fixture {fixture_name}: {error}")
        });

    if !output.status.success() {
        panic!(
            "failed to build fixture {fixture_name}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let library_path = find_cdylib_artifact(&output.stdout, crate_name).unwrap_or_else(|| {
        panic!(
            "failed to locate cdylib artifact for fixture {fixture_name}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    });

    BuiltFixtureCdylib {
        workspace_dir,
        manifest_path,
        namespace: namespace.to_string(),
        crate_name: crate_name.to_string(),
        library_path,
    }
}

pub fn generate_fixture_package(name: &str) -> GeneratedFixturePackage {
    generate_fixture_package_with_options(name, FixturePackageOptions::default())
}

pub fn generate_fixture_package_with_options(
    name: &str,
    options: FixturePackageOptions,
) -> GeneratedFixturePackage {
    let built_fixture = build_fixture_cdylib(name);
    let package_dir = temp_dir_path(&format!("fixture-{name}-package"));

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: Some(format!("{}-package", built_fixture.namespace)),
        node_engine: None,
        bundled_prebuilds: options.bundled_prebuilds,
        manual_load: options.manual_load,
    })
    .unwrap_or_else(|error| panic!("failed to generate fixture package {name}: {error:#}"));

    let library_filename = built_fixture.library_path.file_name().unwrap_or_else(|| {
        panic!(
            "fixture library path has no filename: {}",
            built_fixture.library_path
        )
    });
    let staged_library_package_relative_path =
        read_staged_library_package_relative_path(&package_dir, &built_fixture.namespace);
    let staged_library_path = package_dir.join(&staged_library_package_relative_path);
    let staged_library_components = staged_library_package_relative_path
        .components()
        .map(|component| component.as_str())
        .collect::<Vec<_>>();
    let (sibling_library_path, bundled_prebuild_target, bundled_prebuild_path) =
        match staged_library_components.as_slice() {
            [file_name] => {
                assert_eq!(
                    *file_name, library_filename,
                    "generated root-staged library should keep the input filename"
                );
                (Some(staged_library_path), None, None)
            }
            ["prebuilds", target, file_name] => {
                assert_eq!(
                    *file_name, library_filename,
                    "generated bundled prebuild should keep the input filename"
                );
                (None, Some((*target).to_string()), Some(staged_library_path))
            }
            _ => panic!(
                "unexpected generated staged library path {} in {}",
                staged_library_package_relative_path, package_dir
            ),
        };

    GeneratedFixturePackage {
        built_fixture,
        package_dir,
        staged_library_package_relative_path,
        sibling_library_path,
        bundled_prebuild_target,
        bundled_prebuild_path,
    }
}

pub fn load_fixture_component_interface(fixture: &BuiltFixtureCdylib) -> ComponentInterface {
    let loader = BindgenLoader::new(BindgenPaths::default());
    let metadata = loader
        .load_metadata(&fixture.library_path)
        .unwrap_or_else(|error| {
            panic!(
                "failed to load UniFFI metadata from fixture library {}\nerror: {error:#}",
                fixture.library_path
            )
        });
    let cis = loader.load_cis(metadata).unwrap_or_else(|error| {
        panic!(
            "failed to load UniFFI component interfaces from fixture library {}\nerror: {error:#}",
            fixture.library_path
        )
    });

    cis.into_iter()
        .find(|ci| ci.crate_name() == fixture.crate_name)
        .unwrap_or_else(|| {
            panic!(
                "fixture library {} did not expose component interface for crate {}",
                fixture.library_path, fixture.crate_name
            )
        })
}

pub fn install_generated_package_dependencies(package_dir: &Utf8PathBuf) {
    rewrite_package_dependency_to_local_fixture(package_dir, "koffi", &local_koffi_fixture_dir());
    npm_install(package_dir);
}

pub fn install_generated_package_benchmark_dependencies(package_dir: &Utf8PathBuf) {
    add_package_dependency(package_dir, "tinybench", "3.0.1");
    npm_install(package_dir);
}

pub fn install_generated_package_dependencies_with_real_koffi(package_dir: &Utf8PathBuf) {
    npm_install(package_dir);
}

pub fn install_fixture_package_dependencies(package_dir: &Utf8PathBuf) {
    install_generated_package_dependencies(package_dir);
}

pub fn install_fixture_package_benchmark_dependencies(package_dir: &Utf8PathBuf) {
    install_generated_package_benchmark_dependencies(package_dir);
}

pub fn install_fixture_package_dependencies_with_real_koffi(package_dir: &Utf8PathBuf) {
    install_generated_package_dependencies_with_real_koffi(package_dir);
}

fn npm_install(package_dir: &Utf8PathBuf) {
    let output = Command::new(npm_command())
        .args(["install", "--no-package-lock"])
        .current_dir(package_dir.as_std_path())
        .output()
        .unwrap_or_else(|error| panic!("failed to run npm install in {package_dir}: {error}"));

    if !output.status.success() {
        panic!(
            "failed to run npm install in {package_dir}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

pub fn run_node_script(package_dir: &Utf8PathBuf, script_name: &str, source: &str) {
    let script_path = package_dir.join(script_name);
    fs::write(script_path.as_std_path(), source)
        .unwrap_or_else(|error| panic!("failed to write Node script {script_path}: {error}"));

    run_node_program(package_dir, script_name, &[], &[]);
}

pub fn run_node_program(
    package_dir: &Utf8PathBuf,
    script_relative_path: &str,
    node_args: &[&str],
    envs: &[(&str, &str)],
) -> String {
    let script_path = package_dir.join(script_relative_path);
    let output = Command::new("node")
        .args(node_args)
        .arg(script_path.as_str())
        .current_dir(package_dir.as_std_path())
        .envs(envs.iter().copied())
        .output()
        .unwrap_or_else(|error| panic!("failed to run Node script {script_path}: {error}"));

    if !output.status.success() {
        panic!(
            "Node script {script_path} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8_lossy(&output.stdout).into_owned()
}

pub fn stage_package_benchmark_scripts(package_dir: &Utf8PathBuf) {
    let source_dir = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("benchmarks");
    let target_dir = package_dir.join("benchmarks");
    copy_dir_all(&source_dir, &target_dir);
}

fn write_temp_fixture_file(path: &Utf8PathBuf, contents: &str) {
    fs::write(path.as_std_path(), contents.trim_start())
        .unwrap_or_else(|error| panic!("failed to write temp fixture file {path}: {error}"));
}

pub fn run_typescript_check(package_dir: &Utf8PathBuf, script_name: &str, source: &str) {
    let script_path = package_dir.join(script_name);
    fs::write(script_path.as_std_path(), source)
        .unwrap_or_else(|error| panic!("failed to write TypeScript script {script_path}: {error}"));

    let tsconfig_path = package_dir.join("tsconfig.json");
    fs::write(
        tsconfig_path.as_std_path(),
        r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "NodeNext",
    "moduleResolution": "NodeNext",
    "lib": ["ES2022", "DOM"],
    "noEmit": true,
    "strict": true,
    "skipLibCheck": false
  },
  "files": ["smoke.ts"]
}
"#,
    )
    .unwrap_or_else(|error| panic!("failed to write TypeScript config {tsconfig_path}: {error}"));

    let tsc_path = find_typescript_cli().unwrap_or_else(|| {
        panic!("failed to locate a local TypeScript compiler for generated package checks")
    });
    let output = Command::new(tsc_path.as_str())
        .arg("--project")
        .arg(tsconfig_path.as_str())
        .current_dir(package_dir.as_std_path())
        .output()
        .unwrap_or_else(|error| {
            panic!("failed to run TypeScript compiler in {package_dir}: {error}")
        });

    if !output.status.success() {
        panic!(
            "TypeScript check failed in {package_dir}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

pub fn remove_dir_all(path: &Utf8PathBuf) {
    if path.exists() {
        fs::remove_dir_all(path.as_std_path())
            .unwrap_or_else(|error| panic!("failed to remove temp dir {path}: {error}"));
    }
}

pub fn read_package_file_tree(package_dir: &Utf8PathBuf) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    collect_package_file_tree(package_dir, package_dir, &mut files);
    files
}

pub fn temp_dir_path(name: &str) -> Utf8PathBuf {
    static TEMP_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    Utf8PathBuf::from_path_buf(env::temp_dir().join(format!(
        "uniffi-bindgen-node-js-tests-{name}-{}-{unique}-{counter}",
        process::id()
    )))
    .expect("temp dir path should be utf-8")
}

fn copy_dir_all(src: &Utf8PathBuf, dst: &Utf8PathBuf) {
    fs::create_dir_all(dst.as_std_path())
        .unwrap_or_else(|error| panic!("failed to create fixture dir {dst}: {error}"));

    for entry in fs::read_dir(src.as_std_path())
        .unwrap_or_else(|error| panic!("failed to read fixture dir {src}: {error}"))
    {
        let entry =
            entry.unwrap_or_else(|error| panic!("failed to read dir entry in {src}: {error}"));
        let entry_path = Utf8PathBuf::from_path_buf(entry.path())
            .unwrap_or_else(|path| panic!("fixture path should be utf-8: {}", path.display()));
        let target_path = dst.join(entry.file_name().to_string_lossy().as_ref());

        if entry_path.is_dir() {
            copy_dir_all(&entry_path, &target_path);
        } else {
            fs::copy(entry_path.as_std_path(), target_path.as_std_path()).unwrap_or_else(|error| {
                panic!("failed to copy fixture file {entry_path} to {target_path}: {error}")
            });
        }
    }
}

fn collect_package_file_tree(
    package_dir: &Utf8PathBuf,
    current_dir: &Utf8PathBuf,
    files: &mut BTreeMap<String, Vec<u8>>,
) {
    let mut entries = fs::read_dir(current_dir.as_std_path())
        .unwrap_or_else(|error| panic!("failed to read directory {current_dir}: {error}"))
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            Utf8PathBuf::from_path_buf(entry.path())
                .unwrap_or_else(|path| panic!("package path should be utf-8: {}", path.display()))
        })
        .collect::<Vec<_>>();
    entries.sort_unstable();

    for entry_path in entries {
        if entry_path.is_dir() {
            collect_package_file_tree(package_dir, &entry_path, files);
            continue;
        }

        let relative_path = entry_path
            .strip_prefix(package_dir)
            .unwrap_or_else(|error| {
                panic!(
                    "package path {entry_path} should live under package root {package_dir}: {error}"
                )
            })
            .to_string();
        let contents = fs::read(entry_path.as_std_path())
            .unwrap_or_else(|error| panic!("failed to read generated file {entry_path}: {error}"));
        files.insert(relative_path, contents);
    }
}

fn local_koffi_fixture_dir() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("npm-fixtures")
        .join("koffi")
}

fn npm_command() -> &'static str {
    if cfg!(windows) { "npm.cmd" } else { "npm" }
}

fn find_typescript_cli() -> Option<Utf8PathBuf> {
    find_command_in_path(if cfg!(windows) {
        &["tsc.cmd", "tsc"]
    } else {
        &["tsc"]
    })
    .or_else(find_homebrew_heroku_typescript_cli)
}

fn find_command_in_path(candidates: &[&str]) -> Option<Utf8PathBuf> {
    let path = env::var_os("PATH")?;

    for directory in env::split_paths(&path) {
        for candidate in candidates {
            let candidate_path = directory.join(candidate);
            if candidate_path.is_file()
                && let Ok(candidate_path) = Utf8PathBuf::from_path_buf(candidate_path)
            {
                return Some(candidate_path);
            }
        }
    }

    None
}

fn find_homebrew_heroku_typescript_cli() -> Option<Utf8PathBuf> {
    let heroku_root = Utf8PathBuf::from("/opt/homebrew/Cellar/heroku");
    let Ok(entries) = fs::read_dir(heroku_root.as_std_path()) else {
        return None;
    };

    let mut version_dirs = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| Utf8PathBuf::from_path_buf(entry.path()).ok())
        .collect::<Vec<_>>();
    version_dirs.sort_unstable_by(|left, right| right.cmp(left));

    for version_dir in version_dirs {
        let libexec_tsc = version_dir
            .join("libexec")
            .join("node_modules")
            .join("typescript")
            .join("bin")
            .join("tsc");
        if libexec_tsc.is_file() {
            return Some(libexec_tsc);
        }

        let client_root = version_dir.join("lib").join("client");
        let Ok(client_entries) = fs::read_dir(client_root.as_std_path()) else {
            continue;
        };
        let mut client_dirs = client_entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| Utf8PathBuf::from_path_buf(entry.path()).ok())
            .collect::<Vec<_>>();
        client_dirs.sort_unstable_by(|left, right| right.cmp(left));

        for client_dir in client_dirs {
            let client_tsc = client_dir
                .join("node_modules")
                .join("typescript")
                .join("bin")
                .join("tsc");
            if client_tsc.is_file() {
                return Some(client_tsc);
            }
        }
    }

    None
}

fn rewrite_package_dependency_to_local_fixture(
    package_dir: &Utf8PathBuf,
    dependency_name: &str,
    dependency_dir: &Utf8PathBuf,
) {
    set_package_dependency(
        package_dir,
        dependency_name,
        &format!("file:{}", dependency_dir),
    );
}

fn add_package_dependency(package_dir: &Utf8PathBuf, dependency_name: &str, version_spec: &str) {
    set_package_dependency(package_dir, dependency_name, version_spec);
}

fn set_package_dependency(package_dir: &Utf8PathBuf, dependency_name: &str, dependency_spec: &str) {
    let package_json_path = package_dir.join("package.json");
    let mut package_json: Value = serde_json::from_str(
        &fs::read_to_string(package_json_path.as_std_path()).unwrap_or_else(|error| {
            panic!("failed to read generated package manifest {package_json_path}: {error}")
        }),
    )
    .unwrap_or_else(|error| {
        panic!("failed to parse generated package manifest {package_json_path}: {error}")
    });

    let package_object = package_json
        .as_object_mut()
        .unwrap_or_else(|| panic!("generated package manifest is not a JSON object"));
    let dependencies = package_object
        .entry("dependencies".to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()))
        .as_object_mut()
        .unwrap_or_else(|| {
            panic!("generated package manifest dependencies field is not an object")
        });
    dependencies.insert(
        dependency_name.to_string(),
        Value::String(dependency_spec.to_string()),
    );

    fs::write(
        package_json_path.as_std_path(),
        serde_json::to_vec_pretty(&package_json).expect("package.json should serialize"),
    )
    .unwrap_or_else(|error| {
        panic!("failed to write generated package manifest {package_json_path}: {error}")
    });
}

fn run_cargo_command(
    fixture_name: &str,
    operation: &str,
    manifest_path: &Utf8PathBuf,
    target_dir: &Utf8PathBuf,
    args: &[&str],
) {
    let output = Command::new(env!("CARGO"))
        .args(args)
        .arg("--manifest-path")
        .arg(manifest_path.as_str())
        .env("CARGO_TARGET_DIR", target_dir.as_str())
        .output()
        .unwrap_or_else(|error| {
            panic!("failed to run cargo {operation} for fixture {fixture_name}: {error}")
        });

    if !output.status.success() {
        panic!(
            "failed to run cargo {operation} for fixture {fixture_name}\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
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

fn read_staged_library_package_relative_path(
    package_dir: &Utf8PathBuf,
    namespace: &str,
) -> Utf8PathBuf {
    let ffi_js_path = package_dir.join(format!("{namespace}-ffi.js"));
    let ffi_js = fs::read_to_string(ffi_js_path.as_std_path())
        .unwrap_or_else(|error| panic!("failed to read generated ffi file {ffi_js_path}: {error}"));
    let raw_relative_path = ffi_js
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("stagedLibraryPackageRelativePath: ")
                .map(|value| value.trim_end_matches(','))
        })
        .unwrap_or_else(|| {
            panic!(
                "generated ffi file {} should include stagedLibraryPackageRelativePath metadata",
                ffi_js_path
            )
        });
    let relative_path: String = serde_json::from_str(raw_relative_path).unwrap_or_else(|error| {
        panic!(
            "generated ffi metadata in {ffi_js_path} should serialize stagedLibraryPackageRelativePath as JSON: {error}"
        )
    });

    Utf8PathBuf::from(relative_path)
}
