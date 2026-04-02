use std::fs;

use anyhow::{Result, anyhow, bail};
use camino::{Utf8Path, Utf8PathBuf};
use uniffi_bindgen::interface::ComponentInterface;

use super::{spec::NodePackageSpec, target::current_host_prebuild_target};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GeneratedPackageLayout {
    pub(crate) root_dir: Utf8PathBuf,
    pub(crate) namespace: String,
    pub(crate) package_name: String,
    pub(crate) metadata: PackageMetadataLayout,
    pub(crate) namespace_api: NamespaceApiLayout,
    pub(crate) ffi: ComponentFfiLayout,
    pub(crate) runtime: RuntimeLayout,
    pub(crate) native_library: StagedNativeLibraryLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageMetadataLayout {
    pub(crate) package_json: Utf8PathBuf,
    pub(crate) index_js: Utf8PathBuf,
    pub(crate) index_dts: Utf8PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NamespaceApiLayout {
    pub(crate) js: Utf8PathBuf,
    pub(crate) dts: Utf8PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ComponentFfiLayout {
    pub(crate) js: Utf8PathBuf,
    pub(crate) dts: Utf8PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeLayout {
    pub(crate) dir: Utf8PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StagedNativeLibraryLayout {
    pub(crate) source_path: Utf8PathBuf,
    pub(crate) file_name: String,
    pub(crate) package_relative_path: Utf8PathBuf,
    pub(crate) output_path: Utf8PathBuf,
    pub(crate) bundled_prebuild_target: Option<String>,
}

impl GeneratedPackageLayout {
    pub(crate) fn from_parts(
        out_dir: &Utf8Path,
        lib_source: &Utf8Path,
        ci: &ComponentInterface,
        package_spec: &NodePackageSpec,
    ) -> Result<Self> {
        let namespace = ci.namespace().trim();
        if namespace.is_empty() {
            bail!("node bindings generation requires a non-empty UniFFI namespace");
        }

        let package_name = package_spec.package_name.trim();
        if package_name.is_empty() {
            return Err(anyhow!("node bindings generation requires a package_name"));
        }

        let metadata = PackageMetadataLayout {
            package_json: out_dir.join("package.json"),
            index_js: out_dir.join("index.js"),
            index_dts: out_dir.join("index.d.ts"),
        };
        let namespace_api = NamespaceApiLayout {
            js: out_dir.join(format!("{namespace}.js")),
            dts: out_dir.join(format!("{namespace}.d.ts")),
        };
        let ffi = ComponentFfiLayout {
            js: out_dir.join(format!("{namespace}-ffi.js")),
            dts: out_dir.join(format!("{namespace}-ffi.d.ts")),
        };
        let runtime = RuntimeLayout {
            dir: out_dir.join("runtime"),
        };
        let native_library = StagedNativeLibraryLayout::from_source(
            out_dir,
            lib_source,
            package_spec.bundled_prebuilds,
        )?;

        Ok(Self {
            root_dir: out_dir.to_path_buf(),
            namespace: namespace.to_string(),
            package_name: package_name.to_string(),
            metadata,
            namespace_api,
            ffi,
            runtime,
            native_library,
        })
    }

    pub(crate) fn ensure_output_dirs(&self) -> Result<()> {
        for directory in self.output_directories() {
            fs::create_dir_all(directory.as_std_path())?;
        }
        Ok(())
    }

    #[cfg(test)]
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
        self.metadata.package_json.clone()
    }

    pub(crate) fn index_js_path(&self) -> Utf8PathBuf {
        self.metadata.index_js.clone()
    }

    pub(crate) fn index_dts_path(&self) -> Utf8PathBuf {
        self.metadata.index_dts.clone()
    }

    pub(crate) fn component_js_path(&self) -> Utf8PathBuf {
        self.namespace_api.js.clone()
    }

    pub(crate) fn component_dts_path(&self) -> Utf8PathBuf {
        self.namespace_api.dts.clone()
    }

    pub(crate) fn component_ffi_js_path(&self) -> Utf8PathBuf {
        self.ffi.js.clone()
    }

    pub(crate) fn component_ffi_dts_path(&self) -> Utf8PathBuf {
        self.ffi.dts.clone()
    }

    pub(crate) fn runtime_path(&self, file_name: &str) -> Utf8PathBuf {
        self.runtime.dir.join(file_name)
    }

    fn output_directories(&self) -> Vec<Utf8PathBuf> {
        let mut directories = vec![self.root_dir.clone(), self.runtime.dir.clone()];
        if let Some(native_library_dir) = self.native_library.output_path.parent()
            && native_library_dir != self.root_dir
        {
            directories.push(native_library_dir.to_path_buf());
        }
        directories
    }
}

impl StagedNativeLibraryLayout {
    fn from_source(
        out_dir: &Utf8Path,
        lib_source: &Utf8Path,
        bundled_prebuilds: bool,
    ) -> Result<Self> {
        let file_name = lib_source
            .file_name()
            .ok_or_else(|| anyhow!("built UniFFI cdylib '{}' has no filename", lib_source))?
            .to_string();

        let bundled_prebuild_target = bundled_prebuilds
            .then(current_host_prebuild_target)
            .transpose()?;
        let package_relative_path = match bundled_prebuild_target.as_deref() {
            Some(target) => Utf8PathBuf::from("prebuilds").join(target).join(&file_name),
            None => Utf8PathBuf::from(&file_name),
        };

        Ok(Self {
            source_path: lib_source.to_path_buf(),
            file_name: file_name.clone(),
            output_path: out_dir.join(&package_relative_path),
            package_relative_path,
            bundled_prebuild_target,
        })
    }
}

// GENERATED CODE
#[cfg(test)]
mod tests {
    use super::*;

    use uniffi_bindgen::interface::ComponentInterface;

    // Layout tests stay synthetic because they only validate package path derivation. The
    // integration suite covers loader-driven package generation end to end.

    struct TestLayoutInput {
        ci: ComponentInterface,
        spec: NodePackageSpec,
    }

    fn component_with_namespace(namespace: &str) -> TestLayoutInput {
        TestLayoutInput {
            ci: ComponentInterface::from_webidl(
                &format!("namespace {namespace} {{}};"),
                "fixture_crate",
            )
            .expect("valid test UDL"),
            spec: NodePackageSpec {
                package_name: format!("{namespace}-package"),
                library_name: "fixture".to_string(),
                node_engine: ">=16".to_string(),
                bundled_prebuilds: false,
                manual_load: false,
            },
        }
    }

    fn fixture_library_path(file_name: &str) -> Utf8PathBuf {
        Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout-input").join(file_name)
    }

    #[test]
    fn generated_package_layout_resolves_output_paths_from_out_dir_and_namespace() {
        let out_dir = Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout");
        let lib_source = fixture_library_path("libexample.dylib");
        let component = component_with_namespace("example");

        let layout = GeneratedPackageLayout::from_parts(
            &out_dir,
            &lib_source,
            &component.ci,
            &component.spec,
        )
        .expect("layout");

        assert_eq!(layout.root_dir, out_dir);
        assert_eq!(
            layout.metadata.package_json,
            layout.root_dir.join("package.json")
        );
        assert_eq!(layout.metadata.index_js, layout.root_dir.join("index.js"));
        assert_eq!(
            layout.metadata.index_dts,
            layout.root_dir.join("index.d.ts")
        );
        assert_eq!(layout.namespace_api.js, layout.root_dir.join("example.js"));
        assert_eq!(
            layout.namespace_api.dts,
            layout.root_dir.join("example.d.ts")
        );
        assert_eq!(layout.ffi.js, layout.root_dir.join("example-ffi.js"));
        assert_eq!(layout.ffi.dts, layout.root_dir.join("example-ffi.d.ts"));
        assert_eq!(layout.runtime.dir, layout.root_dir.join("runtime"));
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
        assert_eq!(layout.native_library.source_path, lib_source);
        assert_eq!(layout.native_library.file_name, "libexample.dylib");
        assert_eq!(
            layout.native_library.package_relative_path,
            Utf8PathBuf::from("libexample.dylib")
        );
        assert_eq!(
            layout.native_library.output_path,
            layout.root_dir.join("libexample.dylib")
        );
        assert_eq!(layout.native_library.bundled_prebuild_target, None);
    }

    #[test]
    fn generated_package_layout_computes_staged_native_library_paths() {
        let layout = GeneratedPackageLayout {
            root_dir: Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout"),
            namespace: "example".to_string(),
            package_name: "example-package".to_string(),
            metadata: PackageMetadataLayout {
                package_json: Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout/package.json"),
                index_js: Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout/index.js"),
                index_dts: Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout/index.d.ts"),
            },
            namespace_api: NamespaceApiLayout {
                js: Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout/example.js"),
                dts: Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout/example.d.ts"),
            },
            ffi: ComponentFfiLayout {
                js: Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout/example-ffi.js"),
                dts: Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout/example-ffi.d.ts"),
            },
            runtime: RuntimeLayout {
                dir: Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout/runtime"),
            },
            native_library: StagedNativeLibraryLayout {
                source_path: Utf8PathBuf::from("/tmp/input/libexample.dylib"),
                file_name: "libexample.dylib".to_string(),
                package_relative_path: Utf8PathBuf::from("libexample.dylib"),
                output_path: Utf8PathBuf::from(
                    "/tmp/uniffi-bindgen-node-js-layout/libexample.dylib",
                ),
                bundled_prebuild_target: None,
            },
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

    #[test]
    fn generated_package_layout_uses_the_input_filename_for_root_staging() {
        let out_dir = Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout");
        let lib_source = fixture_library_path("fixture.dll");
        let component = component_with_namespace("example");

        let layout = GeneratedPackageLayout::from_parts(
            &out_dir,
            &lib_source,
            &component.ci,
            &component.spec,
        )
        .expect("layout");

        assert_eq!(layout.native_library.file_name, "fixture.dll");
        assert_eq!(
            layout.native_library.package_relative_path,
            Utf8PathBuf::from("fixture.dll")
        );
        assert_eq!(
            layout.native_library.output_path,
            out_dir.join("fixture.dll")
        );
        assert_eq!(layout.native_library.bundled_prebuild_target, None);
    }

    #[test]
    fn generated_package_layout_uses_the_host_target_for_bundled_prebuild_staging() {
        let out_dir = Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout");
        let lib_source = fixture_library_path("libexample.so");
        let mut component = component_with_namespace("example");
        component.spec.bundled_prebuilds = true;

        let layout = GeneratedPackageLayout::from_parts(
            &out_dir,
            &lib_source,
            &component.ci,
            &component.spec,
        )
        .expect("layout");

        let target = current_host_prebuild_target().expect("host target");
        assert_eq!(layout.native_library.file_name, "libexample.so");
        assert_eq!(
            layout.native_library.bundled_prebuild_target.as_deref(),
            Some(target.as_str())
        );
        assert_eq!(
            layout.native_library.package_relative_path,
            Utf8PathBuf::from("prebuilds")
                .join(&target)
                .join("libexample.so")
        );
        assert_eq!(
            layout.native_library.output_path,
            out_dir
                .join("prebuilds")
                .join(&target)
                .join("libexample.so")
        );
    }

    #[test]
    fn generated_package_layout_creates_the_runtime_and_native_output_directories() {
        let out_dir = Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout-created-dirs");
        let lib_source = fixture_library_path("libexample.so");
        let mut component = component_with_namespace("example");
        component.spec.bundled_prebuilds = true;
        let layout = GeneratedPackageLayout::from_parts(
            &out_dir,
            &lib_source,
            &component.ci,
            &component.spec,
        )
        .expect("layout");

        if out_dir.exists() {
            fs::remove_dir_all(out_dir.as_std_path()).expect("cleanup pre-existing temp dir");
        }

        layout
            .ensure_output_dirs()
            .expect("expected package directories to be created");

        assert!(
            layout.root_dir.is_dir(),
            "expected package root at {}",
            layout.root_dir
        );
        assert!(
            layout.runtime.dir.is_dir(),
            "expected runtime directory at {}",
            layout.runtime.dir
        );
        let native_parent = layout
            .native_library
            .output_path
            .parent()
            .expect("bundled native library should have a parent");
        assert!(
            native_parent.is_dir(),
            "expected native library directory at {native_parent}"
        );

        fs::remove_dir_all(out_dir.as_std_path()).expect("cleanup temp dir");
    }
}
