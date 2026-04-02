use std::fs;

mod support;

use self::support::{generate_fixture_package, load_fixture_component_interface, remove_dir_all};

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("test strings should serialize as JSON")
}

fn read_generated_component_ffi_js(package_dir: &camino::Utf8PathBuf, namespace: &str) -> String {
    fs::read_to_string(
        package_dir
            .join(format!("{namespace}-ffi.js"))
            .as_std_path(),
    )
    .expect("generated ffi js should be readable")
}

fn read_generated_component_ffi_dts(package_dir: &camino::Utf8PathBuf, namespace: &str) -> String {
    fs::read_to_string(
        package_dir
            .join(format!("{namespace}-ffi.d.ts"))
            .as_std_path(),
    )
    .expect("generated ffi d.ts should be readable")
}

fn extract_generated_block<'a>(contents: &'a str, start_marker: &str, end_marker: &str) -> &'a str {
    let start = contents
        .find(start_marker)
        .unwrap_or_else(|| panic!("generated source should contain block marker {start_marker}"));
    let end = contents[start..]
        .find(end_marker)
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("generated source should terminate block {start_marker}"));
    &contents[start..end]
}

fn assert_substrings_in_order<'a>(haystack: &str, expected: impl IntoIterator<Item = &'a str>) {
    let mut offset = 0;
    for needle in expected {
        let relative_index = haystack[offset..].find(needle).unwrap_or_else(|| {
            panic!(
                "expected to find {needle:?} after:\n{}",
                &haystack[..offset]
            )
        });
        offset += relative_index + needle.len();
    }
}

#[test]
fn basic_fixture_ffi_metadata_and_lifecycle_follow_the_v2_contract() {
    let generated = generate_fixture_package("basic");
    let component_interface = load_fixture_component_interface(&generated.built_fixture);
    let ffi_js =
        read_generated_component_ffi_js(&generated.package_dir, &generated.built_fixture.namespace);
    let staged_library_file_name = generated
        .built_fixture
        .library_path
        .file_name()
        .expect("fixture cdylib should have a filename");
    let checksum_symbol = component_interface
        .iter_checksums()
        .next()
        .map(|(name, _)| name.to_string())
        .expect("fixture component should expose at least one checksum");
    let metadata_block = extract_generated_block(
        &ffi_js,
        "export const ffiMetadata = Object.freeze({",
        "function createBindingCore(",
    );
    let lifecycle_block = extract_generated_block(
        &ffi_js,
        "export function load(libraryPath = undefined) {",
        "export const ffiFunctions = Object.freeze({",
    );

    let expected_metadata_lines = vec![
        "export const ffiMetadata = Object.freeze({".to_string(),
        format!(
            "  namespace: {},",
            json_string(&generated.built_fixture.namespace)
        ),
        format!(
            "  cdylibName: {},",
            json_string(&generated.built_fixture.crate_name)
        ),
        format!(
            "  stagedLibraryFileName: {},",
            json_string(staged_library_file_name)
        ),
        format!(
            "  stagedLibraryPackageRelativePath: {},",
            json_string(generated.staged_library_package_relative_path.as_str())
        ),
        "  bundledPrebuilds: false,".to_string(),
        "  manualLoad: false,".to_string(),
        "export const ffiIntegrity = Object.freeze({".to_string(),
        format!(
            "  contractVersionFunction: {},",
            json_string(component_interface.ffi_uniffi_contract_version().name())
        ),
        format!(
            "  expectedContractVersion: {},",
            component_interface.uniffi_contract_version()
        ),
        "  checksums: Object.freeze({".to_string(),
        format!("    {}:", json_string(&checksum_symbol)),
        "let loadedBindings = null;".to_string(),
        "let loadedFfiTypes = null;".to_string(),
        "let loadedFfiFunctions = null;".to_string(),
        "let cachedBindingCore = null;".to_string(),
        "let cachedLibraryPath = null;".to_string(),
        "let runtimeHooks = Object.freeze({});".to_string(),
        "const libraryNotLoadedMessage =".to_string(),
        "function defaultSiblingLibraryPath() {".to_string(),
        "return join(moduleDirectory, ffiMetadata.stagedLibraryPackageRelativePath);".to_string(),
        "function resolveLibraryPath(libraryPath = undefined) {".to_string(),
        "if (ffiMetadata.bundledPrebuilds) {".to_string(),
        "packageRelativePath: ffiMetadata.stagedLibraryPackageRelativePath,".to_string(),
        "function canonicalizeExistingLibraryPath(libraryPath) {".to_string(),
    ];
    assert_substrings_in_order(
        metadata_block,
        expected_metadata_lines.iter().map(String::as_str),
    );

    let expected_lifecycle_lines = vec![
        "export function load(libraryPath = undefined) {".to_string(),
        "const resolution = resolveLibraryPath(libraryPath);".to_string(),
        "const resolvedLibraryPath = resolution.libraryPath;".to_string(),
        "const canonicalLibraryPath = canonicalizeExistingLibraryPath(resolvedLibraryPath);"
            .to_string(),
        "if (loadedBindings !== null) {".to_string(),
        "if (loadedBindings.libraryPath === canonicalLibraryPath) {".to_string(),
        "return loadedBindings;".to_string(),
        "Call unload() before loading a different library path.".to_string(),
        "if (packageRelativePath !== null && !existsSync(resolvedLibraryPath)) {".to_string(),
        "No staged UniFFI library was found at".to_string(),
        "let bindingCore =".to_string(),
        "cachedLibraryPath === canonicalLibraryPath".to_string(),
        "cachedBindingCore.library.unload();".to_string(),
        "const bindings = createBindings(canonicalLibraryPath, bindingCore, resolution);"
            .to_string(),
        "runtimeHooks.onLoad?.(bindings);".to_string(),
        "validateContractVersion(bindings);".to_string(),
        "validateChecksums(bindings);".to_string(),
        "bindingCore = cacheBindingCore(canonicalLibraryPath, bindings);".to_string(),
        "runtimeHooks.onUnload?.(bindings);".to_string(),
        "bindings.library.unload();".to_string(),
        "loadedBindings = bindings;".to_string(),
        "loadedFfiTypes = bindings.ffiTypes;".to_string(),
        "loadedFfiFunctions = bindings.ffiFunctions;".to_string(),
        "export function unload() {".to_string(),
        "loadedBindings = null;".to_string(),
        "loadedFfiTypes = null;".to_string(),
        "loadedFfiFunctions = null;".to_string(),
        "export function isLoaded() {".to_string(),
        "export function configureRuntimeHooks(hooks = undefined) {".to_string(),
        "function throwLibraryNotLoaded() {".to_string(),
        "export function getFfiBindings() {".to_string(),
        "export function getFfiTypes() {".to_string(),
        "function getLoadedFfiFunctions() {".to_string(),
        "export function getContractVersion(bindings = getFfiBindings()) {".to_string(),
        "export function validateContractVersion(bindings = getFfiBindings()) {".to_string(),
        "export function getChecksums(bindings = getFfiBindings()) {".to_string(),
        format!(
            "{}: bindings.ffiFunctions.{}()",
            json_string(&checksum_symbol),
            checksum_symbol
        ),
        "export function validateChecksums(bindings = getFfiBindings()) {".to_string(),
        format!(
            "const expected = ffiIntegrity.checksums[{}];",
            json_string(&checksum_symbol)
        ),
        format!(
            "const actual = actualChecksums[{}];",
            json_string(&checksum_symbol)
        ),
        format!(
            "throw new ChecksumMismatchError({}, expected, actual, {{",
            json_string(&checksum_symbol)
        ),
    ];
    assert_substrings_in_order(
        lifecycle_block,
        expected_lifecycle_lines.iter().map(String::as_str),
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(&generated.package_dir);
}

#[test]
fn basic_fixture_ffi_typescript_contract_matches_the_v2_lifecycle_surface() {
    let generated = generate_fixture_package("basic");
    let ffi_dts = read_generated_component_ffi_dts(
        &generated.package_dir,
        &generated.built_fixture.namespace,
    );
    let dts_block = extract_generated_block(
        &ffi_dts,
        "export interface FfiMetadata {",
        "export declare const ffiFunctions: Readonly<Record<string, (...args: any[]) => any>>;",
    );
    let expected_dts_lines = [
        "export interface FfiMetadata {",
        "namespace: string;",
        "cdylibName: string;",
        "stagedLibraryFileName: string;",
        "stagedLibraryPackageRelativePath: string;",
        "bundledPrebuilds: boolean;",
        "manualLoad: boolean;",
        "export interface FfiBindings {",
        "libraryPath: string;",
        "packageRelativePath: string | null;",
        "library: unknown;",
        "ffiTypes: Readonly<Record<string, unknown>>;",
        "ffiCallbacks: Readonly<Record<string, unknown>>;",
        "ffiStructs: Readonly<Record<string, unknown>>;",
        "ffiFunctions: Readonly<Record<string, (...args: any[]) => any>>;",
        "export interface FfiIntegrity {",
        "contractVersionFunction: string;",
        "expectedContractVersion: number;",
        "checksums: Readonly<Record<string, number>>;",
        "export interface FfiRuntimeHooks {",
        "onLoad?(bindings: Readonly<FfiBindings>): void;",
        "onUnload?(bindings: Readonly<FfiBindings>): void;",
        "export declare const ffiMetadata: Readonly<FfiMetadata>;",
        "export declare const ffiIntegrity: Readonly<FfiIntegrity>;",
        "export declare function configureRuntimeHooks(hooks?: FfiRuntimeHooks | null): void;",
        "export declare function load(libraryPath?: string | null): Readonly<FfiBindings>;",
        "export declare function unload(): boolean;",
        "export declare function isLoaded(): boolean;",
        "export declare function getFfiBindings(): Readonly<FfiBindings>;",
        "export declare function getFfiTypes(): Readonly<Record<string, unknown>>;",
        "export declare function getContractVersion(bindings?: Readonly<FfiBindings>): number;",
        "export declare function validateContractVersion(bindings?: Readonly<FfiBindings>): number;",
        "export declare function getChecksums(",
        "bindings?: Readonly<FfiBindings>,",
        "): Readonly<Record<string, number>>;",
        "export declare function validateChecksums(",
        "bindings?: Readonly<FfiBindings>,",
        "): Readonly<Record<string, number>>;",
    ];
    assert_substrings_in_order(dts_block, expected_dts_lines);

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(&generated.package_dir);
}
