# Opt-In Bundled Prebuilds And Remove lib_path_modules

Summary

- Add an opt-in bundled_prebuilds mode so one generated npm package can contain multiple native UniFFI libraries and select the correct one at runtime.
- Keep current behavior as the default. Existing sibling-library lookup and lib_path_literal remain unchanged unless bundled_prebuilds is enabled.
- Remove lib_path_modules entirely from the supported config surface instead of keeping it as a parse-and-reject stub.

Interfaces

- Add [bindings.node] bundled_prebuilds = true and a matching CLI flag --bundled-prebuilds.
- Add matching --config-override keys for bundled_prebuilds / bundled-prebuilds and bindings.node.bundled_prebuilds / bindings.node.bundled-prebuilds.
- Extend generated ComponentMetadata and FfiMetadata with bundledPrebuilds: boolean.
- Keep load(libraryPath?: string | null) unchanged. Explicit load(path) remains the highest-precedence override.
- Remove lib_path_modules, lib_path_module, out_lib_path_module, and out_lib_path_modules from TOML aliases and CLI override parsing. Old TOML configs using those keys should
  now fail as unknown fields, and old CLI overrides should now fail with the existing unsupported-key error.

Implementation Changes

- Delete the lib_path_modules field from NodeBindingGeneratorConfig, remove its serde aliases, and remove the NodeBindingConfigOverride::LibPathModules variant and parser
  branch.
- Add bundled_prebuilds: bool to config, CLI overrides, defaults, and validation.
- Reject bundled_prebuilds = true together with lib_path_literal to avoid ambiguous auto-load behavior.
- Implement bundled-prebuild resolution in the generated FFI loader with this fixed contract: prebuilds/<target>/<default library filename>.
- Reuse the existing filename logic, so bundled entries look like prebuilds/darwin-arm64/libcrate.dylib, prebuilds/linux-x64-gnu/libcrate.so, and prebuilds/win32-x64/
  crate.dll.
- Compute <target> from Node runtime values: non-Linux targets are ${process.platform}-${process.arch}; Linux targets are ${process.platform}-${process.arch}-gnu when
  process.report?.getReport?.().header.glibcVersionRuntime is present and ${process.platform}-${process.arch}-musl otherwise.
- Change loader precedence to: explicit load(path) argument, then libPathLiteral if configured, then bundled prebuild when bundledPrebuilds is true, then existing sibling-
  library lookup.
- In bundled mode, check that the expected prebuild file exists before calling koffi.load and throw a targeted error that includes the resolved target id and expected in-
  package path.
- Keep manual_load behavior unchanged. Auto-load uses bundled resolution when enabled; manual-load packages still expose load and unload.
- Do not add a new runtime dependency unless implementation proves Linux libc detection cannot be done reliably with Node built-ins.
- Update README and generated-package docs to describe bundled_prebuilds, the prebuilds/<target>/ layout, and the removal of legacy lib_path_modules config.

Test Plan

- Add config tests that accept bundled_prebuilds = true and reject bundled_prebuilds + lib_path_literal.
- Add removal tests showing TOML with lib_path_modules-style keys now fails config deserialization and CLI overrides with those keys now fail as unsupported.
- Update codegen snapshot or section-based tests for generated FFI JS and DTS so they assert the new metadata field, bundled-prebuild branch, target-key computation, and
  clearer missing-file diagnostics.
- Add a package-generation test for bundled mode that stages the current host library into prebuilds/<current-target>/ and verifies the package shape without a root-level
  sibling library.
- Add a Node smoke test for bundled mode that installs dependencies, loads from the staged host prebuild automatically, and exercises the existing fixture API successfully.
- Add a manual-load regression test showing load(explicitPath) still overrides bundled mode and repeated loads remain idempotent for the same resolved path.
- Keep existing sibling-mode smoke, TypeScript, and package-generation tests unchanged to prove backward compatibility for the default path.

Assumptions

- The release pipeline already knows how to build the UniFFI cdylib for each desired target and can assemble a final package directory containing prebuilds/<target>/....
- Removing lib_path_modules is acceptable even though old configs using those keys will now fail earlier and more generically.
- Bundled-prebuild mode is opt-in and will not replace the existing sibling-library packaging model.
- Target ids follow Node naming for process.platform and process.arch, with an added Linux libc suffix of gnu or musl.
- If Linux libc cannot be determined reliably from the supported Node runtime, the loader should fail with a clear error instead of guessing and loading the wrong artifact.

## Instructions

You are in a Ralph Wiggum loop. Work through the first few TODOs in the `## TODO` section below.

- update PROMPT.md with an updated TODO list after each change
- never ever change any PROMPT.md text _except_ the items in the `## TODO` section
- you may update the TODO items as you see fit--remove outdated items, add new items, mark items as completed
- commit after each completed TODO
- use conventional commit syntax for commit messages
- if there are no items left in the TODO, delete PROMPT.md

## TODO

- [x] Add bundled_prebuilds: bool to NodeBindingGeneratorConfig and default it to false.
- [x] Add --bundled-prebuilds to GenerateArgs and pass it through NodeBindingCliOverrides::from_parts.
- [x] Add BundledPrebuilds(bool) to NodeBindingConfigOverride and parse the bundled_prebuilds / bundled-prebuilds override keys.
- [x] Remove lib_path_modules from NodeBindingGeneratorConfig, its serde aliases, and its default initialization.
- [x] Remove NodeBindingConfigOverride::LibPathModules and delete all lib_path_module[s] and out_lib_path_module[s] override aliases.
- [x] Update config validation to reject bundled_prebuilds = true together with lib_path_literal.
- [x] Thread bundled_prebuilds through GeneratedPackage, TemplateContext, and template inputs.
- [x] Extend generated JS and DTS metadata so componentMetadata and ffiMetadata expose bundledPrebuilds.
- [x] Update templates/component/component-ffi.js.j2 to compute the default library filename, compute the bundled target id, and resolve prebuilds/<target>/<filename> when
  bundled mode is enabled.
- [x] Implement Linux gnu vs musl detection in the generated loader using process.report?.getReport?.().header.glibcVersionRuntime.
- [x] Add a bundled-mode missing-file guard before koffi.load and emit an error that names the computed target id and expected path.
- [x] Keep load(libraryPath) override semantics intact and update resolver order to explicit path -> libPathLiteral -> bundled prebuild -> sibling library.
- [x] Update templates/component/component-ffi.d.ts.j2 and templates/component/component.d.ts.j2 to match the new metadata surface without changing load().
- [ ] Update README examples and limitations text to describe bundled prebuilds and remove references to lib_path_modules.
- [ ] Replace existing lib_path_modules rejection tests with tests that assert old TOML keys now fail as unknown fields and old CLI override keys now fail as unsupported.
- [x] Add config tests that accept bundled_prebuilds = true and reject bundled_prebuilds + lib_path_literal.
- [ ] Add codegen tests that assert the emitted bundled metadata, target-resolution helpers, resolver precedence, and bundled missing-file diagnostic.
- [ ] Extend test support helpers so a generated fixture package can stage the current host library under prebuilds/<current-target>/.
- [ ] Add a bundled-mode package-generation test that verifies the package works when only the prebuilds/<current-target>/ artifact is present.
- [ ] Add a bundled-mode Node smoke test that imports the package, auto-loads the host prebuild, and exercises the existing fixture API successfully.
- [ ] Add a bundled-mode manual-load regression test showing load(explicitPath) still overrides bundled resolution and remains idempotent for the same path.
- [ ] Add a negative runtime test for bundled mode with no matching staged prebuild and assert the error names the computed target and expected path.
- [ ] Run the relevant Rust test suite after implementation, including config, codegen, package-generation, and Node smoke tests.
