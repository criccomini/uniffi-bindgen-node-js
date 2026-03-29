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
- `src/bindings/mod.rs`: generation orchestration and package writing
- `src/bindings/api/`: high-level JavaScript and declaration emission
- `src/bindings/ffi.rs`: low-level FFI module generation
- `tests/`: snapshot, smoke, packaging, and regression tests
- `fixtures/`: UniFFI fixture crates used by tests

## Koffi Caveat

When running the real Koffi benchmark suites, use Node 22 or earlier for now. On Node versions newer than 22, the callback benchmarks can abort inside Koffi's synchronous callback path.

Upstream issue: <https://github.com/Koromix/koffi/issues/261>

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
