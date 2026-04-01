# Development

This document is for contributors working on `uniffi-bindgen-node-js`. End-user installation and usage live in [README.md](./README.md).

## Local Setup

Build the project:

```sh
cargo build
```

Run the CLI from the repo:

```sh
cargo run -- --help
```

Inspect the generator subcommand:

```sh
cargo run -- generate --help
```

Run the v2 generator against a built cdylib:

```sh
cargo run -- generate path/to/libyour_fixture.dylib \
  --out-dir /tmp/uniffi-package
```

Pass `--manifest-path` when loader-based UDL or config resolution needs an explicit Cargo manifest hint:

```sh
cargo run -- generate path/to/libyour_fixture.dylib \
  --manifest-path path/to/Cargo.toml \
  --out-dir /tmp/uniffi-package
```

## Common Commands

Run the Rust test suite:

```sh
cargo test
```

Run a single test target while iterating:

```sh
cargo test --test node_package_generation
```

Run the ignored real-Koffi callback smoke test locally with Node 22 active and npm registry access available:

```sh
cargo test --locked --test node_real_koffi_tests -- --ignored
```

## Architecture

The crate now routes generation through a loader-based v2 pipeline instead of the old bindgen-orchestration path.

The high-level flow is:

1. `src/subcommands/generate.rs` parses CLI arguments and validates the user-facing surface.
2. `src/node_v2/mod.rs` builds `BindgenPaths`, constructs `BindgenLoader`, loads metadata and component interfaces from the built cdylib, loads per-component config, applies Node defaults and CLI overrides, applies rename config, selects one component, and calls `derive_ffi_funcs()`.
3. `src/bindings/package_writer.rs` turns the selected `ComponentInterface` into package files and delegates path decisions to `src/bindings/layout.rs`.
4. `src/bindings/api/`, `src/bindings/ffi.rs`, and `src/bindings/runtime.rs` render the package contents and shared runtime helpers.

Keep loader and config orchestration in `src/node_v2/`. Keep rendering code centered on `ComponentInterface` and the package writer. If a change mixes those responsibilities, split it before adding more behavior.

## Complexity

Measure Rust cyclomatic complexity with `lizard` and review the `CCN` column in the output:

```sh
uvx --from 'lizard==1.21.2' lizard -l rust src
```

This runs a pinned `lizard` version through `uvx`, limits parsing to Rust files with `-l rust`, and scans the repository's `src` tree.

## Leak Investigation

The repository includes manual leak probes for the generated Node runtime and the Rust generator process.

Runtime prep:

```sh
cargo run --bin runtime_leak_prep -- basic --out-dir /tmp/uniffi-basic-leaks
cargo run --bin runtime_leak_prep -- callbacks --out-dir /tmp/uniffi-callback-leaks
cargo run --bin runtime_leak_prep -- basic --manual-load --out-dir /tmp/uniffi-basic-manual-leaks
```

That helper binary builds the fixture cdylib in a temporary workspace, generates a package into `--out-dir`, stages the native library next to the generated JavaScript files, and runs `npm install --no-package-lock` unless you pass `--skip-npm-install`.

Run the runtime probes with Node 22 and `--expose-gc`:

```sh
UNIFFI_LEAK_PACKAGE_DIR=/tmp/uniffi-basic-leaks \
  /tmp/node-v22.22.2-darwin-arm64/bin/node --expose-gc \
  scripts/leaks/runtime-basic-soak.mjs

UNIFFI_LEAK_PACKAGE_DIR=/tmp/uniffi-basic-leaks \
  /tmp/node-v22.22.2-darwin-arm64/bin/node --expose-gc \
  scripts/leaks/runtime-basic-soak.mjs --scenario bytes

UNIFFI_LEAK_PACKAGE_DIR=/tmp/uniffi-basic-leaks \
  /tmp/node-v22.22.2-darwin-arm64/bin/node --expose-gc \
  scripts/leaks/runtime-basic-soak.mjs --scenario objects

UNIFFI_LEAK_PACKAGE_DIR=/tmp/uniffi-basic-leaks \
  /tmp/node-v22.22.2-darwin-arm64/bin/node --expose-gc \
  scripts/leaks/runtime-basic-soak.mjs --scenario async

UNIFFI_LEAK_PACKAGE_DIR=/tmp/uniffi-callback-leaks \
  /tmp/node-v22.22.2-darwin-arm64/bin/node --expose-gc \
  scripts/leaks/runtime-callback-soak.mjs

UNIFFI_LEAK_PACKAGE_DIR=/tmp/uniffi-basic-manual-leaks \
  /tmp/node-v22.22.2-darwin-arm64/bin/node --expose-gc \
  scripts/leaks/runtime-load-unload-soak.mjs

UNIFFI_LEAK_PACKAGE_DIR=/tmp/uniffi-basic-manual-leaks \
  /tmp/node-v22.22.2-darwin-arm64/bin/node --expose-gc \
  scripts/leaks/runtime-load-unload-soak.mjs --case reader-build
```

Use `--baseline-only` to capture an idle baseline and `--pause` to stop the process for live `leaks <pid>` inspection before exit.

`runtime-basic-soak.mjs` defaults to `--scenario full`. Use `--scenario bytes`, `--scenario objects`, or `--scenario async` to isolate the major operation families inside the basic fixture workload.

`runtime-load-unload-soak.mjs` defaults to `--case store-fetch`. Use `--case reader-build` to focus on manual-load cycles that build and destroy raw-external object handles across repeated `load()` / `unload()` boundaries.

For a smaller second-level bisect inside the basic probe:

```sh
UNIFFI_LEAK_PACKAGE_DIR=/tmp/uniffi-basic-leaks \
  /tmp/node-v22.22.2-darwin-arm64/bin/node --expose-gc \
  scripts/leaks/runtime-basic-soak.mjs --scenario bytes --case echo-bytes

UNIFFI_LEAK_PACKAGE_DIR=/tmp/uniffi-basic-leaks \
  /tmp/node-v22.22.2-darwin-arm64/bin/node --expose-gc \
  scripts/leaks/runtime-basic-soak.mjs --scenario bytes --case echo-record

UNIFFI_LEAK_PACKAGE_DIR=/tmp/uniffi-basic-leaks \
  /tmp/node-v22.22.2-darwin-arm64/bin/node --expose-gc \
  scripts/leaks/runtime-basic-soak.mjs --scenario bytes --case echo-byte-map

UNIFFI_LEAK_PACKAGE_DIR=/tmp/uniffi-basic-leaks \
  /tmp/node-v22.22.2-darwin-arm64/bin/node --expose-gc \
  scripts/leaks/runtime-basic-soak.mjs --scenario bytes --case temporal

UNIFFI_LEAK_PACKAGE_DIR=/tmp/uniffi-basic-leaks \
  /tmp/node-v22.22.2-darwin-arm64/bin/node --expose-gc \
  scripts/leaks/runtime-basic-soak.mjs --scenario async --case store-fetch

UNIFFI_LEAK_PACKAGE_DIR=/tmp/uniffi-basic-leaks \
  /tmp/node-v22.22.2-darwin-arm64/bin/node --expose-gc \
  scripts/leaks/runtime-basic-soak.mjs --scenario async --case reader-build

UNIFFI_LEAK_PACKAGE_DIR=/tmp/uniffi-basic-leaks \
  /tmp/node-v22.22.2-darwin-arm64/bin/node --expose-gc \
  scripts/leaks/runtime-basic-soak.mjs --scenario async --case reader-label
```

For at-exit leak reports on macOS:

```sh
UNIFFI_LEAK_PACKAGE_DIR=/tmp/uniffi-basic-leaks \
  leaks --atExit -- \
  /tmp/node-v22.22.2-darwin-arm64/bin/node --expose-gc \
  scripts/leaks/runtime-basic-soak.mjs
```

Generator leak probe:

```sh
cargo run --bin generator_leak_probe -- both --pause-after-warmup --pause-at-end
```

That helper binary builds the fixture cdylibs once, loops generation inside one long-lived Rust process, prints the process ID for `leaks <pid>`, and removes the per-iteration output directories after each cycle.

To inspect the normal CLI path at exit:

```sh
leaks --atExit -- cargo run -- generate \
  target/debug/libyour_fixture.dylib \
  --crate-name your_fixture \
  --out-dir /tmp/uniffi-generate-leaks
```

## CI And Publishing

GitHub Actions runs the full suite on pull requests and on every push to `main`.

The Linux workflow uses Node 22 because the callback benchmarks currently abort on newer Node releases, installs a global `tsc` binary for the generated-package TypeScript checks, prefetches fixture dependencies for the offline fixture builds, and then runs:

```sh
cargo test --locked
cargo test --locked -- --ignored
```

A separate lint job also enforces:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Separate macOS and Windows jobs run generated-package smoke coverage from `tests/node_smoke_tests.rs` for the basic, callback, bundled-prebuild, manual-load, and missing-prebuild cases.

After those CI jobs pass on a `main` push, a serialized release job publishes the version already in `Cargo.toml`, tags it as `v0.0.X`, creates the matching GitHub release, and then bumps `Cargo.toml` and `Cargo.lock` to the next `0.0.X` patch version with an automated `chore(release): prepare v0.0.X` commit back to `main`.

Only plain `0.0.X` versions are supported by the automation. If you need a different versioning scheme, change the workflow first.

Repository prerequisites for the automated release path:

- set `CARGO_REGISTRY_TOKEN` to a crates.io API token with publish access for `uniffi-bindgen-node-js`
- allow `github-actions[bot]` to push the automated `chore(release): prepare v0.0.X` commit to `main`
- allow the workflow token to create `v*` tags and GitHub releases
- keep the root crate version in `Cargo.toml` on the `0.0.X` line, because the release job only computes patch bumps there

Contributors do not need to push release tags by hand anymore. Merging to `main` is the release trigger.

## What The Tests Cover

Local tests cover:

- generator config parsing and output-path handling
- snapshot and codegen output for fixture crates
- generated package creation and dependency installation
- plain JavaScript smoke tests
- TypeScript declaration checks

Some real-runtime Node tests are intentionally ignored because they require registry access to install the actual `koffi` package.

## Repository Layout

- `src/subcommands/generate.rs`: CLI arguments and command execution
- `src/node_v2/`: loader-based generation orchestration, config normalization, component selection, path resolution, and validation
- `src/bindings/package_writer.rs`: package assembly and file emission orchestration
- `src/bindings/layout.rs`: generated package path layout decisions
- `src/bindings/api/`: high-level JavaScript and declaration emission
- `src/bindings/ffi.rs`: low-level FFI module generation
- `src/bindings/runtime.rs`: shared runtime helper emission
- `src/bin/`: contributor-facing helper binaries, including leak tooling
- `scripts/leaks/`: manual Node soak probes used during leak investigations
- `tests/`: snapshot, smoke, packaging, and regression tests
- `fixtures/`: UniFFI fixture crates used by tests

## Koffi Caveat

When running the real Koffi benchmark suites, use Node 22 or earlier for now. On Node versions newer than 22, the callback benchmarks can abort inside Koffi's synchronous callback path.

Upstream issue: <https://github.com/Koromix/koffi/issues/261>

Run locally with:

```sh
PATH=/opt/homebrew/opt/node@22/bin:$PATH cargo test --test node_benchmarks -- --ignored --nocapture
```

## Coverage

Install the LLVM coverage tooling once:

```sh
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov
```

Generate the HTML report:

```sh
cargo llvm-cov --html
```

Write LCOV output:

```sh
cargo llvm-cov report --lcov --output-path target/llvm-cov/lcov.info
```

Artifacts land here:

- HTML: `target/llvm-cov/html/index.html`
- LCOV: `target/llvm-cov/lcov.info`
