# uniffi-bindgen-node-js

`uniffi-bindgen-node-js` generates ESM Node packages for UniFFI `cdylib`s. Point it at a built Rust dynamic library and it writes a package with JavaScript, TypeScript declarations, a native loader, and the runtime helpers needed to call the library through `koffi`.

Contributor setup, tests, and coverage live in [DEVELOPMENT.md](./DEVELOPMENT.md).

## What It Produces

Each `generate` run writes one npm package containing:

- a public JavaScript API for your UniFFI namespace
- matching `.d.ts` files
- a low-level FFI module
- runtime helpers used by the generated bindings
- a `package.json` that declares `koffi` as the runtime dependency

Generated packages are ESM-only and do not require a TypeScript build step.

## Install

Build and install the CLI from this repository:

```sh
cargo install --path .
```

Or run it without installing:

```sh
cargo run -- --help
```

## Quick Start

1. Build your UniFFI crate as a `cdylib`.

```sh
cargo build --release -p your-crate
```

2. Generate a Node package from the built library.

```sh
uniffi-bindgen-node-js generate \
  path/to/your/built/library \
  --crate-name your-crate \
  --out-dir ./generated/your-package
```

Use the built library file for your platform:

- macOS: `libyour_crate.dylib`
- Linux: `libyour_crate.so`
- Windows: `your_crate.dll`

3. Install the generated package dependencies.

```sh
cd ./generated/your-package
npm install
```

You can then publish that directory as a package or install it into another app with `npm install ./generated/your-package`.

4. Consume the generated package from Node.

```js
import { greet } from "./generated/your-package/index.js";

console.log(greet("world"));
```

`--crate-name` accepts either the Cargo package name (`your-crate`) or the underscored library crate name (`your_crate`).

## CLI Reference

```sh
uniffi-bindgen-node-js generate [OPTIONS] --crate-name <CRATE_NAME> --out-dir <OUT_DIR> <LIB_SOURCE>
```

Required inputs:

- `LIB_SOURCE`: path to a built `.so`, `.dylib`, or `.dll`
- `--crate-name`: Cargo package name or library crate name
- `--out-dir`: directory where the npm package will be written

Optional flags:

- `--package-name <name>`: npm package name to write into `package.json`
- `--cdylib-name <name>`: native library basename used by the generated loader
- `--node-engine <range>`: value written to `package.json` `engines.node`
- `--lib-path-literal <path>`: hard-coded default path for the native library
- `--bundled-prebuilds`: resolve the default library from `prebuilds/<target>/...`
- `--manual-load`: export explicit `load()` and `unload()` helpers instead of auto-loading on import
- `--config-override KEY=VALUE`: override supported `[bindings.node]` settings from the CLI

## Packaging Modes

By default, the generated package expects a single native library for the current build and places it next to the generated JavaScript files:

```text
your-package/
  index.js
  your_namespace.js
  your_namespace-ffi.js
  libyour_crate.dylib
```

If you pass `--bundled-prebuilds`, the generated loader looks for platform-specific libraries under `prebuilds/<target>/`:

```text
your-package/
  index.js
  your_namespace.js
  your_namespace-ffi.js
  prebuilds/
    darwin-arm64/libyour_crate.dylib
    linux-x64-gnu/libyour_crate.so
    win32-x64/your_crate.dll
```

This mode defines the lookup contract only. You still need to build and stage the per-target native libraries yourself.

Linux bundled targets include a libc suffix:

- `-gnu` when Node reports glibc at runtime
- `-musl` otherwise

## Manual Loading

Without `--manual-load`, the generated package loads the native library during import.

With `--manual-load`, the top-level package exports `load()` and `unload()` so you can choose the library path explicitly:

```sh
uniffi-bindgen-node-js generate \
  path/to/your/built/library \
  --crate-name your-crate \
  --out-dir ./generated/your-package \
  --manual-load
```

```js
import { load, unload, greet } from "./generated/your-package/index.js";

load("./path/to/your/native/library");
console.log(greet("world"));
unload();
```

## `uniffi.toml` Configuration

You can store Node generator settings in `uniffi.toml`:

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

CLI flags apply after config-file settings.

`bundled_prebuilds = true` cannot be combined with `lib_path_literal`.

## Compatibility

- First-class target: UniFFI `0.29.5`
- Generated output: ESM-only
- Default Node engine range: `>=16`

## Supported UniFFI Surface

The current generator supports:

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
- callback interfaces, including async methods

Public API conventions:

- `Option<T>` becomes `T | undefined`
- `Vec<T>` becomes `Array<T>`
- `HashMap<K, V>` becomes `Map<K, V>`
- `bytes` becomes `Uint8Array`
- Node `Buffer` inputs also work for `bytes`
- 64-bit integers use `bigint`
- `timestamp` becomes `Date` with millisecond precision
- `duration` becomes `number` as a non-negative integer millisecond count
- records become plain JavaScript objects
- objects become JavaScript classes backed by UniFFI handles

## Current Limitations

The generator does not yet support:

- UniFFI custom types
- UniFFI external types
- timestamps in the public Node API
- durations in the public Node API
- automatic multi-target package assembly
