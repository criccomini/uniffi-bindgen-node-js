#![allow(dead_code)]

use std::{
    env, fs, process,
    process::Command,
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
            manual_load: false,
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
    let packaged_library_path = package_dir.join(library_filename);
    fs::copy(
        built_fixture.library_path.as_std_path(),
        packaged_library_path.as_std_path(),
    )
    .unwrap_or_else(|error| {
        panic!(
            "failed to copy fixture library {} to {}: {error}",
            built_fixture.library_path, packaged_library_path
        )
    });

    GeneratedFixturePackage {
        built_fixture,
        package_dir,
    }
}

pub fn remove_dir_all(path: &Utf8PathBuf) {
    if path.exists() {
        fs::remove_dir_all(path.as_std_path())
            .unwrap_or_else(|error| panic!("failed to remove temp dir {path}: {error}"));
    }
}

pub fn temp_dir_path(name: &str) -> Utf8PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    Utf8PathBuf::from_path_buf(env::temp_dir().join(format!(
        "uniffi-bindgen-node-js-tests-{name}-{}-{unique}",
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
