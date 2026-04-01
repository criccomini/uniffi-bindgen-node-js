mod support;

use std::fs;

use self::support::{
    FixturePackageOptions, build_fixture_cdylib, fixtures::fixture_spec, generate_fixture_package,
    generate_fixture_package_with_options, install_fixture_package_dependencies,
    read_package_file_tree, remove_dir_all, temp_dir_path,
};
use serde_json::Value;
use uniffi_bindgen_node_js::{GenerateNodePackageOptions, generate_node_package};

#[test]
fn generates_basic_fixture_node_package_in_a_temp_directory() {
    let generated = generate_fixture_package("basic");
    let package_dir = &generated.package_dir;
    let spec = fixture_spec("basic");
    let namespace = &generated.built_fixture.namespace;
    let expected_library_filename = format!(
        "{}{}.{}",
        std::env::consts::DLL_PREFIX,
        generated.built_fixture.crate_name,
        std::env::consts::DLL_EXTENSION
    );

    for relative_path in [
        "package.json",
        "index.js",
        "index.d.ts",
        &format!("{namespace}.js"),
        &format!("{namespace}.d.ts"),
        &format!("{namespace}-ffi.js"),
        &format!("{namespace}-ffi.d.ts"),
        "runtime/errors.js",
        "runtime/ffi-types.js",
        "runtime/ffi-converters.js",
        "runtime/rust-call.js",
        "runtime/async-rust-call.js",
        "runtime/handle-map.js",
        "runtime/callbacks.js",
        "runtime/objects.js",
        &expected_library_filename,
    ] {
        let path = package_dir.join(relative_path);
        assert!(path.is_file(), "expected generated package file at {path}");
    }

    let mut expected_paths = spec.generated_package_relative_paths();
    expected_paths.push(expected_library_filename);
    expected_paths.sort();

    remove_dir_all(&generated.built_fixture.workspace_dir);
    assert_eq!(
        read_package_file_tree(package_dir)
            .into_keys()
            .collect::<Vec<_>>(),
        expected_paths,
        "unexpected generated package file layout"
    );
    remove_dir_all(package_dir);
}

#[test]
fn infers_the_only_component_when_crate_name_is_omitted() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("infer-basic-package");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: None,
        out_dir: package_dir.clone(),
        package_name: Some(format!("{}-package", built_fixture.namespace)),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect("single-component library should not require --crate-name");

    assert!(
        package_dir.join("package.json").is_file(),
        "expected generated package manifest at {}",
        package_dir.join("package.json")
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn defaults_package_name_to_the_selected_component_namespace() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("default-package-name");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: None,
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect("package generation should default the npm package name from the namespace");

    let package_json: Value = serde_json::from_str(
        &fs::read_to_string(package_dir.join("package.json").as_std_path())
            .expect("package.json should be readable"),
    )
    .expect("package.json should parse");

    assert_eq!(
        package_json.get("name").and_then(Value::as_str),
        Some(built_fixture.namespace.as_str()),
        "unexpected package.json contents: {package_json:#}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn rerunning_generation_into_fresh_empty_directories_is_deterministic() {
    let built_fixture = build_fixture_cdylib("basic");
    let first_package_dir = temp_dir_path("deterministic-package-first");
    let second_package_dir = temp_dir_path("deterministic-package-second");

    for out_dir in [&first_package_dir, &second_package_dir] {
        generate_node_package(GenerateNodePackageOptions {
            lib_source: built_fixture.library_path.clone(),
            manifest_path: Some(built_fixture.manifest_path.clone()),
            crate_name: Some(built_fixture.crate_name.clone()),
            out_dir: out_dir.clone(),
            package_name: Some(format!("{}-package", built_fixture.namespace)),
            node_engine: None,
            bundled_prebuilds: false,
            manual_load: false,
        })
        .expect("package generation should be deterministic across fresh output directories");
    }

    assert_eq!(
        read_package_file_tree(&first_package_dir),
        read_package_file_tree(&second_package_dir),
        "expected byte-for-byte identical package output across fresh directories"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&first_package_dir);
    remove_dir_all(&second_package_dir);
}

#[test]
fn package_name_override_wins_over_the_namespace_default() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("override-package-name");
    let package_name = "custom-generated-package";

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: Some(package_name.to_string()),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect("package generation should keep the explicit package-name override");

    let package_json: Value = serde_json::from_str(
        &fs::read_to_string(package_dir.join("package.json").as_std_path())
            .expect("package.json should be readable"),
    )
    .expect("package.json should parse");

    assert_eq!(
        package_json.get("name").and_then(Value::as_str),
        Some(package_name),
        "unexpected package.json contents: {package_json:#}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn node_engine_override_is_written_to_package_json() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("override-node-engine");
    let node_engine = ">=20.11.0";

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: None,
        node_engine: Some(node_engine.to_string()),
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect("package generation should keep the explicit node-engine override");

    let package_json: Value = serde_json::from_str(
        &fs::read_to_string(package_dir.join("package.json").as_std_path())
            .expect("package.json should be readable"),
    )
    .expect("package.json should parse");

    assert_eq!(
        package_json
            .get("engines")
            .and_then(Value::as_object)
            .and_then(|engines| engines.get("node"))
            .and_then(Value::as_str),
        Some(node_engine),
        "unexpected package.json contents: {package_json:#}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn bundled_prebuilds_option_emits_bundled_loader_metadata() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("bundled-prebuild-loader");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: None,
        node_engine: None,
        bundled_prebuilds: true,
        manual_load: false,
    })
    .expect("package generation should keep bundled-prebuild loader support");

    let ffi_js = fs::read_to_string(
        package_dir
            .join(format!("{}-ffi.js", built_fixture.namespace))
            .as_std_path(),
    )
    .expect("component ffi js should be readable");

    assert!(
        ffi_js.contains("bundledPrebuilds: true"),
        "unexpected component FFI JS contents: {ffi_js}"
    );
    assert!(
        ffi_js.contains("packageRelativePath: `prebuilds/${target}/${filename}`,"),
        "unexpected component FFI JS contents: {ffi_js}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn manual_load_option_exports_manual_lifecycle_helpers() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("manual-load-helpers");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: None,
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: true,
    })
    .expect("package generation should keep manual-load support");

    let component_js = fs::read_to_string(
        package_dir
            .join(format!("{}.js", built_fixture.namespace))
            .as_std_path(),
    )
    .expect("component js should be readable");
    let ffi_js = fs::read_to_string(
        package_dir
            .join(format!("{}-ffi.js", built_fixture.namespace))
            .as_std_path(),
    )
    .expect("component ffi js should be readable");

    assert!(
        component_js.contains(&format!(
            "export {{ load, unload }} from \"./{}-ffi.js\";",
            built_fixture.namespace
        )),
        "unexpected component JS contents: {component_js}"
    );
    assert!(
        ffi_js.contains("manualLoad: true"),
        "unexpected component FFI JS contents: {ffi_js}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn generates_udl_backed_callback_fixture_when_manifest_path_is_provided() {
    let built_fixture = build_fixture_cdylib("callbacks");
    let package_dir = temp_dir_path("callbacks-manifest-path-package");

    generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: Some(format!("{}-package", built_fixture.namespace)),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect("UDL-backed library should load with --manifest-path");

    assert!(
        package_dir.join("package.json").is_file(),
        "expected generated package manifest at {}",
        package_dir.join("package.json")
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn rejects_missing_library_source_from_programmatic_entrypoint() {
    let package_dir = temp_dir_path("missing-library-package");
    let missing_library_path = package_dir.join("missing-library.so");

    let error = generate_node_package(GenerateNodePackageOptions {
        lib_source: missing_library_path.clone(),
        manifest_path: None,
        crate_name: None,
        out_dir: package_dir.clone(),
        package_name: Some("missing-library-package".to_string()),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect_err("missing library path should be rejected by the v2 entrypoint");

    assert!(
        error.to_string().contains(&format!(
            "built UniFFI cdylib '{}' does not exist",
            missing_library_path
        )),
        "unexpected error: {error:#}"
    );

    remove_dir_all(&package_dir);
}

#[test]
fn rejects_file_out_dir_from_programmatic_entrypoint() {
    let built_fixture = build_fixture_cdylib("basic");
    let package_dir = temp_dir_path("file-out-dir-package");
    std::fs::write(package_dir.as_std_path(), "not a directory")
        .expect("test should create a file-backed out-dir path");

    let error = generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.manifest_path.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: Some(format!("{}-package", built_fixture.namespace)),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect_err("file-backed out-dir should be rejected by the v2 entrypoint");

    assert!(
        error.to_string().contains(&format!(
            "--out-dir '{}' exists but is not a directory",
            package_dir
        )),
        "unexpected error: {error:#}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    std::fs::remove_file(package_dir.as_std_path())
        .expect("test should remove the file-backed out-dir path");
}

#[test]
fn rejects_directory_manifest_path_from_programmatic_entrypoint() {
    let built_fixture = build_fixture_cdylib("callbacks");
    let package_dir = temp_dir_path("directory-manifest-path-package");

    let error = generate_node_package(GenerateNodePackageOptions {
        lib_source: built_fixture.library_path.clone(),
        manifest_path: Some(built_fixture.workspace_dir.clone()),
        crate_name: Some(built_fixture.crate_name.clone()),
        out_dir: package_dir.clone(),
        package_name: Some(format!("{}-package", built_fixture.namespace)),
        node_engine: None,
        bundled_prebuilds: false,
        manual_load: false,
    })
    .expect_err("directory manifest path should be rejected by the v2 entrypoint");

    assert!(
        error.to_string().contains(&format!(
            "--manifest-path '{}' must point to a Cargo.toml file",
            built_fixture.workspace_dir
        )),
        "unexpected error: {error:#}"
    );

    remove_dir_all(&built_fixture.workspace_dir);
    remove_dir_all(&package_dir);
}

#[test]
fn installs_fixture_package_npm_dependencies_in_a_temp_directory() {
    let generated = generate_fixture_package("basic");
    let package_dir = &generated.package_dir;

    install_fixture_package_dependencies(package_dir);

    let installed_koffi_manifest = package_dir
        .join("node_modules")
        .join("koffi")
        .join("package.json");
    assert!(
        installed_koffi_manifest.is_file(),
        "expected installed koffi package manifest at {}",
        installed_koffi_manifest
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

#[test]
fn generates_callback_fixture_package_with_expected_files_and_local_koffi_fixture() {
    let generated = generate_fixture_package("callbacks");
    let package_dir = &generated.package_dir;
    let spec = fixture_spec("callbacks");
    let expected_library_filename = format!(
        "{}{}.{}",
        std::env::consts::DLL_PREFIX,
        generated.built_fixture.crate_name,
        std::env::consts::DLL_EXTENSION
    );

    for relative_path in spec
        .generated_package_relative_paths()
        .into_iter()
        .chain(std::iter::once(expected_library_filename))
    {
        let path = package_dir.join(&relative_path);
        assert!(path.is_file(), "expected generated package file at {path}");
    }

    install_fixture_package_dependencies(package_dir);

    let installed_koffi_manifest = package_dir
        .join("node_modules")
        .join("koffi")
        .join("package.json");
    assert!(
        installed_koffi_manifest.is_file(),
        "expected installed koffi package manifest at {}",
        installed_koffi_manifest
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

#[test]
fn generates_bundled_basic_fixture_package_with_only_a_host_prebuild() {
    let generated = generate_fixture_package_with_options(
        "basic",
        FixturePackageOptions {
            bundled_prebuilds: true,
            ..FixturePackageOptions::default()
        },
    );
    let package_dir = &generated.package_dir;
    let namespace = &generated.built_fixture.namespace;
    let expected_library_filename = format!(
        "{}{}.{}",
        std::env::consts::DLL_PREFIX,
        generated.built_fixture.crate_name,
        std::env::consts::DLL_EXTENSION
    );
    let bundled_target = generated
        .bundled_prebuild_target
        .as_deref()
        .expect("bundled-mode fixture package should record the staged target");
    let bundled_library_path = generated
        .bundled_prebuild_path
        .as_ref()
        .expect("bundled-mode fixture package should record the staged prebuild path");
    let bundled_library_relative_path =
        format!("prebuilds/{bundled_target}/{expected_library_filename}");
    let root_library_path = package_dir.join(&expected_library_filename);

    assert!(
        generated.sibling_library_path.is_none(),
        "bundled-mode helper should not stage a sibling library at the package root"
    );

    for relative_path in [
        "package.json",
        "index.js",
        "index.d.ts",
        &format!("{namespace}.js"),
        &format!("{namespace}.d.ts"),
        &format!("{namespace}-ffi.js"),
        &format!("{namespace}-ffi.d.ts"),
        "runtime/errors.js",
        "runtime/ffi-types.js",
        "runtime/ffi-converters.js",
        "runtime/rust-call.js",
        "runtime/async-rust-call.js",
        "runtime/handle-map.js",
        "runtime/callbacks.js",
        "runtime/objects.js",
        &bundled_library_relative_path,
    ] {
        let path = package_dir.join(relative_path);
        assert!(path.is_file(), "expected generated package file at {path}");
    }

    assert!(
        !root_library_path.exists(),
        "bundled-mode package should not stage a root-level sibling library at {root_library_path}"
    );
    assert_eq!(
        bundled_library_path,
        &package_dir.join(&bundled_library_relative_path),
        "generator should stage the host library at the expected bundled-prebuild path"
    );

    let mut expected_paths = fixture_spec("basic").generated_package_relative_paths();
    expected_paths.push(bundled_library_relative_path.clone());
    expected_paths.sort();
    assert_eq!(
        read_package_file_tree(package_dir)
            .into_keys()
            .collect::<Vec<_>>(),
        expected_paths,
        "unexpected bundled package file layout"
    );

    install_fixture_package_dependencies(package_dir);

    let installed_koffi_manifest = package_dir
        .join("node_modules")
        .join("koffi")
        .join("package.json");
    assert!(
        installed_koffi_manifest.is_file(),
        "expected installed koffi package manifest at {}",
        installed_koffi_manifest
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}
