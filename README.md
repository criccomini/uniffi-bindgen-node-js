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

Install the CLI from crates.io:

```sh
cargo install uniffi-bindgen-node-js
```

If you want the current repository checkout instead:

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

`--crate-name` is optional when the built library exposes exactly one UniFFI component. If the library exposes multiple components, the generator reports the discovered crate names and requires `--crate-name`.

Use the built library file for your platform:

- macOS: `libyour_crate.dylib`
- Linux: `libyour_crate.so`
- Windows: `your_crate.dll`

3. Copy your built native library into the generated package.

By default, place it next to the generated JavaScript files:

```sh
cp path/to/your/built/library ./generated/your-package/
```

If you generated with `--bundled-prebuilds`, copy it into the expected `prebuilds/<target>/` path instead.

4. Install the generated package dependencies.

```sh
cd ./generated/your-package
npm install
```

You can then publish that directory as a package or install it into another app with `npm install ./generated/your-package`.

5. Consume the generated package from Node.

```js
import { greet } from "./generated/your-package/index.js";

console.log(greet("world"));
```

`--crate-name` accepts either the Cargo package name (`your-crate`) or the underscored library crate name (`your_crate`).

## CLI Reference

```sh
uniffi-bindgen-node-js generate [OPTIONS] --out-dir <OUT_DIR> <LIB_SOURCE>
```

Required inputs:

- `LIB_SOURCE`: path to a built `.so`, `.dylib`, or `.dll`
- `--out-dir`: directory where the npm package will be written

Optional component selection and source resolution:

- `--crate-name <name>`: Cargo package name or library crate name when the library exposes more than one UniFFI component
- `--manifest-path <Cargo.toml>`: Cargo.toml hint used when BindgenLoader needs workspace, UDL, or `uniffi.toml` resolution help

Optional Node package settings:

- `--package-name <name>`: npm package name to write into `package.json`
- `--node-engine <range>`: value written to `package.json` `engines.node`
- `--bundled-prebuilds`: resolve the packaged native library from `prebuilds/<host-target>/` instead of the package root
- `--manual-load`: export explicit `load()` and `unload()` helpers instead of auto-loading on import

Generated packages are always ESM. The CLI does not offer CommonJS output or legacy native-library path overrides.

## Packaging Modes

By default, the generated loader expects the native library at the package root next to the generated JavaScript files:

```text
your-package/
  index.js
  your_namespace.js
  your_namespace-ffi.js
  libyour_crate.dylib
```

If you pass `--bundled-prebuilds`, the generated loader expects the native library under `prebuilds/<host-target>/` for the current host build and resolves platform-specific libraries from that layout:

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

Each invocation describes one host-target build layout. Building a package that contains multiple targets still requires running the generator separately for each built cdylib and assembling the output layout yourself as part of your packaging process.

Linux bundled targets include a libc suffix:

- `-gnu` when Node reports glibc at runtime
- `-musl` otherwise

## Manual Loading

Without `--manual-load`, the generated package loads the native library at its default packaged path during import.

With `--manual-load`, the package exports `load()` and `unload()` so you control when loading happens. Calling `load()` with no argument uses the default packaged path; passing `load(path)` overrides it explicitly:

```sh
uniffi-bindgen-node-js generate \
  path/to/your/built/library \
  --crate-name your-crate \
  --out-dir ./generated/your-package \
  --manual-load
```

```js
import { load, unload, greet } from "./generated/your-package/index.js";

load();
console.log(greet("world"));
unload();
```

## `uniffi.toml` Configuration

You can store Node generator settings in `uniffi.toml`:

```toml
[bindings.node]
package_name = "your-package"
node_engine = ">=16"
bundled_prebuilds = true
manual_load = false
```

Defaults:

- `package_name`: UniFFI namespace
- `node_engine`: `>=16`
- `bundled_prebuilds`: `false`
- `manual_load`: `false`

CLI flags apply after config-file settings.

Generated packages are always ESM. The generator rejects legacy `[bindings.node]` keys such as `cdylib_name`, `lib_path_literal`, `module_format`, and `commonjs` with explicit diagnostics.

## Compatibility

- First-class target: UniFFI `0.31.x`
- Generated output: ESM-only
- Default Node engine range: `>=16`

## Supported UniFFI Surface

The generator currently supports:

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

Unsupported or not-yet-adopted UniFFI surfaces fail generation with an explicit diagnostic rather than producing partial bindings.

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
- CommonJS package output
- automatic multi-target package assembly
