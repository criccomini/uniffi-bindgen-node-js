#![allow(dead_code)]

use std::{
    env, fs, process,
    time::{SystemTime, UNIX_EPOCH},
};

use camino::Utf8PathBuf;
use uniffi_bindgen::{Component, GenerationSettings, interface::ComponentInterface};
use uniffi_bindgen_node_js::bindings::{
    NodeBindingCliOverrides, NodeBindingGenerator, NodeBindingGeneratorConfig,
};

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
