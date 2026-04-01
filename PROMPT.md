# Upgrade uniffi-bindgen-node-js to UniFFI 0.31 with a Node-v2 Architecture

## Summary

- Rebuild the crate around UniFFI 0.31.x and BindgenLoader, not BindingGenerator or library_mode::generate_bindings.
- Keep the product tightly focused on: given a built UniFFI cdylib, generate a self-contained ESM Node package that uses koffi.
- Preserve bundled prebuild support. In v2, --bundled-prebuilds still means the generated package uses prebuilds/<target>/...; the difference is that the generator itself
  stages the provided cdylib into that location for the host target instead of relying on caller-side copy logic.
- Treat this as a clean redesign. Do not preserve deprecated internal APIs, compatibility shims, or legacy configuration surface just because v1 had them.
- Feature target for v2: keep current Node capabilities, fully adapt them to UniFFI 0.30/0.31 loader/handle/callback semantics, and reject unsupported newer UniFFI features
  explicitly rather than partially supporting them.

## Public Interface and Behavior

- Keep a single top-level generate CLI command.
- Primary input remains a built native library path (.so, .dylib, .dll).
- --out-dir remains required.
- --crate-name becomes an optional component selector:
    - If the library yields exactly one UniFFI component, infer it.
    - If it yields multiple components, require --crate-name and report available crate names.
- Keep these supported Node options:
    - --package-name
    - --node-engine
    - --bundled-prebuilds
    - --manual-load
- Add --manifest-path <Cargo.toml> as an explicit source-resolution hint for UDL/config lookup when needed by BindgenLoader.
- Remove these v1-specific compatibility surfaces:
    - --config-override
    - --cdylib-name
    - --lib-path-literal
- Default staging behavior:
    - Without --bundled-prebuilds, copy the input cdylib into the package root next to generated JS files.
    - With --bundled-prebuilds, copy the input cdylib into prebuilds/<host-target>/....
    - With --manual-load, still stage the cdylib; only the load lifecycle changes.
- Keep ESM-only output.
- Keep the generated loader contract and runtime behavior Node-specific and package-oriented, not generic UniFFI bindgen plumbing.
- Narrow the Rust library API to one curated programmatic entrypoint such as generate_node_package(options) -> Result<()>. Do not keep NodeBindingGenerator,
  NodeBindingCliOverrides, or NodeBindingGeneratorConfig as public API.

## Architecture and Implementation

- Replace the current orchestration in src/subcommands/generate.rs and src/bindings/mod.rs with a Node-v2 pipeline:
    1. Parse and validate CLI options.
    2. Build BindgenPaths.
    3. Seed path/config resolution from --manifest-path when provided; otherwise rely on cargo-metadata/workspace resolution.
    4. Create BindgenLoader.
    5. Call load_metadata(lib_source).
    6. Call load_cis(metadata).
    7. Call load_components(cis, parse_node_config).
    8. Select one component.
    9. Apply Node defaults plus explicit CLI overrides.
   10. Apply UniFFI rename config before deriving FFI functions.
   11. Call derive_ffi_funcs() on selected component(s).
   12. Build a Node-specific normalized IR.
   13. Render package files.
   14. Stage the input cdylib into the package layout.
- Keep the renderer based on ComponentInterface, not UniFFI’s experimental pipeline IR.
- Split responsibilities cleanly:
    - CLI parsing and user-facing validation
    - loader/path/config orchestration
    - component selection
    - Node config normalization
    - package layout and native-library staging
    - public API IR
    - FFI IR
    - template rendering
    - runtime asset emission
- Update all UniFFI-semantic assumptions for 0.30/0.31:
    - object/interface handles now follow the newer handle ABI rather than v1’s RustArcPtr assumptions
    - callback/trait interface vtable layout and handle semantics must be updated
    - symbol and naming logic must tolerate full module_path values
    - checksum and contract-version handling must match UniFFI 0.31
    - async future/callback helper naming must match UniFFI 0.30/0.31
- Keep unsupported-feature policy explicit:
    - if v2 does not support a UniFFI surface, fail generation with a clear diagnostic
    - do not emit placeholder bindings or partially incorrect output
- Keep package contents stable in spirit:
    - package.json
    - index.js
    - index.d.ts
    - namespace API JS/TS files
    - *-ffi.js
    - *-ffi.d.ts
    - runtime helper files
    - staged native library in root or prebuilds/<target>
- Keep koffi as the runtime FFI layer.

## Testing and Acceptance

- Shift end-to-end confidence to real built fixtures and the new loader-driven generation path.
- Keep small renderer-focused unit tests only where direct synthetic ComponentInterface construction remains materially simpler and still reflects post-loader semantics.
- Preserve and update coverage for:
    - package generation
    - snapshot output
    - TypeScript declarations
    - async runtime behavior
    - callback interfaces
    - manual load / unload behavior
    - bundled prebuild resolution
    - unsupported-feature diagnostics
    - leak probes
- Add explicit acceptance cases for:
    - single-component cdylib without --crate-name
    - multi-component cdylib requiring --crate-name
    - proc-macro-only component generation from a library path
    - UDL-backed library generation that succeeds with --manifest-path
    - UDL-backed library generation that fails clearly without enough source/config context
    - host-target staging in bundled-prebuild mode
    - root-level staging in default mode
    - manual_load with staged binaries
    - rename config being applied before derive_ffi_funcs()
    - updated handle ABI behavior in generated runtime and koffi test shim

## Assumptions

- v2 keeps the same product scope: generate a Node package from a built cdylib.
- v2 does not expand to a UDL-first UX.
- v2 preserves bundled-prebuild layout but does not add automatic multi-target assembly in one invocation.
- Primary happy path is libraries with enough embedded UniFFI metadata; --manifest-path is the source-resolution escape hatch for UDL/config lookup.
- Unsupported surfaces should fail clearly for now, including any UniFFI 0.30/0.31 additions not intentionally adopted in v2.
- Compatibility with v1 internal APIs and deprecated CLI/config knobs is out of scope.

## Instructions

You are in a Ralph Wiggum loop. You are making progress on the plan defined above. Work through the first few TODOs in the `## TODO` section below.

- update PROMPT.md with an updated TODO list after each change
- never ever change any PROMPT.md text _except_ the items in the `## TODO` section
- you may update the TODO items as you see fit--remove outdated items, add new items, mark items as completed
- commit after each completed TODO
- use conventional commit syntax for commit messages
- if there are no items left in TODO.md, append a final line in PROMPT.md that contains only the emoji: ✅

## TODO

- [x] Bump root dependencies to UniFFI 0.31.x.
- [x] Bump fixture crate dependencies to UniFFI 0.31.x.
- [x] Update any supporting dependency versions needed to compile cleanly with UniFFI 0.31.x.
- [x] Regenerate Cargo.lock.
- [x] Remove reliance on deprecated BindingGenerator.
- [x] Remove reliance on library_mode::generate_bindings.
- [x] Remove reliance on CrateConfigSupplier as the primary architecture.
- [x] Remove or demote public exposure of NodeBindingGenerator.
- [x] Remove or demote public exposure of NodeBindingGeneratorConfig.
- [x] Remove or demote public exposure of NodeBindingCliOverrides.
- [x] Design and add a single v2 programmatic entrypoint for Node package generation.
- [x] Define a new request/options struct for the v2 programmatic entrypoint.
- [x] Define the internal result/data flow for generation so orchestration is explicit and testable.
- [x] Introduce a new Node-v2 orchestration module separate from rendering code.
- [x] Introduce a new Node-v2 config parsing/normalization module.
- [x] Introduce a new package-layout/staging module.
- [x] Introduce a new component-selection module.
- [x] Introduce a new public-API IR builder module or boundary.
- [x] Introduce a new FFI-IR builder module or boundary.
- [x] Move package-writing logic out of the legacy monolithic src/bindings/mod.rs shape.
- [x] Keep template rendering reusable but isolate it from loader/config orchestration.
- [x] Keep runtime asset emission separate from component-specific rendering.
- [x] Define the final v2 module layout and move code accordingly.
- [x] Build BindgenPaths in the new orchestration path.
- [x] Support --manifest-path for path/config/UDL resolution.
- [x] Decide and implement the exact BindgenPaths layering order used by v2.
- [x] Ensure config lookup is deterministic when both workspace discovery and --manifest-path are available.
- [x] Create BindgenLoader in the new flow.
- [x] Load metadata from the input library path with BindgenLoader.
- [x] Convert metadata to ComponentInterface values with BindgenLoader.
- [x] Load per-component TOML configuration with BindgenLoader.
- [x] Parse the Node-specific section of uniffi.toml in the new flow.
- [x] Apply Node defaults after raw config parse.
- [x] Apply explicit CLI overrides after config defaults.
- [x] Apply rename config before deriving FFI functions.
- [x] Call derive_ffi_funcs() in the new flow.
- [x] Select exactly one component for package generation.
- [x] Implement the single-component inference path.
- [x] Implement multi-component error reporting with discovered crate names.
- [x] Keep explicit crate filtering behavior when --crate-name is provided.
- [x] Validate the library input path in the new flow.
- [x] Validate --out-dir in the new flow.
- [x] Validate --manifest-path if provided.
- [x] Validate mutually incompatible Node options in the new v2 surface.
- [x] Remove --config-override from CLI parsing.
- [x] Remove --cdylib-name from CLI parsing.
- [x] Remove --lib-path-literal from CLI parsing.
- [x] Update CLI help text to reflect the v2 surface.
- [x] Update CLI error messages to mention v2 behavior rather than v1 terminology.
- [x] Decide and implement how package naming defaults are derived in v2.
- [x] Keep --package-name override support.
- [x] Keep --node-engine override support.
- [x] Keep --bundled-prebuilds flag support.
- [x] Keep --manual-load flag support.
- [x] Preserve ESM-only output policy in validation and docs.
- [x] Remove v1 CommonJS-compatibility checks that only exist because of legacy config surface.
- [x] Keep explicit rejection for CommonJS-oriented config if it still appears in old config files.
- [x] Decide whether old config keys are hard errors or ignored with diagnostics.
- [x] Implement v2 config diagnostics for removed keys.
- [x] Update README.md CLI reference for the v2 interface.
- [x] Update README.md packaging explanation to state that the generator stages the native library itself.
- [x] Update README.md bundled-prebuild explanation to state host-target staging per invocation.
- [x] Update README.md manual-load explanation to reflect staged binaries plus explicit lifecycle control.
- [x] Update README.md compatibility section to UniFFI 0.31.x.
- [x] Update README.md supported-surface and limitations section for v2.
- [x] Update DEVELOPMENT.md architecture notes to describe loader-based v2.
- [x] Update DEVELOPMENT.md contributor commands if CLI shape changes.
- [x] Update DEVELOPMENT.md leak-probe instructions if package staging behavior changes.
- [x] Remove outdated references to v1 wording from docs and error strings.
- [x] Design the new internal package layout abstraction around actual staged artifacts.
- [x] Make the package writer create the output directory structure.
- [x] Make the package writer emit metadata files.
- [x] Make the package writer emit namespace API JS/TS files.
- [x] Make the package writer emit FFI JS/TS files.
- [x] Make the package writer emit runtime helper files.
- [x] Make the package writer stage the native library in default mode.
- [x] Make the package writer stage the native library in bundled-prebuild mode.
- [x] Compute the host prebuild target directory string in production code rather than only in tests.
- [x] Reuse one authoritative implementation for target naming between production and tests.
- [x] Ensure staged library filenames come from the actual input library filename, not a separate config field.
- [x] Ensure Windows/non-lib filename handling remains correct.
- [x] Ensure root-level staging and bundled-prebuild staging are mutually exclusive in one run.
- [x] Ensure manual_load does not suppress native-library staging.
- [x] Decide and implement overwrite behavior when a staged library already exists in out_dir.
- [ ] Ensure package generation is deterministic when rerun into an empty directory.
- [ ] Keep generated package structure consistent with current published shape where still appropriate.
- [ ] Revisit package.json generation for any Node engine or export-field cleanup needed in v2.
- [ ] Revisit top-level entrypoint generation to ensure manual-load and auto-load behavior are cleanly separated.
- [ ] Revisit namespace file generation to ensure imports are minimal and correct after refactor.
- [ ] Revisit runtime-helper file set to ensure only needed helpers are emitted.
- [ ] Rebuild the normalized IR used by templates so orchestration concerns are not mixed into rendering structs.
- [ ] Audit all ComponentInterface-to-IR conversion assumptions against UniFFI 0.31.
- [ ] Update object-handle modeling for the post-0.30 handle ABI.
- [ ] Remove RustArcPtr-specific assumptions where UniFFI now uses handles.
- [ ] Update object free/clone logic to match UniFFI 0.30/0.31 semantics.
- [ ] Update callback-interface handle logic to match UniFFI 0.30/0.31.
- [ ] Update any generic-ABI object retyping logic in generated runtime/tests.
- [ ] Update FFI type rendering for changed UniFFI FfiType expectations.
- [ ] Update return-value normalizers if the new handle ABI changes numeric handling.
- [ ] Audit and update any module_path-derived symbol synthesis now that full module paths are present.
- [ ] Remove any symbol synthesis that duplicates information UniFFI now exposes directly.
- [ ] Revalidate checksum extraction logic against UniFFI 0.31.
- [ ] Revalidate contract-version lookup logic against UniFFI 0.31.
- [ ] Update checksum mismatch diagnostics if needed for the new naming/layout.
- [ ] Update contract mismatch diagnostics if needed for the new naming/layout.
- [ ] Revalidate async Rust future helper symbol names against UniFFI 0.30/0.31.
- [ ] Revalidate foreign-future callback naming against UniFFI 0.30/0.31.
- [ ] Revalidate callback vtable field ordering against UniFFI 0.30/0.31.
- [ ] Update generated callback registration code to the new ordering and semantics.
- [ ] Update generated callback cleanup/unload logic if the new ABI requires it.
- [ ] Revalidate object finalization and unload behavior with the new handle ABI.
- [ ] Revalidate manual-load reentrancy/idempotence under the new runtime model.
- [ ] Revalidate cached binding-core logic against the new handle/callback semantics.
- [ ] Revalidate bundled-prebuild resolution logic and target naming in generated JS.
- [ ] Keep root-level default resolution behavior for non-bundled packages.
- [ ] Keep explicit load(path) override behavior in manual-load mode.
- [ ] Ensure generated loader error messages mention staged package paths accurately.
- [ ] Revisit unsupported-feature validation for v2.
- [ ] Keep explicit rejection for custom types unless intentionally added.
- [ ] Keep explicit rejection for external types unless intentionally added.
- [ ] Decide whether record/enum methods are supported in v2.
- [ ] If record/enum methods are not supported, add explicit diagnostics for them.
- [ ] Decide whether exported trait methods on records/enums are supported in v2.
- [ ] If exported trait methods are not supported, add explicit diagnostics for them.
- [ ] Audit current supported surface against UniFFI 0.31 so v2 diagnostics are complete.
- [ ] Update diagnostics to stop saying “v1” unless the message is intentionally historical.
- [ ] Keep diagnostics actionable and grouped by unsupported feature family.
- [ ] Add coverage for rename config behavior.
- [ ] Add coverage for full module_path names.
- [ ] Add coverage for multi-component library detection and error messages.
- [ ] Add coverage for component selection when --crate-name is present.
- [ ] Add coverage for default component inference when only one component exists.
- [ ] Add coverage for --manifest-path success cases.
- [ ] Add coverage for --manifest-path validation errors.
- [ ] Add coverage for UDL-backed library generation with insufficient source/config context.
- [ ] Add coverage for proc-macro-only library generation.
- [ ] Add coverage for default root-level staging of the input cdylib.
- [ ] Add coverage for bundled-prebuild staging of the input cdylib.
- [ ] Add coverage for host-target directory naming in bundled mode.
- [ ] Add coverage for manual-load with bundled-prebuild packages.
- [ ] Add coverage for manual-load with root-staged packages.
- [ ] Add coverage for load/unload idempotence after the ABI updates.
- [ ] Add coverage for callback interfaces under the new UniFFI ABI.
- [ ] Add coverage for async callbacks under the new UniFFI ABI.
- [ ] Add coverage for object clone/free semantics under the new ABI.
- [ ] Add coverage for checksum validation under UniFFI 0.31.
- [ ] Add coverage for contract-version validation under UniFFI 0.31.
- [ ] Add coverage for error-class generation and throwing behavior after the refactor.
- [ ] Add coverage for TypeScript output after any naming or surface changes.
- [ ] Add coverage for package generation snapshots using the loader-based flow.
- [ ] Replace tests that currently depend on BindingGenerator::write_bindings.
- [ ] Replace tests that currently depend on manual GenerationSettings construction where no longer appropriate.
- [ ] Replace test helpers that manually construct Component<NodeBindingGeneratorConfig> where end-to-end loader coverage is required.
- [ ] Decide which unit tests should remain synthetic for speed and precision.
- [ ] Keep fixture-based end-to-end tests for the critical path.
- [ ] Update tests/support/mod.rs to stop copying native libraries after generation.
- [ ] Move any remaining staging helpers that are still needed into production code or delete them.
- [ ] Update tests/node_package_generation.rs for generator-owned staging.
- [ ] Update tests/node_smoke_tests.rs for new runtime/ABI expectations.
- [ ] Update tests/node_async_runtime_tests.rs for any async symbol/name/layout changes.
- [ ] Update tests/node_buffer_tests.rs for any ABI or converter changes.
- [ ] Update tests/node_typescript_tests.rs for any public-surface changes.
- [ ] Update tests/unsupported_feature_diagnostics.rs to assert new v2 diagnostics.
- [ ] Update tests/ffi_initialization_codegen.rs to assert new FFI metadata and lifecycle output.
- [ ] Update tests/codegen_snapshots.rs to use the loader-based path.
- [ ] Update tests/node_real_koffi_tests.rs for any ABI/runtime changes.
- [ ] Update tests/npm-fixtures/koffi/index.js so the fake koffi behavior matches UniFFI 0.30/0.31 handle/callback semantics.
- [ ] Update snapshot files for the new generated output.
- [ ] Update fixture crates if UniFFI 0.31 requires source-level changes.
- [ ] Add a fixture or test case for multi-component libraries if none exists today.
- [ ] Add a fixture or test case for UDL-backed library generation through the new loader path if needed.
- [ ] Update leak helper binaries to call the new v2 entrypoint.
- [ ] Update leak helper binaries to rely on generator-owned staging rather than local copy logic.
- [ ] Revalidate leak probes after the refactor.
- [ ] Revalidate ignored real-koffi tests after the refactor.
- [ ] Run cargo test --locked.
- [ ] Run ignored Rust tests that are part of the existing acceptance bar.
- [ ] Run the generated-package smoke suites on supported platforms as available.
- [ ] Run TypeScript declaration checks through the existing test harness.
- [ ] Run the leak probes as a final regression pass.
- [ ] Review the final crate for any dead code left behind by the v1 architecture removal.
- [ ] Remove obsolete helper functions and tests tied only to v1 compatibility behavior.
- [ ] Remove obsolete config parsing branches tied only to removed CLI/config knobs.
- [ ] Remove obsolete documentation references to deprecated architecture.
- [ ] Ensure the final crate surface is clean, Node-focused, and clearly centered on self-contained package generation from a built cdylib.
