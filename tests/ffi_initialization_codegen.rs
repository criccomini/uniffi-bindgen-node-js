mod support;

use insta::assert_snapshot;
use uniffi_bindgen::BindingGenerator;
use uniffi_bindgen_node_js::bindings::NodeBindingGeneratorConfig;

use self::support::{
    component_from_webidl, generation_settings, generator, read_generated_file, remove_dir_all,
};

fn extract_section(contents: &str, start_marker: &str, end_marker: &str) -> String {
    let start = contents
        .find(start_marker)
        .unwrap_or_else(|| panic!("missing start marker {start_marker:?}"));
    let end = contents[start..]
        .find(end_marker)
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("missing end marker {end_marker:?}"));
    contents[start..end].trim().to_string()
}

fn normalize_checksum_value(contents: &str) -> String {
    contents
        .lines()
        .map(|line| {
            if line
                .trim_start()
                .starts_with("\"uniffi_fixture_crate_checksum_func_current_generation\": ")
            {
                "    \"uniffi_fixture_crate_checksum_func_current_generation\": <CHECKSUM>,"
                    .to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_contains_in_order(contents: &str, snippets: &[&str]) {
    let mut search_start = 0;

    for snippet in snippets {
        let relative_offset = contents[search_start..]
            .find(snippet)
            .unwrap_or_else(|| panic!("missing ordered snippet {snippet:?}"));
        search_start += relative_offset + snippet.len();
    }
}

fn generate_simple_component_files(
    configure: impl FnOnce(&mut NodeBindingGeneratorConfig),
) -> (String, String, String) {
    let generator = generator();
    let settings = generation_settings("ffi-initialization");
    let output_dir = settings.out_dir.clone();
    let mut component = component_from_webidl(
        r#"
        namespace example {
            u64 current_generation();
        };
        "#,
    );
    configure(&mut component.config);

    generator
        .write_bindings(&settings, &[component])
        .expect("write_bindings should succeed");

    let component_js = read_generated_file(&output_dir, "example.js");
    let ffi_js = read_generated_file(&output_dir, "example-ffi.js");
    let ffi_dts = read_generated_file(&output_dir, "example-ffi.d.ts");
    remove_dir_all(&output_dir);

    (component_js, ffi_js, ffi_dts)
}

fn generate_simple_component_ffi() -> (String, String) {
    let (_, ffi_js, ffi_dts) = generate_simple_component_files(|_| {});
    (ffi_js, ffi_dts)
}

#[test]
fn generated_ffi_js_snapshots_contract_and_checksum_initialization() {
    let (ffi_js, _) = generate_simple_component_ffi();
    let metadata_section = normalize_checksum_value(&extract_section(
        &ffi_js,
        "export const ffiMetadata = Object.freeze({",
        "function createBindingCore(",
    ));
    let lifecycle_section = extract_section(
        &ffi_js,
        "export function load(libraryPath = undefined) {",
        "export const ffiFunctions = Object.freeze({",
    );

    assert_snapshot!(
        format!("=== metadata ===\n{metadata_section}\n\n=== lifecycle ===\n{lifecycle_section}"),
        @r#"
    === metadata ===
    export const ffiMetadata = Object.freeze({
      namespace: "example",
      cdylibName: "fixture",
      libPathLiteral: null,
      bundledPrebuilds: false,
      manualLoad: false,
    });

    export const ffiIntegrity = Object.freeze({
      contractVersionFunction: "ffi_fixture_crate_uniffi_contract_version",
      expectedContractVersion: 29,
      checksums: Object.freeze({

        "uniffi_fixture_crate_checksum_func_current_generation": <CHECKSUM>,

      }),
    });

    let loadedBindings = null;
    let loadedFfiTypes = null;
    let loadedFfiFunctions = null;
    // Koffi retains native state for repeated lib.func() declarations, so keep a
    // single binding core alive across unload/load cycles and evict stale cores
    // when switching to a different canonical library path.
    let cachedBindingCore = null;
    let cachedLibraryPath = null;
    let runtimeHooks = Object.freeze({});
    const moduleFilename = fileURLToPath(import.meta.url);
    const moduleDirectory = dirname(moduleFilename);
    const libraryNotLoadedMessage =
      "The native library is not loaded. Call load(libraryPath) first.";

    function defaultSiblingLibraryFilename() {
      const extensionByPlatform = {
        darwin: ".dylib",
        linux: ".so",
        win32: ".dll",
      };
      const extension = extensionByPlatform[process.platform] ?? ".so";

      if (process.platform === "win32") {
        return ffiMetadata.cdylibName.endsWith(extension)
          ? ffiMetadata.cdylibName
          : `${ffiMetadata.cdylibName}${extension}`;
      }

      const libraryBaseName = ffiMetadata.cdylibName.startsWith("lib")
        ? ffiMetadata.cdylibName
        : `lib${ffiMetadata.cdylibName}`;
      return libraryBaseName.endsWith(extension)
        ? libraryBaseName
        : `${libraryBaseName}${extension}`;
    }

    function defaultBundledTarget() {
      if (process.platform !== "linux") {
        return `${process.platform}-${process.arch}`;
      }

      const glibcVersionRuntime =
        process.report?.getReport?.().header?.glibcVersionRuntime;
      const linuxLibc = glibcVersionRuntime == null ? "musl" : "gnu";
      return `${process.platform}-${process.arch}-${linuxLibc}`;
    }

    function defaultBundledLibrary() {
      const target = defaultBundledTarget();
      const filename = defaultSiblingLibraryFilename();
      return Object.freeze({
        target,
        packageRelativePath: `prebuilds/${target}/${filename}`,
        libraryPath: join(moduleDirectory, "prebuilds", target, filename),
      });
    }

    function defaultSiblingLibraryPath() {
      return join(moduleDirectory, defaultSiblingLibraryFilename());
    }

    function resolveLibraryPath(libraryPath = undefined) {
      const rawLibraryPath = libraryPath ?? ffiMetadata.libPathLiteral;
      if (rawLibraryPath != null) {
        return Object.freeze({
          libraryPath: isAbsolute(rawLibraryPath)
            ? rawLibraryPath
            : join(moduleDirectory, rawLibraryPath),
          bundledPrebuild: null,
        });
      }

      if (ffiMetadata.bundledPrebuilds) {
        const bundledPrebuild = defaultBundledLibrary();
        return Object.freeze({
          libraryPath: bundledPrebuild.libraryPath,
          bundledPrebuild,
        });
      }

      return Object.freeze({
        libraryPath: defaultSiblingLibraryPath(),
        bundledPrebuild: null,
      });
    }

    function canonicalizeExistingLibraryPath(libraryPath) {
      if (!existsSync(libraryPath)) {
        return libraryPath;
      }

      return typeof realpathSync.native === "function"
        ? realpathSync.native(libraryPath)
        : realpathSync(libraryPath);
    }

    === lifecycle ===
    export function load(libraryPath = undefined) {
      const resolution = resolveLibraryPath(libraryPath);
      const resolvedLibraryPath = resolution.libraryPath;
      const bundledPrebuild = resolution.bundledPrebuild;
      const canonicalLibraryPath = canonicalizeExistingLibraryPath(resolvedLibraryPath);

      if (loadedBindings !== null) {
        if (loadedBindings.libraryPath === canonicalLibraryPath) {
          return loadedBindings;
        }

        throw new Error(
          `The native library is already loaded from ${JSON.stringify(loadedBindings.libraryPath)}. Call unload() before loading a different library path.`,
        );
      }

      if (bundledPrebuild !== null && !existsSync(resolvedLibraryPath)) {
        throw new Error(
          `No bundled UniFFI library was found for target ${JSON.stringify(bundledPrebuild.target)}. Expected ${JSON.stringify(bundledPrebuild.packageRelativePath)} inside the generated package.`,
        );
      }

      let bindingCore =
        cachedLibraryPath === canonicalLibraryPath
          ? cachedBindingCore
          : null;
      if (bindingCore == null && cachedBindingCore != null) {
        cachedBindingCore.library.unload();
        clearBindingCoreCache();
      }

      const bindings = createBindings(canonicalLibraryPath, bindingCore);
      try {
        runtimeHooks.onLoad?.(bindings);
        if (bindingCore == null) {
          validateContractVersion(bindings);
          validateChecksums(bindings);
          bindingCore = cacheBindingCore(canonicalLibraryPath, bindings);
        }
      } catch (error) {
        try {
          runtimeHooks.onUnload?.(bindings);
        } catch {
          // Preserve the original initialization failure.
        }
        if (bindingCore == null) {
          try {
            bindings.library.unload();
          } catch {
            // Preserve the original initialization failure.
          }
        }
        throw error;
      }

      loadedBindings = bindings;
      loadedFfiTypes = bindings.ffiTypes;
      loadedFfiFunctions = bindings.ffiFunctions;
      return loadedBindings;
    }

    export function unload() {
      if (loadedBindings === null) {
        return false;
      }

      let hookError = null;
      try {
        runtimeHooks.onUnload?.(loadedBindings);
      } catch (error) {
        hookError = error;
      }
      loadedBindings = null;
      loadedFfiTypes = null;
      loadedFfiFunctions = null;
      if (hookError != null) {
        throw hookError;
      }
      return true;
    }

    export function isLoaded() {
      return loadedBindings !== null;
    }

    export function configureRuntimeHooks(hooks = undefined) {
      runtimeHooks = Object.freeze(hooks ?? {});
    }


    if (!ffiMetadata.manualLoad) {
      load();
    }


    function throwLibraryNotLoaded() {
      throw new LibraryNotLoadedError(libraryNotLoadedMessage);
    }

    export function getFfiBindings() {
      if (loadedBindings === null) {
        throwLibraryNotLoaded();
      }

      return loadedBindings;
    }

    export function getFfiTypes() {
      if (loadedFfiTypes === null) {
        throwLibraryNotLoaded();
      }

      return loadedFfiTypes;
    }

    function getLoadedFfiFunctions() {
      if (loadedFfiFunctions === null) {
        throwLibraryNotLoaded();
      }

      return loadedFfiFunctions;
    }

    export function getContractVersion(bindings = getFfiBindings()) {
      return bindings.ffiFunctions.ffi_fixture_crate_uniffi_contract_version();
    }

    export function validateContractVersion(bindings = getFfiBindings()) {
      const actual = getContractVersion(bindings);
      const expected = ffiIntegrity.expectedContractVersion;
      if (actual !== expected) {
        throw new ContractVersionMismatchError(expected, actual);
      }
      return actual;
    }

    export function getChecksums(bindings = getFfiBindings()) {
      return Object.freeze({

        "uniffi_fixture_crate_checksum_func_current_generation": bindings.ffiFunctions.uniffi_fixture_crate_checksum_func_current_generation(),

      });
    }

    export function validateChecksums(bindings = getFfiBindings()) {
      const actualChecksums = getChecksums(bindings);

      {
        const expected = ffiIntegrity.checksums["uniffi_fixture_crate_checksum_func_current_generation"];
        const actual = actualChecksums["uniffi_fixture_crate_checksum_func_current_generation"];
        if (actual !== expected) {
          throw new ChecksumMismatchError("uniffi_fixture_crate_checksum_func_current_generation", expected, actual);
        }
      }

      return actualChecksums;
    }
    "#
    );
}

#[test]
fn generated_ffi_dts_snapshots_initialization_contract() {
    let (_, ffi_dts) = generate_simple_component_ffi();
    let initialization_contract = extract_section(
        &ffi_dts,
        "export interface FfiMetadata {",
        "export declare const ffiFunctions: Readonly<Record<string, (...args: any[]) => any>>;",
    );

    assert_snapshot!(
        initialization_contract,
        @r#"
    export interface FfiMetadata {
      namespace: string;
      cdylibName: string;
      libPathLiteral: string | null;
      bundledPrebuilds: boolean;
      manualLoad: boolean;
    }

    export interface FfiBindings {
      libraryPath: string;
      library: unknown;
      ffiTypes: Readonly<Record<string, unknown>>;
      ffiCallbacks: Readonly<Record<string, unknown>>;
      ffiStructs: Readonly<Record<string, unknown>>;
      ffiFunctions: Readonly<Record<string, (...args: any[]) => any>>;
    }

    export interface FfiIntegrity {
      contractVersionFunction: string;
      expectedContractVersion: number;
      checksums: Readonly<Record<string, number>>;
    }

    export interface FfiRuntimeHooks {
      onLoad?(bindings: Readonly<FfiBindings>): void;
      onUnload?(bindings: Readonly<FfiBindings>): void;
    }

    export declare const ffiMetadata: Readonly<FfiMetadata>;
    export declare const ffiIntegrity: Readonly<FfiIntegrity>;
    export declare function configureRuntimeHooks(hooks?: FfiRuntimeHooks | null): void;
    export declare function load(libraryPath?: string | null): Readonly<FfiBindings>;
    export declare function unload(): boolean;
    export declare function isLoaded(): boolean;
    export declare function getFfiBindings(): Readonly<FfiBindings>;
    export declare function getFfiTypes(): Readonly<Record<string, unknown>>;
    export declare function getContractVersion(bindings?: Readonly<FfiBindings>): number;
    export declare function validateContractVersion(bindings?: Readonly<FfiBindings>): number;
    export declare function getChecksums(
      bindings?: Readonly<FfiBindings>,
    ): Readonly<Record<string, number>>;
    export declare function validateChecksums(
      bindings?: Readonly<FfiBindings>,
    ): Readonly<Record<string, number>>;
    "#
    );
}

#[test]
fn generated_bundled_ffi_js_emits_bundled_resolution_contract() {
    let (component_js, ffi_js, _) = generate_simple_component_files(|config| {
        config.bundled_prebuilds = true;
    });
    let component_metadata = extract_section(
        &component_js,
        "export const componentMetadata = Object.freeze({",
        "export { ffiMetadata } from",
    );
    let metadata_and_resolution = extract_section(
        &ffi_js,
        "export const ffiMetadata = Object.freeze({",
        "function createBindingCore(",
    );
    let lifecycle_section = extract_section(
        &ffi_js,
        "export function load(libraryPath = undefined) {",
        "export const ffiFunctions = Object.freeze({",
    );

    assert!(
        component_metadata.contains("bundledPrebuilds: true"),
        "component metadata should expose bundledPrebuilds:\n{component_metadata}"
    );
    assert!(
        metadata_and_resolution.contains("bundledPrebuilds: true"),
        "ffi metadata should expose bundledPrebuilds:\n{metadata_and_resolution}"
    );
    assert!(
        metadata_and_resolution.contains("function defaultBundledTarget()"),
        "bundled target helper should be emitted:\n{metadata_and_resolution}"
    );
    assert!(
        metadata_and_resolution.contains(
            "const glibcVersionRuntime =\n    process.report?.getReport?.().header?.glibcVersionRuntime;"
        ),
        "linux libc detection should use process.report:\n{metadata_and_resolution}"
    );
    assert!(
        metadata_and_resolution
            .contains("const linuxLibc = glibcVersionRuntime == null ? \"musl\" : \"gnu\";"),
        "linux libc suffix should distinguish musl and gnu:\n{metadata_and_resolution}"
    );
    assert!(
        metadata_and_resolution.contains("packageRelativePath: `prebuilds/${target}/${filename}`,"),
        "bundled libraries should resolve under prebuilds/<target>/<filename>:\n{metadata_and_resolution}"
    );
    assert_contains_in_order(
        &metadata_and_resolution,
        &[
            "const rawLibraryPath = libraryPath ?? ffiMetadata.libPathLiteral;",
            "if (rawLibraryPath != null) {",
            "if (ffiMetadata.bundledPrebuilds) {",
            "libraryPath: defaultSiblingLibraryPath(),",
        ],
    );
    assert_contains_in_order(
        &lifecycle_section,
        &[
            "if (bundledPrebuild !== null && !existsSync(resolvedLibraryPath)) {",
            "No bundled UniFFI library was found for target ${JSON.stringify(bundledPrebuild.target)}.",
            "Expected ${JSON.stringify(bundledPrebuild.packageRelativePath)} inside the generated package.",
            "let bindingCore =",
            "const bindings = createBindings(canonicalLibraryPath, bindingCore);",
        ],
    );
}
