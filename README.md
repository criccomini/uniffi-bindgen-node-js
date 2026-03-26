# uniffi-bindgen-node-js

`uniffi-bindgen-node-js` is a single Cargo package that exposes:

- a reusable `uniffi_bindgen_node_js` Rust library
- a `uniffi-bindgen-node-js` CLI for generating Node bindings from a built UniFFI `cdylib`

The generator emits one npm package per invocation with a public API layer, a low-level FFI layer, and shared runtime helpers built around `koffi`.

## Installation

Build the CLI locally:

```sh
cargo build
```

Install it into your Cargo bin directory:

```sh
cargo install --path .
```

Or run it from the repo without installing:

```sh
cargo run -- --help
```

## CLI usage

Generate a package from a built UniFFI dynamic library:

```sh
uniffi-bindgen-node-js generate \
  path/to/libyour_crate.dylib \
  --crate-name your-crate \
  --out-dir /tmp/your-package
```

Available generator flags:

- `--crate-name <crate>`: Cargo package name or underscored library crate name for UniFFI library-mode generation.
- `--out-dir <dir>`: output directory for the generated npm package.
- `--package-name <name>`: generated npm package name.
- `--cdylib-name <name>`: native library basename used for sibling-library lookup.
- `--node-engine <range>`: value written to `package.json` `engines.node`.
- `--lib-path-literal <path>`: emitted literal path for the native library.
- `--bundled-prebuilds`: emit runtime resolution for packaged native libraries under `prebuilds/<target>/`.
- `--manual-load`: disables eager library loading and exposes explicit load helpers.
- `--config-override KEY=VALUE`: override supported `[bindings.node]` settings from the CLI.

The CLI expects a built `cdylib` as `lib_source`. By default the generated package looks for a sibling native library next to the emitted JS files. With `--bundled-prebuilds`, the generated loader instead resolves `prebuilds/<target>/<default-library-filename>` inside the package, while still allowing `load(path)` to override the runtime path explicitly.

## UniFFI config

Generator settings can also come from `uniffi.toml`:

```toml
[bindings.node]
package_name = "your-package"
cdylib_name = "your_crate"
node_engine = ">=16"
bundled_prebuilds = true
manual_load = false
```

Defaults:

- `package_name`: UniFFI namespace
- `cdylib_name`: UniFFI `cdylib` name from generation settings
- `node_engine`: `>=16`
- `lib_path_literal`: unset
- `bundled_prebuilds`: `false`
- `manual_load`: `false`

CLI flags apply after config-file settings. `bundled_prebuilds = true` cannot be combined with `lib_path_literal`, because both would otherwise define the default auto-load path.

## Generated package layout

Each invocation writes a package directory containing:

- `package.json`
- `index.js`
- `index.d.ts`
- `<namespace>.js`
- `<namespace>.d.ts`
- `<namespace>-ffi.js`
- `<namespace>-ffi.d.ts`
- `runtime/errors.js`
- `runtime/ffi-types.js`
- `runtime/ffi-converters.js`
- `runtime/rust-call.js`
- `runtime/async-rust-call.js`
- `runtime/handle-map.js`
- `runtime/callbacks.js`
- `runtime/objects.js`

The generated `package.json` declares `koffi` as the runtime FFI dependency.

Default native-library packaging modes:

- sibling mode: copy the built library next to the generated JS files, for example `<package>/libyour_crate.dylib`
- bundled-prebuild mode: stage one library per target under `prebuilds/<target>/<filename>`, for example `prebuilds/darwin-arm64/libyour_crate.dylib`, `prebuilds/linux-x64-gnu/libyour_crate.so`, or `prebuilds/win32-x64/your_crate.dll`

Bundled target IDs use Node's `process.platform` and `process.arch`. Linux targets add a libc suffix: `-gnu` when `process.report?.getReport?.().header.glibcVersionRuntime` is present, otherwise `-musl`.

## Output format

v1 output is ESM-only. Generated packages set `"type": "module"` and export the generated entrypoints through `index.js`.

The generator emits ready-to-consume JavaScript and declaration files directly:

- `index.js` and `index.d.ts`
- `<namespace>.js` and `<namespace>.d.ts`
- `<namespace>-ffi.js` and `<namespace>-ffi.d.ts`

Downstream consumers do not need a TypeScript build step to use the generated package.

## Compatibility

The first-class target for this generator is UniFFI 0.29.

This repo currently pins:

- `uniffi = 0.29.5`
- `uniffi_bindgen = 0.29.5`

Support for newer UniFFI releases is follow-up work.

## Supported UniFFI surface

The current generator is scoped to:

- top-level functions
- objects, constructors, and synchronous methods
- async methods using the UniFFI Rust-future polling path
- records
- flat enums
- tagged enums
- UniFFI error enums as JavaScript error classes
- `Option<T>`
- `Vec<T>`
- `HashMap<K, V>`
- `bytes` as `Uint8Array`
- synchronous callback interfaces

Public API conventions:

- byte values use `Uint8Array`, and Node `Buffer` inputs work because `Buffer` is a `Uint8Array`
- `Option<T>` maps to `T | undefined`
- `Vec<T>` maps to `Array<T>`
- `HashMap<K, V>` maps to `Map<K, V>`
- 64-bit integers use bigint-aware converters
- records are plain JavaScript objects with matching declaration interfaces
- objects are JavaScript classes backed by UniFFI handles

## Current limitations

The generator rejects or does not yet support:

- UniFFI custom types
- UniFFI external types
- async callback-interface methods
- timestamps in the public Node API
- durations in the public Node API
- automatic multi-target package assembly; bundled prebuilds only define the runtime lookup contract and still require the release pipeline to stage `prebuilds/<target>/...`

## Development

Local tests cover:

- generator config parsing and output-path handling
- snapshot/codegen output for fixture crates
- generated package creation and dependency installation
- plain JavaScript smoke tests
- TypeScript declaration checks
