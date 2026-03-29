# Prompt For Codex: Iterative Performance Work In This Repo

## Summary

Use the repo’s existing benchmark harness as the source of truth. Measure a baseline, make one performance change at a time, rerun the same benchmark suite, compare before/
after on the same harness, then rerun the full perf suite and run cargo test last if code changed.

## Prompt

You are working in the `uniffi-bindgen-node-js` repo. Iteratively improve performance, and use the existing perf tests in this repo as the only scoring mechanism for whether a
change helped.

Benchmark source of truth:
- Rust driver: `tests/node_benchmarks.rs`
- JS benchmark suites:
  - `tests/benchmarks/basic-hot-path.mjs`
  - `tests/benchmarks/callback-hot-path.mjs`
  - `tests/benchmarks/startup-lifecycle.mjs`
- Benchmark helpers/options: `tests/benchmarks/common.mjs`

How the perf tests actually run:
- Use the Rust test harness, not the `.mjs` files directly from the repo root.
- The harness builds fixture `cdylib`s, generates temporary npm packages, stages the benchmark scripts into those generated packages, runs `npm install --no-package-lock`, and
then executes Node with `--expose-gc`.
- These benchmark tests are `#[ignore]` by default because they require npm registry access to install real `koffi` and `tinybench`.

Commands:
- List available perf tests:
  `cargo test --test node_benchmarks -- --ignored --list`
- Run the full perf suite and print benchmark output:
  `cargo test --test node_benchmarks -- --ignored --nocapture`
- Run individual suites:
  `cargo test --test node_benchmarks benchmarks_basic_generated_package_hot_paths -- --ignored --exact --nocapture`
  `cargo test --test node_benchmarks benchmarks_callback_generated_package_hot_paths -- --ignored --exact --nocapture`
  `cargo test --test node_benchmarks benchmarks_generated_package_startup_and_lifecycle -- --ignored --exact --nocapture`

What each suite measures:
- `benchmarks_basic_generated_package_hot_paths`: generated-package runtime hot paths for bytes, records, maps, object construction/methods, async methods, typed-error paths,
builders, and readers.
- `benchmarks_callback_generated_package_hot_paths`: generated-package callback/runtime hot paths for sync sinks, async sinks, settings serialization, and write-batch
operations.
- `benchmarks_generated_package_startup_and_lifecycle`: cold-process startup/lifecycle for eager import, manual `load()`, and `load() + unload()` using fresh child processes.

Benchmark tuning knobs:
- Hot-path suites read:
  - `UNIFFI_BENCH_TIME_MS`
  - `UNIFFI_BENCH_ITERATIONS`
  - `UNIFFI_BENCH_WARMUP_TIME_MS`
  - `UNIFFI_BENCH_WARMUP_ITERATIONS`
- Startup suite reads:
  - `UNIFFI_BENCH_STARTUP_TIME_MS`
  - `UNIFFI_BENCH_STARTUP_ITERATIONS`
- If you change any benchmark env vars, keep them identical between baseline and comparison runs and report the exact values used.

How to evaluate performance:
- Establish a before-change baseline with the exact suite you want to improve.
- After each code change, rerun the same suite with the same command and same env.
- Use the `tinybench` table printed to stdout as the comparison source. Compare the same named benchmark rows before vs. after. Do not compare numbers across different suites.
- Treat results as relative before/after measurements on the same machine and same harness. The current harness uses the repo’s existing dev/test flow, so use it for
comparison, not as an absolute production benchmark.
- Before finishing, rerun the full perf suite to check for regressions outside the targeted area.
- If one benchmark improves and another regresses, report that tradeoff explicitly.
- Do not change the benchmark harness, fixtures, or benchmark parameters just to improve reported numbers. If a harness change is truly required, separate it from performance
claims and justify it.

Working loop:
1. Run a baseline benchmark.
2. Make one focused performance change.
3. Rerun the relevant benchmark suite and compare against baseline.
4. Repeat while the benchmark data shows real improvement.
5. Rerun the full perf suite before stopping.
6. If you made code changes, run `cargo test` as the last step.

Reporting requirements:
- Show the benchmark command(s) used.
- Show before/after results for the benchmark rows that materially changed.
- Call out regressions and inconclusive/noisy results.
- State whether the full perf suite was rerun.
- State whether `cargo test` was run last and whether it passed.
- If benchmarks cannot run because npm registry access is unavailable, say so explicitly and do not claim unmeasured performance improvements.

## Test Plan

- Baseline with the targeted perf suite.
- Re-measure the same suite after each iteration.
- Rerun the full perf suite before stopping.
- Run cargo test last if any code changed.

## Assumptions

- Use the current perf harness as-is.
- Do not redefine success criteria or add new perf tests unless explicitly asked.
- The repo’s current perf coverage is runtime behavior of generated fixture packages, and performance claims should be scoped to that.

## Instructions

- never ever change any PROMPT.md
- commit after each change
- use conventional commit syntax for commit messages
