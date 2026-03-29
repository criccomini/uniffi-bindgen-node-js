mod support;

use self::support::{
    FixturePackageOptions, generate_fixture_package, generate_fixture_package_with_options,
    install_fixture_package_benchmark_dependencies,
    install_fixture_package_dependencies_with_real_koffi, remove_dir_all, run_node_program,
    stage_package_benchmark_scripts,
};

#[test]
#[ignore = "requires npm registry access to install real koffi and tinybench"]
fn benchmarks_basic_generated_package_hot_paths() {
    let generated = generate_fixture_package("basic");
    let package_dir = &generated.package_dir;

    stage_package_benchmark_scripts(package_dir);
    install_fixture_package_benchmark_dependencies(package_dir);

    let stdout = run_node_program(
        package_dir,
        "benchmarks/basic-hot-path.mjs",
        &["--expose-gc"],
        &[],
    );
    if !stdout.trim().is_empty() {
        println!("{stdout}");
    }

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

#[test]
#[ignore = "requires npm registry access to install real koffi and tinybench"]
fn benchmarks_callback_generated_package_hot_paths() {
    let generated = generate_fixture_package("callbacks");
    let package_dir = &generated.package_dir;

    stage_package_benchmark_scripts(package_dir);
    install_fixture_package_benchmark_dependencies(package_dir);

    let stdout = run_node_program(
        package_dir,
        "benchmarks/callback-hot-path.mjs",
        &["--expose-gc"],
        &[],
    );
    if !stdout.trim().is_empty() {
        println!("{stdout}");
    }

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

#[test]
#[ignore = "requires npm registry access to install real koffi and tinybench"]
fn benchmarks_generated_package_startup_and_lifecycle() {
    let eager_generated = generate_fixture_package("basic");
    let eager_package_dir = &eager_generated.package_dir;
    let manual_generated = generate_fixture_package_with_options(
        "basic",
        FixturePackageOptions {
            manual_load: true,
            ..FixturePackageOptions::default()
        },
    );
    let manual_package_dir = &manual_generated.package_dir;

    stage_package_benchmark_scripts(eager_package_dir);
    stage_package_benchmark_scripts(manual_package_dir);
    install_fixture_package_benchmark_dependencies(eager_package_dir);
    install_fixture_package_dependencies_with_real_koffi(manual_package_dir);

    let stdout = run_node_program(
        eager_package_dir,
        "benchmarks/startup-lifecycle.mjs",
        &["--expose-gc"],
        &[("UNIFFI_MANUAL_PACKAGE_DIR", manual_package_dir.as_str())],
    );
    if !stdout.trim().is_empty() {
        println!("{stdout}");
    }

    remove_dir_all(&eager_generated.built_fixture.workspace_dir);
    remove_dir_all(eager_package_dir);
    remove_dir_all(&manual_generated.built_fixture.workspace_dir);
    remove_dir_all(manual_package_dir);
}
