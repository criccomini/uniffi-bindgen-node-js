use std::fs;

use anyhow::{Result, anyhow, bail};
use camino::{Utf8Path, Utf8PathBuf};
use uniffi_bindgen::Component;

use crate::node_v2::config::NodeBindingGeneratorConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedPackageLayout {
    pub(crate) root_dir: Utf8PathBuf,
    pub(crate) namespace: String,
    pub(crate) package_name: String,
}

impl GeneratedPackageLayout {
    pub(crate) fn from_component(
        out_dir: &Utf8Path,
        component: &Component<NodeBindingGeneratorConfig>,
    ) -> Result<Self> {
        let namespace = component.ci.namespace().trim();
        if namespace.is_empty() {
            bail!("node bindings generation requires a non-empty UniFFI namespace");
        }

        let package_name = component
            .config
            .package_name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("node bindings generation requires a package_name"))?;

        Ok(Self {
            root_dir: out_dir.to_path_buf(),
            namespace: namespace.to_string(),
            package_name: package_name.to_string(),
        })
    }

    pub(crate) fn ensure_root_dir(&self) -> Result<()> {
        fs::create_dir_all(self.root_dir.as_std_path())?;
        Ok(())
    }

    pub(crate) fn staged_native_library_path(
        &self,
        library_filename: &str,
        bundled_prebuild_target: Option<&str>,
    ) -> Utf8PathBuf {
        match bundled_prebuild_target {
            Some(target) => self
                .root_dir
                .join("prebuilds")
                .join(target)
                .join(library_filename),
            None => self.root_dir.join(library_filename),
        }
    }

    pub(crate) fn package_json_path(&self) -> Utf8PathBuf {
        self.root_dir.join("package.json")
    }

    pub(crate) fn index_js_path(&self) -> Utf8PathBuf {
        self.root_dir.join("index.js")
    }

    pub(crate) fn index_dts_path(&self) -> Utf8PathBuf {
        self.root_dir.join("index.d.ts")
    }

    pub(crate) fn component_js_path(&self) -> Utf8PathBuf {
        self.root_dir.join(format!("{}.js", self.namespace))
    }

    pub(crate) fn component_dts_path(&self) -> Utf8PathBuf {
        self.root_dir.join(format!("{}.d.ts", self.namespace))
    }

    pub(crate) fn component_ffi_js_path(&self) -> Utf8PathBuf {
        self.root_dir.join(format!("{}-ffi.js", self.namespace))
    }

    pub(crate) fn component_ffi_dts_path(&self) -> Utf8PathBuf {
        self.root_dir.join(format!("{}-ffi.d.ts", self.namespace))
    }

    pub(crate) fn runtime_path(&self, file_name: &str) -> Utf8PathBuf {
        self.root_dir.join("runtime").join(file_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use uniffi_bindgen::interface::ComponentInterface;

    fn component_with_namespace(namespace: &str) -> Component<NodeBindingGeneratorConfig> {
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

    #[test]
    fn generated_package_layout_resolves_output_paths_from_out_dir_and_namespace() {
        let out_dir = Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout");
        let component = component_with_namespace("example");

        let layout = GeneratedPackageLayout::from_component(&out_dir, &component).expect("layout");

        assert_eq!(layout.root_dir, out_dir);
        assert_eq!(
            layout.package_json_path(),
            layout.root_dir.join("package.json")
        );
        assert_eq!(layout.index_js_path(), layout.root_dir.join("index.js"));
        assert_eq!(layout.index_dts_path(), layout.root_dir.join("index.d.ts"));
        assert_eq!(
            layout.component_js_path(),
            layout.root_dir.join("example.js")
        );
        assert_eq!(
            layout.component_dts_path(),
            layout.root_dir.join("example.d.ts")
        );
        assert_eq!(
            layout.component_ffi_js_path(),
            layout.root_dir.join("example-ffi.js")
        );
        assert_eq!(
            layout.component_ffi_dts_path(),
            layout.root_dir.join("example-ffi.d.ts")
        );
        assert_eq!(
            layout.runtime_path("errors.js"),
            layout.root_dir.join("runtime/errors.js")
        );
    }

    #[test]
    fn generated_package_layout_computes_staged_native_library_paths() {
        let layout = GeneratedPackageLayout {
            root_dir: Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout"),
            namespace: "example".to_string(),
            package_name: "example-package".to_string(),
        };

        assert_eq!(
            layout.staged_native_library_path("libexample.dylib", None),
            layout.root_dir.join("libexample.dylib")
        );
        assert_eq!(
            layout.staged_native_library_path("libexample.dylib", Some("darwin-aarch64")),
            layout
                .root_dir
                .join("prebuilds")
                .join("darwin-aarch64")
                .join("libexample.dylib")
        );
    }
}
