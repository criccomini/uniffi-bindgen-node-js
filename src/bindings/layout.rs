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
    pub(crate) fn from_component(
        out_dir: &Utf8Path,
        lib_source: &Utf8Path,
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
            component.config.bundled_prebuilds,
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

    pub(crate) fn ensure_root_dir(&self) -> Result<()> {
        fs::create_dir_all(self.root_dir.as_std_path())?;
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

fn current_host_prebuild_target() -> Result<String> {
    let platform = current_node_platform()?;
    let arch = current_node_arch()?;

    if platform != "linux" {
        return Ok(format!("{platform}-{arch}"));
    }

    Ok(format!("{platform}-{arch}-{}", current_linux_libc()?))
}

fn current_node_platform() -> Result<&'static str> {
    match std::env::consts::OS {
        "macos" => Ok("darwin"),
        "windows" => Ok("win32"),
        "linux" => Ok("linux"),
        "android" => Ok("android"),
        "aix" => Ok("aix"),
        "freebsd" => Ok("freebsd"),
        "openbsd" => Ok("openbsd"),
        other => bail!("unsupported host OS for Node bundled-prebuild staging: {other}"),
    }
}

fn current_node_arch() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("x64"),
        "x86" => Ok("ia32"),
        "aarch64" => Ok("arm64"),
        "arm" => Ok("arm"),
        "loongarch64" => Ok("loong64"),
        "powerpc64" => Ok("ppc64"),
        "riscv64" => Ok("riscv64"),
        "s390x" => Ok("s390x"),
        other => bail!("unsupported host architecture for Node bundled-prebuild staging: {other}"),
    }
}

fn current_linux_libc() -> Result<&'static str> {
    if cfg!(target_env = "gnu") {
        Ok("gnu")
    } else if cfg!(target_env = "musl") {
        Ok("musl")
    } else {
        bail!("unsupported Linux target environment for Node bundled-prebuild staging")
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

    fn fixture_library_path(file_name: &str) -> Utf8PathBuf {
        Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout-input").join(file_name)
    }

    #[test]
    fn generated_package_layout_resolves_output_paths_from_out_dir_and_namespace() {
        let out_dir = Utf8PathBuf::from("/tmp/uniffi-bindgen-node-js-layout");
        let lib_source = fixture_library_path("libexample.dylib");
        let component = component_with_namespace("example");

        let layout = GeneratedPackageLayout::from_component(&out_dir, &lib_source, &component)
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

        let layout = GeneratedPackageLayout::from_component(&out_dir, &lib_source, &component)
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
        component.config.bundled_prebuilds = true;

        let layout = GeneratedPackageLayout::from_component(&out_dir, &lib_source, &component)
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
}
