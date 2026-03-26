#![allow(dead_code)]

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

#[derive(Clone, Copy, Debug)]
pub struct FixtureSpec {
    pub dir_name: &'static str,
    pub namespace: &'static str,
    pub crate_name: &'static str,
    pub udl_file: &'static str,
}

impl FixtureSpec {
    pub fn package_name(&self) -> String {
        format!("{}-package", self.namespace)
    }

    pub fn generated_binding_relative_paths(&self) -> Vec<String> {
        let mut files = vec![
            "index.js".to_string(),
            "index.d.ts".to_string(),
            format!("{}.js", self.namespace),
            format!("{}.d.ts", self.namespace),
            format!("{}-ffi.js", self.namespace),
            format!("{}-ffi.d.ts", self.namespace),
        ];
        files.extend(RUNTIME_FILES.iter().map(|path| (*path).to_string()));
        files
    }

    pub fn generated_package_relative_paths(&self) -> Vec<String> {
        let mut files = vec!["package.json".to_string()];
        files.extend(self.generated_binding_relative_paths());
        files
    }
}

pub fn fixture_spec(name: &str) -> FixtureSpec {
    match name {
        "basic" => FixtureSpec {
            dir_name: "basic-fixture",
            namespace: "fixture",
            crate_name: "fixture_basic",
            udl_file: "fixture.udl",
        },
        "callbacks" => FixtureSpec {
            dir_name: "callback-fixture",
            namespace: "callbacks_fixture",
            crate_name: "fixture_callbacks",
            udl_file: "callbacks_fixture.udl",
        },
        _ => panic!("unknown fixture '{name}'"),
    }
}
