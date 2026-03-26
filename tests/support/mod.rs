#![allow(dead_code)]

use std::{
    env, fs, process,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use camino::Utf8PathBuf;
use serde_json::Value;
use uniffi_bindgen::{Component, GenerationSettings, interface::ComponentInterface};
use uniffi_bindgen_node_js::bindings::{
    NodeBindingCliOverrides, NodeBindingGenerator, NodeBindingGeneratorConfig,
};

pub struct BuiltFixtureCdylib {
    pub workspace_dir: Utf8PathBuf,
    pub manifest_path: Utf8PathBuf,
    pub namespace: String,
    pub crate_name: String,
    pub library_path: Utf8PathBuf,
}

pub struct GeneratedFixturePackage {
    pub built_fixture: BuiltFixtureCdylib,
    pub package_dir: Utf8PathBuf,
    pub sibling_library_path: Option<Utf8PathBuf>,
    pub bundled_prebuild_target: Option<String>,
    pub bundled_prebuild_path: Option<Utf8PathBuf>,
}

#[derive(Clone, Copy, Debug)]
pub struct FixturePackageOptions {
    pub bundled_prebuilds: bool,
    pub manual_load: bool,
    pub stage_root_sibling_library: bool,
    pub stage_host_prebuild: bool,
}

impl Default for FixturePackageOptions {
    fn default() -> Self {
        Self {
            bundled_prebuilds: false,
            manual_load: false,
            stage_root_sibling_library: true,
            stage_host_prebuild: false,
        }
    }
}

pub fn generator() -> NodeBindingGenerator {
    NodeBindingGenerator::new(NodeBindingCliOverrides::default())
}

pub fn generation_settings(name: &str) -> GenerationSettings {
    GenerationSettings {
        out_dir: temp_dir_path(name),
        try_format_code: false,
        cdylib: Some("fixture".to_string()),
    }
}

pub fn component_from_webidl(source: &str) -> Component<NodeBindingGeneratorConfig> {
    Component {
        ci: ComponentInterface::from_webidl(source, "fixture_crate").expect("valid test UDL"),
        config: NodeBindingGeneratorConfig {
            package_name: Some("fixture-package".to_string()),
            cdylib_name: Some("fixture".to_string()),
            ..NodeBindingGeneratorConfig::default()
        },
    }
}

pub fn component_with_namespace(namespace: &str) -> Component<NodeBindingGeneratorConfig> {
    Component {
        ci: ComponentInterface::from_webidl(
            &format!("namespace {namespace} {{}};"),
            "fixture_crate",
        )
        .expect("valid test UDL"),
        config: NodeBindingGeneratorConfig {
            package_name: Some(format!("{namespace}-package")),
            cdylib_name: Some("fixture".to_string()),
            ..NodeBindingGeneratorConfig::default()
        },
    }
}

pub fn read_generated_file(out_dir: &Utf8PathBuf, relative_path: &str) -> String {
    fs::read_to_string(out_dir.join(relative_path).as_std_path())
        .unwrap_or_else(|error| panic!("failed to read generated file {relative_path}: {error}"))
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

    let library_path =
        find_cdylib_artifact(&output.stdout, &spec.crate_name).unwrap_or_else(|| {
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

pub fn generate_fixture_package(name: &str) -> GeneratedFixturePackage {
    generate_fixture_package_with_options(name, FixturePackageOptions::default())
}

pub fn generate_fixture_package_with_options(
    name: &str,
    options: FixturePackageOptions,
) -> GeneratedFixturePackage {
    let built_fixture = build_fixture_cdylib(name);
    let package_dir = temp_dir_path(&format!("fixture-{name}-package"));

    uniffi_bindgen_node_js::subcommands::generate::run(
        uniffi_bindgen_node_js::subcommands::generate::GenerateArgs {
            lib_source: built_fixture.library_path.clone(),
            crate_name: built_fixture.crate_name.clone(),
            out_dir: package_dir.clone(),
            package_name: Some(format!("{}-package", built_fixture.namespace)),
            cdylib_name: Some(built_fixture.crate_name.clone()),
            node_engine: None,
            lib_path_literal: None,
            bundled_prebuilds: options.bundled_prebuilds,
            manual_load: options.manual_load,
            config_override: Vec::new(),
        },
    )
    .unwrap_or_else(|error| panic!("failed to generate fixture package {name}: {error:#}"));

    let library_filename = built_fixture.library_path.file_name().unwrap_or_else(|| {
        panic!(
            "fixture library path has no filename: {}",
            built_fixture.library_path
        )
    });
    let sibling_library_path = options.stage_root_sibling_library.then(|| {
        let packaged_library_path = package_dir.join(library_filename);
        copy_library(&built_fixture.library_path, &packaged_library_path, "fixture");
        packaged_library_path
    });
    let (bundled_prebuild_target, bundled_prebuild_path) = if options.stage_host_prebuild {
        let target = current_bundled_prebuild_target();
        let packaged_library_path = package_dir
            .join("prebuilds")
            .join(&target)
            .join(library_filename);
        copy_library(&built_fixture.library_path, &packaged_library_path, "fixture");
        (Some(target), Some(packaged_library_path))
    } else {
        (None, None)
    };

    GeneratedFixturePackage {
        built_fixture,
        package_dir,
        sibling_library_path,
        bundled_prebuild_target,
        bundled_prebuild_path,
    }
}

pub fn install_generated_package_dependencies(package_dir: &Utf8PathBuf) {
    rewrite_package_dependency_to_local_fixture(package_dir, "koffi", &local_koffi_fixture_dir());

    let output = Command::new("npm")
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

pub fn install_fixture_package_dependencies(package_dir: &Utf8PathBuf) {
    install_generated_package_dependencies(package_dir);
}

pub fn run_node_script(package_dir: &Utf8PathBuf, script_name: &str, source: &str) {
    let script_path = package_dir.join(script_name);
    fs::write(script_path.as_std_path(), source)
        .unwrap_or_else(|error| panic!("failed to write Node script {script_path}: {error}"));

    let output = Command::new("node")
        .arg(script_path.as_str())
        .current_dir(package_dir.as_std_path())
        .output()
        .unwrap_or_else(|error| panic!("failed to run Node script {script_path}: {error}"));

    if !output.status.success() {
        panic!(
            "Node script {script_path} failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
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

struct FixtureSpec {
    dir_name: &'static str,
    namespace: &'static str,
    crate_name: &'static str,
}

fn fixture_spec(name: &str) -> FixtureSpec {
    match name {
        "basic" => FixtureSpec {
            dir_name: "basic-fixture",
            namespace: "fixture",
            crate_name: "fixture_basic",
        },
        "callbacks" => FixtureSpec {
            dir_name: "callback-fixture",
            namespace: "callbacks_fixture",
            crate_name: "fixture_callbacks",
        },
        _ => panic!("unknown fixture '{name}'"),
    }
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

pub fn current_bundled_prebuild_target() -> String {
    let platform = current_node_platform();
    let arch = current_node_arch();

    if platform != "linux" {
        return format!("{platform}-{arch}");
    }

    format!("{platform}-{arch}-{}", current_linux_libc())
}

fn local_koffi_fixture_dir() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("npm-fixtures")
        .join("koffi")
}

fn find_typescript_cli() -> Option<Utf8PathBuf> {
    let output = Command::new("zsh")
        .args([
            "-lc",
            "command -v tsc || ls -1d /opt/homebrew/Cellar/heroku/*/libexec/node_modules/typescript/bin/tsc 2>/dev/null | head -n 1 || ls -1d /opt/homebrew/Cellar/heroku/*/lib/client/*/node_modules/typescript/bin/tsc 2>/dev/null | head -n 1",
        ])
        .output()
        .unwrap_or_else(|error| panic!("failed to search for a local TypeScript compiler: {error}"));

    if !output.status.success() {
        return None;
    }

    let candidate = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if candidate.is_empty() {
        None
    } else {
        Some(Utf8PathBuf::from(candidate))
    }
}

fn rewrite_package_dependency_to_local_fixture(
    package_dir: &Utf8PathBuf,
    dependency_name: &str,
    dependency_dir: &Utf8PathBuf,
) {
    let package_json_path = package_dir.join("package.json");
    let mut package_json: Value = serde_json::from_str(
        &fs::read_to_string(package_json_path.as_std_path()).unwrap_or_else(|error| {
            panic!("failed to read generated package manifest {package_json_path}: {error}")
        }),
    )
    .unwrap_or_else(|error| {
        panic!("failed to parse generated package manifest {package_json_path}: {error}")
    });

    let dependencies = package_json
        .get_mut("dependencies")
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("generated package manifest is missing dependencies"));
    dependencies.insert(
        dependency_name.to_string(),
        Value::String(format!("file:{}", dependency_dir)),
    );

    fs::write(
        package_json_path.as_std_path(),
        serde_json::to_vec_pretty(&package_json).expect("package.json should serialize"),
    )
    .unwrap_or_else(|error| {
        panic!("failed to write generated package manifest {package_json_path}: {error}")
    });
}

fn copy_library(source_path: &Utf8PathBuf, destination_path: &Utf8PathBuf, context: &str) {
    if let Some(parent) = destination_path.parent() {
        fs::create_dir_all(parent.as_std_path()).unwrap_or_else(|error| {
            panic!("failed to create {context} library dir {parent}: {error}")
        });
    }

    fs::copy(source_path.as_std_path(), destination_path.as_std_path()).unwrap_or_else(|error| {
        panic!("failed to copy {context} library {source_path} to {destination_path}: {error}")
    });
}

fn current_node_platform() -> &'static str {
    match env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        "linux" => "linux",
        "android" => "android",
        "aix" => "aix",
        "freebsd" => "freebsd",
        "openbsd" => "openbsd",
        other => panic!("unsupported host OS for Node bundled-prebuild tests: {other}"),
    }
}

fn current_node_arch() -> &'static str {
    match env::consts::ARCH {
        "x86_64" => "x64",
        "x86" => "ia32",
        "aarch64" => "arm64",
        "arm" => "arm",
        "loongarch64" => "loong64",
        "powerpc64" => "ppc64",
        "riscv64" => "riscv64",
        "s390x" => "s390x",
        other => panic!("unsupported host architecture for Node bundled-prebuild tests: {other}"),
    }
}

fn current_linux_libc() -> &'static str {
    if cfg!(target_env = "gnu") {
        "gnu"
    } else if cfg!(target_env = "musl") {
        "musl"
    } else {
        panic!("unsupported Linux target environment for Node bundled-prebuild tests");
    }
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
