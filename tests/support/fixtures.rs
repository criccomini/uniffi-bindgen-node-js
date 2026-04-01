#![allow(dead_code)]

#[derive(Clone, Copy, Debug)]
pub struct FixtureSpec {
    pub dir_name: &'static str,
    pub namespace: &'static str,
    pub crate_name: &'static str,
    pub udl_file: &'static str,
}

impl FixtureSpec {
    pub fn runtime_relative_paths(&self) -> Vec<String> {
        let module_stems = match self.dir_name {
            "basic-fixture" => vec![
                "errors",
                "ffi-types",
                "ffi-converters",
                "rust-call",
                "async-rust-call",
                "handle-map",
                "callbacks",
                "objects",
            ],
            "callback-fixture" => vec![
                "errors",
                "ffi-types",
                "ffi-converters",
                "rust-call",
                "async-rust-call",
                "handle-map",
                "callbacks",
                "objects",
            ],
            "docs-fixture" => vec![
                "errors",
                "ffi-types",
                "ffi-converters",
                "rust-call",
                "handle-map",
                "callbacks",
                "objects",
            ],
            _ => panic!("unknown fixture runtime file set for '{}'", self.dir_name),
        };

        module_stems
            .into_iter()
            .flat_map(|stem| [format!("runtime/{stem}.js"), format!("runtime/{stem}.d.ts")])
            .collect()
    }

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
        files.extend(self.runtime_relative_paths());
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
        "docs" => FixtureSpec {
            dir_name: "docs-fixture",
            namespace: "docs_fixture",
            crate_name: "fixture_docs",
            udl_file: "docs_fixture.udl",
        },
        _ => panic!("unknown fixture '{name}'"),
    }
}
