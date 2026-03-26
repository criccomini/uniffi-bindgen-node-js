use std::{
    fs, process,
    time::{SystemTime, UNIX_EPOCH},
};

use camino::Utf8PathBuf;
use uniffi_bindgen::{
    BindingGenerator, Component, GenerationSettings, interface::ComponentInterface,
};
use uniffi_bindgen_node_js::bindings::{
    NodeBindingCliOverrides, NodeBindingGenerator, NodeBindingGeneratorConfig,
};

const RUNTIME_FILES: &[&str] = &[
    "runtime/errors.js",
    "runtime/errors.d.ts",
    "runtime/ffi-types.js",
    "runtime/ffi-types.d.ts",
    "runtime/ffi-converters.js",
    "runtime/ffi-converters.d.ts",
    "runtime/rust-call.js",
    "runtime/rust-call.d.ts",
    "runtime/async-rust-call.js",
    "runtime/async-rust-call.d.ts",
    "runtime/handle-map.js",
    "runtime/handle-map.d.ts",
    "runtime/callbacks.js",
    "runtime/callbacks.d.ts",
    "runtime/objects.js",
    "runtime/objects.d.ts",
];

struct FixtureSpec {
    dir_name: &'static str,
    crate_name: &'static str,
    namespace: &'static str,
    udl_file: &'static str,
}

fn fixture_spec(name: &str) -> FixtureSpec {
    match name {
        "basic" => FixtureSpec {
            dir_name: "basic-fixture",
            crate_name: "fixture_basic",
            namespace: "fixture",
            udl_file: "fixture.udl",
        },
        "callbacks" => FixtureSpec {
            dir_name: "callback-fixture",
            crate_name: "fixture_callbacks",
            namespace: "callbacks_fixture",
            udl_file: "callbacks_fixture.udl",
        },
        _ => panic!("unknown fixture '{name}'"),
    }
}

fn fixture_component(spec: &FixtureSpec) -> Component<NodeBindingGeneratorConfig> {
    let udl_path = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(spec.dir_name)
        .join("src")
        .join(spec.udl_file);
    let udl_source = fs::read_to_string(udl_path.as_std_path())
        .unwrap_or_else(|error| panic!("failed to read fixture UDL at {udl_path}: {error}"));

    Component {
        ci: ComponentInterface::from_webidl(&udl_source, spec.crate_name)
            .unwrap_or_else(|error| panic!("failed to parse fixture UDL {udl_path}: {error}")),
        config: NodeBindingGeneratorConfig {
            package_name: Some(format!("{}-package", spec.namespace)),
            cdylib_name: Some(spec.crate_name.to_string()),
            ..NodeBindingGeneratorConfig::default()
        },
    }
}

fn temp_dir_path(name: &str) -> Utf8PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    Utf8PathBuf::from_path_buf(std::env::temp_dir().join(format!(
        "uniffi-bindgen-node-js-snapshots-{name}-{}-{unique}",
        process::id()
    )))
    .expect("temp dir path should be utf-8")
}

fn snapshot_output_for_fixture(name: &str) -> String {
    let spec = fixture_spec(name);
    let generator = NodeBindingGenerator::new(NodeBindingCliOverrides::default());
    let out_dir = temp_dir_path(spec.dir_name);
    let settings = GenerationSettings {
        out_dir: out_dir.clone(),
        try_format_code: false,
        cdylib: Some(spec.crate_name.to_string()),
    };

    generator
        .write_bindings(&settings, &[fixture_component(&spec)])
        .unwrap_or_else(|error| {
            panic!("failed to generate bindings for {}: {error}", spec.dir_name)
        });

    let mut files = vec![
        "index.js".to_string(),
        "index.d.ts".to_string(),
        format!("{}.js", spec.namespace),
        format!("{}.d.ts", spec.namespace),
        format!("{}-ffi.js", spec.namespace),
        format!("{}-ffi.d.ts", spec.namespace),
    ];
    files.extend(RUNTIME_FILES.iter().map(|path| (*path).to_string()));

    let mut snapshot = String::new();
    for relative_path in files {
        let contents = fs::read_to_string(out_dir.join(&relative_path).as_std_path())
            .unwrap_or_else(|error| {
                panic!(
                    "failed to read generated file {} for fixture {}: {error}",
                    out_dir.join(&relative_path),
                    spec.dir_name
                )
            });
        snapshot.push_str(&format!("=== {relative_path} ===\n{contents}\n"));
    }

    fs::remove_dir_all(out_dir.as_std_path())
        .unwrap_or_else(|error| panic!("failed to remove temp dir {out_dir}: {error}"));

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
