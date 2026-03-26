mod support;

use self::support::{
    generate_slatedb_package, install_generated_package_dependencies, remove_dir_all,
};

#[test]
fn generates_slatedb_node_package_in_a_temp_directory() {
    let generated = generate_slatedb_package();
    let package_dir = &generated.package_dir;
    let namespace = &generated.namespace;
    let expected_library_filename = format!(
        "{}{}.{}",
        std::env::consts::DLL_PREFIX,
        generated.built_slatedb.crate_name,
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
        assert!(path.is_file(), "expected generated SlateDB package file at {path}");
    }

    install_generated_package_dependencies(package_dir);
    assert!(
        package_dir.join("node_modules").join("koffi").join("package.json").is_file(),
        "expected generated SlateDB package dependencies to be installed in {package_dir}"
    );

    remove_dir_all(&generated.built_slatedb.target_dir);
    remove_dir_all(package_dir);
}
