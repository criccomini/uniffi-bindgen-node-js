# Repository Guidelines

## Project Structure & Module Organization
`src/` contains the Rust crate. Keep CLI entrypoints in `src/main.rs` and `src/subcommands/`, loader and config orchestration in `src/node/`, and package emission in `src/bindings/`. Contributor-only helper binaries live in `src/bin/`. Integration and regression tests live in `tests/`, with shared helpers in `tests/support/`, snapshot baselines in `tests/snapshots/`, and Node benchmarks in `tests/benchmarks/`. UniFFI fixture crates used by tests live in `fixtures/*-fixture`, and Askama templates live in `templates/`.

## Build, Test, and Development Commands
Use `cargo build` for a normal build. Use `cargo run -- --help` or `cargo run -- generate --help` to exercise the CLI locally. Run the main suite with `cargo test --locked`; narrow iteration with a single target such as `cargo test --test node_package_generation`. Run ignored runtime and benchmark coverage with `cargo test --locked -- --ignored`; this expects Node 22 for real Koffi paths. Match CI before opening a PR with `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings`.

## Coding Style & Naming Conventions
This crate uses Rust 2024 edition and standard `rustfmt` formatting with 4-space indentation. Follow Rust naming defaults: `snake_case` for modules, files, and functions; `PascalCase` for types; `SCREAMING_SNAKE_CASE` for constants. Keep `src/node/` focused on loader, path, and config concerns, and keep rendering centered in `src/bindings/`. Generated package and CLI names use kebab-case, for example `uniffi-bindgen-node-js`.

## Testing Guidelines
Prefer integration tests in `tests/*.rs` for user-visible behavior. Snapshot coverage uses `insta`; refresh snapshots with `INSTA_UPDATE=always cargo test --test codegen_snapshots` and review changes in `tests/snapshots/`. When changing generated package behavior, add or update fixture-based tests and, if relevant, TypeScript or smoke coverage. There is no separate coverage percentage target, but CI must pass the default suite, ignored tests on Linux, and generated-package smoke checks on macOS and Windows.

## Commit & Pull Request Guidelines
- Use single-line, conventional commit syntax for all commits.
- Commit after every changes.
- Keep commits narrowly scoped and easy to review.
