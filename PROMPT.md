# SlateDB-First Node UniFFI Bindgen

## Summary

- Turn this repo into a single Cargo package that exposes a reusable NodeBindingGenerator library plus a uniffi-bindgen-node-js CLI.
- Target UniFFI 0.29.x first so the generator matches SlateDB’s current bindings/uniffi crate.
- Use UniFFI library-mode generation + Askama templates to emit one ESM npm package per invocation, with ready-to-consume .js + .d.ts output.
- Replace ffi-rs with koffi, and scope v1 to the UniFFI feature set SlateDB actually needs: objects, constructors, sync/async methods, records, flat/tagged enums, error enums,
Option, Vec, HashMap, bytes, and synchronous callback interfaces.

## Public Interfaces

- CLI: generate <lib_source> --crate-name <crate> --out-dir <dir> plus --package-name, --cdylib-name, --node-engine, --lib-path-literal, --manual-load, and --config-override.
- UniFFI config: add [bindings.node] support with the same knobs; defaults are package name = crate namespace, node engine = >=16, auto-load enabled, and library path =
sibling native library file.
- Generated package API conventions:
- bytes -> Uint8Array; Node Buffer inputs work naturally.
- Option<T> -> T | undefined, Vec<T> -> Array<T>, HashMap<K,V> -> Map<K,V>, i64/u64 -> bigint.
- Records -> plain JS objects + .d.ts interfaces.
- Flat enums -> frozen runtime constant objects with string literal values.
- Tagged enums -> tagged JS objects/classes.
- UniFFI errors -> Error subclasses.
- Objects -> JS classes backed by UniFFI handles.
- Generated files: package.json, index.js, index.d.ts, one component API pair (<namespace>.js/.d.ts), one low-level FFI pair (<namespace>-ffi.js/.d.ts), and shared runtime
helper modules under runtime/.

## Implementation Changes

- Add src/lib.rs, src/bindings/*, src/subcommands/*, templates/**/*, and askama.toml; keep this repo as one Cargo package, not a workspace.
- Port the old BindingGenerator/library-mode shape from /tmp/uniffi-bindgen-node, but simplify it for JS output and SlateDB-first scope.
- Pin uniffi/uniffi_bindgen to the SlateDB-compatible 0.29 line first.
- Build a Node runtime around koffi:
- declare UniFFI FFI functions, RustBuffer, ForeignBytes, RustCallStatus, and callback-interface vtables from ci.ffi_definitions().
- port/adapt the minimal runtime pieces from uniffi-bindgen-react-native: errors, ffi-types, ffi-converters, rust-call, async-rust-call, handle-map, callbacks, and
objects.
- normalize Koffi 64-bit integer marshalling so the public API always exposes bigint.
- make load() idempotent; on first load, open the library, register callback vtables, then run UniFFI contract-version and checksum checks.
- use registered Koffi callbacks for long-lived callback-interface vtables and the global Rust-future continuation callback.
- Keep native library resolution simple in v1: sibling library by default plus a literal-path override.
- Reject unsupported v1 constructs with explicit generator errors: custom types, external types, async callback-interface methods, CommonJS output, and multi-package/platform-
switch packaging.

## Test Plan

- Rust unit tests for config parsing, path resolution, Koffi FFI type mapping, bytes/enum/error naming, and unsupported-feature diagnostics.
- Snapshot/codegen tests using small local fixture crates that cover:
- objects + constructors + async methods
- flat/tagged enums and error enums
- records, Option, Vec, Map, bytes
- synchronous callback interfaces
- End-to-end Node smoke tests:
- build a small fixture cdylib, generate a package into /tmp, install npm deps, and execute a JS smoke script.
- run the same smoke flow against local SlateDB bindings/uniffi without modifying that repo; the script should import the generated package, create Settings.default(),
mutate settings via set()/to_json_string(), create a WriteBatch and put()/delete() using Buffer, and call init_logging() with a JS callback implementation.
- Acceptance criteria:
- generated SlateDB bindings import cleanly in plain JS and TypeScript.
- async methods resolve through the Rust-future polling path.
- callback interfaces register and round-trip without handle leaks or stale-handle errors in normal use.

## Assumptions And Defaults

- ESM-only output for v1.
- JS + .d.ts are generated directly from Askama templates; downstream packages do not need a TypeScript build step.
- UniFFI 0.29.x compatibility is the first-class target; widening to newer UniFFI versions is follow-up work after SlateDB is green.
- Runtime choice is koffi, because its docs explicitly cover aggregate C types, JS callbacks, and registered callbacks for long-lived native interactions: https://koffi.dev/
and https://koffi.dev/callbacks

## Instructions

You are in a Ralph Wiggum loop. Work through the first few TODOs in the `## TODO` section below.

- update PROMPT.md with an updated TODO list after each change
- never ever change any PROMPT.md text _except_ the items in the `## TODO` section
- you may update the TODO items as you see fit--remove outdated items, add new items, mark items as completed
- commit after each completed TODO
- use conventional commit syntax for commit messages
- if there are no items left in TODO.md, return codex with a non-zero exit code to signal that 

## TODO

- [x] Convert the repo from the current stub into a real single-package Cargo crate with both a library target and the `uniffi-bindgen-node-js` binary.
- [x] Add `src/lib.rs` and move CLI startup out of `src/main.rs` into reusable modules.
- [x] Add `askama.toml` and a `templates/` directory for all generated output.
- [x] Add Rust dependencies for `anyhow`, `askama`, `camino`, `cargo_metadata`, `clap`, `heck`, `serde`, `serde_json`, `textwrap`, `toml`, `uniffi`, and `uniffi_bindgen`.
- [x] Pin `uniffi` and `uniffi_bindgen` to the UniFFI 0.29 line to match SlateDB.
- [x] Implement a `generate` subcommand that accepts `lib_source`, `--crate-name`, and `--out-dir`.
- [x] Add CLI options for `--package-name`, `--cdylib-name`, `--node-engine`, `--lib-path-literal`, `--manual-load`, and `--config-override`.
- [x] Make the CLI call UniFFI library-mode generation with a custom `NodeBindingGenerator`.
- [x] Parse `[bindings.node]` from `uniffi.toml` and merge it with CLI overrides.
- [x] Fail with clear errors when required inputs are missing or invalid.
- [x] Add `src/bindings/mod.rs` with a `NodeBindingGenerator` that implements `uniffi_bindgen::BindingGenerator`.
- [x] Add a config struct for `[bindings.node]` settings and defaults.
- [x] Write generated files into one output package directory per invocation.
- [x] Generate `package.json`, `index.js`, `index.d.ts`, `<namespace>.js`, `<namespace>.d.ts`, `<namespace>-ffi.js`, and `<namespace>-ffi.d.ts`.
- [x] Generate shared runtime helper files under `runtime/`.
- [x] Add a generator-side component model that collects top-level functions, objects, constructors, methods, records, enums, and errors before template rendering.
- [x] Reject unsupported v1 inputs up front with explicit generator errors for custom types, external types, and callback interfaces that are still waiting on runtime support.
- [x] Render public API types for `bytes`, `i64/u64`, `Option<T>`, `Vec<T>`, `HashMap<K, V>`, and the nested combinations SlateDB needs.
- [x] Generate public JS + `.d.ts` skeletons for top-level functions, objects, constructors, methods, records, flat enums, tagged enums, and error enums.
- [x] Support synchronous callback interfaces needed by SlateDB.
- [x] Reject unsupported v1 features with explicit generator errors: async callback-interface methods, CommonJS output, and multi-package platform-switch packaging.
- [x] Add `koffi` as the generated package FFI dependency instead of `ffi-rs`.
- [x] Implement library loading and symbol binding with `koffi`.
- [x] Declare runtime representations for UniFFI `RustBuffer`, `ForeignBytes`, `RustCallStatus`, handles, and callback vtables.
- [x] Normalize Koffi 64-bit values so generated bindings consistently expose `bigint`.
- [x] Make library loading idempotent.
- [x] Support automatic library loading by default.
- [x] Support `--manual-load` by exporting explicit load and unload helpers.
- [x] Support sibling-library lookup by default plus literal-path override support.
- [x] Add a `runtime/errors.js` + `.d.ts` module with UniFFI error helpers and internal runtime errors.
- [x] Add a `runtime/ffi-types.js` + `.d.ts` module with `RustBuffer` and raw byte helpers.
- [x] Add a `runtime/ffi-converters.js` + `.d.ts` module for primitive, optional, sequence, map, timestamp, duration, string, and byte-array converters.
- [x] Add a `runtime/rust-call.js` + `.d.ts` module for sync Rust call handling and `RustCallStatus` checking.
- [x] Add a `runtime/async-rust-call.js` + `.d.ts` module for Rust future polling, completion, cancellation, and cleanup.
- [x] Add a `runtime/handle-map.js` + `.d.ts` module for foreign callback/object handles.
- [x] Add a `runtime/callbacks.js` + `.d.ts` module for callback-interface registration and callback error propagation.
- [x] Add a `runtime/objects.js` + `.d.ts` module for UniFFI object factories, object converters, and destruction semantics.
- [x] Generate Koffi declarations for every FFI function in `ci.ffi_definitions()`.
- [x] Generate Koffi struct definitions for every UniFFI FFI struct used by the component.
- [x] Generate callback declarations for callback-interface methods and Rust future continuation callbacks.
- [x] Generate low-level wrappers for checksum functions and the UniFFI contract-version function.
- [x] Run contract-version validation at initialization time.
- [x] Run checksum validation at initialization time.
- [x] Generate public object classes for UniFFI objects.
- [ ] Generate constructor wrappers that lift returned handles into JS objects.
- [ ] Generate sync method wrappers that lower arguments, call FFI, check `RustCallStatus`, and lift results.
- [ ] Generate async method wrappers that create, poll, complete, and free Rust futures.
- [ ] Generate record type definitions and record converters.
- [ ] Generate flat enum runtime representations and converters.
- [ ] Generate tagged enum runtime representations and converters.
- [ ] Generate error classes and error converters.
- [ ] Generate callback-interface wrappers for `LogCallback` and `MergeOperator`.
- [ ] Register callback-interface vtables during package initialization.
- [ ] Confirm generated bindings compile against the full SlateDB UniFFI surface without unsupported-feature failures.
- [ ] Ensure `HashMap<String, i64>` maps cleanly to `Map<string, bigint | number>` according to the chosen converter rules.
- [ ] Ensure nested `Vec<Vec<u8>>` arguments and returns work correctly.
- [ ] Ensure `Option<Vec<u8>>` and `Option<Arc<Object>>` patterns work correctly.
- [ ] Ensure synchronous callback interfaces work for `init_logging` and merge operators.
- [ ] Ensure async methods work for DB, reader, iterator, snapshot, transaction, and WAL APIs.
- [ ] Create a minimal local UniFFI fixture crate for objects, records, enums, errors, async methods, and bytes.
- [ ] Create a local fixture crate for synchronous callback interfaces.
- [ ] Add Rust tests that snapshot generated JS and `.d.ts` output for the fixtures.
- [ ] Add Rust tests for config parsing and output path resolution.
- [ ] Add Rust tests for unsupported-feature diagnostics.
- [ ] Add Rust tests for generated checksum and contract-version initialization code.
- [ ] Build a fixture cdylib during tests.
- [ ] Generate a Node package into a temp directory.
- [ ] Install npm dependencies in the temp directory.
- [ ] Run a plain JS smoke script that imports the generated package and exercises sync and async calls.
- [ ] Run a TypeScript smoke script or `tsc --noEmit` check against the generated `.d.ts` output.
- [ ] Verify that passing Node `Buffer` values into `Uint8Array` byte parameters works correctly.
- [ ] Build `/Users/chrisriccomini/Code/slatedb/bindings/uniffi` as a cdylib without modifying the SlateDB repo.
- [ ] Generate a Node package from the built SlateDB library into a temp directory.
- [ ] Install npm dependencies for the generated package.
- [ ] Run a smoke script that imports the generated SlateDB package.
- [ ] In the smoke script, call `Settings.default()`, `set()`, and `to_json_string()`.
- [ ] In the smoke script, create a `WriteBatch`, call `put()` with `Buffer` keys and values, and call `delete()`.
- [ ] In the smoke script, call `init_logging()` with a JS callback implementation and verify callback delivery.
- [ ] Treat successful import plus these calls as the first end-to-end acceptance gate.
- [ ] Add a README for this repo describing installation, CLI usage, supported UniFFI features, and current limitations.
- [ ] Document that v1 is ESM-only and emits ready-to-consume `.js` + `.d.ts`.
- [ ] Document that v1 targets UniFFI 0.29.x first.
- [ ] Run Rust tests last.
- [ ] Run the end-to-end Node tests after Rust tests pass.
