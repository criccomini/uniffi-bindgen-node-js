use std::fs;

mod support;

use self::support::{
    fixtures::fixture_spec, generate_fixture_package, remove_dir_all,
};

fn snapshot_output_for_fixture(name: &str) -> String {
    let spec = fixture_spec(name);
    let generated = generate_fixture_package(name);

    let mut snapshot = String::new();
    for relative_path in spec.generated_binding_relative_paths() {
        let contents = fs::read_to_string(generated.package_dir.join(&relative_path).as_std_path())
            .unwrap_or_else(|error| {
                panic!(
                    "failed to read generated file {} for fixture {}: {error}",
                    generated.package_dir.join(&relative_path),
                    spec.dir_name
                )
            });
        snapshot.push_str(&format!("=== {relative_path} ===\n{contents}\n"));
    }

    remove_dir_all(&generated.package_dir);
    remove_dir_all(&generated.built_fixture.workspace_dir);

    snapshot
}

#[test]
fn snapshots_basic_fixture_generated_output() {
    insta::assert_snapshot!(
        "basic_fixture_generated_output",
        snapshot_output_for_fixture("basic")
    );
}

#[test]
fn snapshots_callback_fixture_generated_output() {
    insta::assert_snapshot!(
        "callback_fixture_generated_output",
        snapshot_output_for_fixture("callbacks")
    );
}

#[test]
fn snapshots_docs_fixture_generated_output() {
    insta::assert_snapshot!(
        "docs_fixture_generated_output",
        snapshot_output_for_fixture("docs")
    );
}
