mod support;

use insta::assert_snapshot;
use uniffi_bindgen::BindingGenerator;

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

fn generate_simple_component_ffi() -> (String, String) {
    let generator = generator();
    let settings = generation_settings("ffi-initialization");
    let output_dir = settings.out_dir.clone();
    let component = component_from_webidl(
        r#"
        namespace example {
            u64 current_generation();
        };
        "#,
    );

    generator
        .write_bindings(&settings, &[component])
        .expect("write_bindings should succeed");

    let ffi_js = read_generated_file(&output_dir, "example-ffi.js");
    let ffi_dts = read_generated_file(&output_dir, "example-ffi.d.ts");
    remove_dir_all(&output_dir);

    (ffi_js, ffi_dts)
}

#[test]
fn generated_ffi_js_snapshots_contract_and_checksum_initialization() {
    let (ffi_js, _) = generate_simple_component_ffi();
    let metadata_section = normalize_checksum_value(&extract_section(
        &ffi_js,
        "export const ffiMetadata = Object.freeze({",
        "function createBindings(",
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
    let runtimeHooks = Object.freeze({});
    const moduleFilename = fileURLToPath(import.meta.url);
    const moduleDirectory = dirname(moduleFilename);

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

    function defaultSiblingLibraryPath() {
      return join(moduleDirectory, defaultSiblingLibraryFilename());
    }

    function resolveLibraryPath(libraryPath = undefined) {
      const rawLibraryPath = libraryPath ?? ffiMetadata.libPathLiteral;
      if (rawLibraryPath == null) {
        return defaultSiblingLibraryPath();
      }

      return isAbsolute(rawLibraryPath)
        ? rawLibraryPath
        : join(moduleDirectory, rawLibraryPath);
    }

    === lifecycle ===
    export function load(libraryPath = undefined) {
      const resolvedLibraryPath = resolveLibraryPath(libraryPath);

      if (loadedBindings !== null) {
        if (loadedBindings.libraryPath === resolvedLibraryPath) {
          return loadedBindings;
        }

        throw new Error(
          `The native library is already loaded from ${JSON.stringify(loadedBindings.libraryPath)}. Call unload() before loading a different library path.`,
        );
      }

      const bindings = createBindings(resolvedLibraryPath);
      try {
        runtimeHooks.onLoad?.(bindings);
        validateContractVersion(bindings);
        validateChecksums(bindings);
      } catch (error) {
        try {
          runtimeHooks.onUnload?.(bindings);
        } catch {
          // Preserve the original initialization failure.
        }
        try {
          bindings.library.unload();
        } catch {
          // Preserve the original initialization failure.
        }
        throw error;
      }

      loadedBindings = bindings;
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
      loadedBindings.library.unload();
      loadedBindings = null;
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


    export function getFfiBindings() {
      if (loadedBindings === null) {
        throw new LibraryNotLoadedError(
          "The native library is not loaded. Call load(libraryPath) first.",
        );
      }

      return loadedBindings;
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
