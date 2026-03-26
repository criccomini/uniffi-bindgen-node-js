mod support;

use self::support::{
    generate_fixture_package, install_fixture_package_dependencies, remove_dir_all,
};

#[test]
fn generates_basic_fixture_node_package_in_a_temp_directory() {
    let generated = generate_fixture_package("basic");
    let package_dir = &generated.package_dir;
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

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
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
