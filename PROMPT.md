You are working in the `uniffi-bindgen-node-js` repo. Iteratively improve performance, and use the repo’s existing perf tests as the only scoring mechanism for whether a
change helped.

Use these files as the source of truth:
- `tests/node_benchmarks.rs`
- `tests/benchmarks/basic-hot-path.mjs`
- `tests/benchmarks/callback-hot-path.mjs`
- `tests/benchmarks/startup-lifecycle.mjs`
- `tests/benchmarks/common.mjs`
- `tests/support/mod.rs`

How perf tests actually run:
- Use the Rust test harness for measurement, not ad hoc scripts from the repo root.
- The harness builds fixture `cdylib`s, generates temporary npm packages, stages `tests/benchmarks` into those packages, runs `npm install --no-package-lock`, and executes
Node with `--expose-gc`.
- These perf tests are `#[ignore]` by default because they require npm registry access to install real `koffi` and `tinybench`.

Perf commands:
- List available perf tests:
  `cargo test --test node_benchmarks -- --ignored --list`
- Run the full perf suite:
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
- `benchmarks_generated_package_startup_and_lifecycle`: cold-process startup/lifecycle for eager import, manual `load()`, and `load() + unload()`.

Benchmark knobs:
- Hot-path suites use:
  `UNIFFI_BENCH_TIME_MS`
  `UNIFFI_BENCH_ITERATIONS`
  `UNIFFI_BENCH_WARMUP_TIME_MS`
  `UNIFFI_BENCH_WARMUP_ITERATIONS`
- Startup suite uses:
  `UNIFFI_BENCH_STARTUP_TIME_MS`
  `UNIFFI_BENCH_STARTUP_ITERATIONS`
- Keep env vars identical between baseline and comparison runs, and report the exact values used.

How to evaluate performance:
- Establish a baseline with the exact suite you are targeting.
- After each code change, rerun the same suite with the same command and same env.
- Compare only the same named benchmark rows before vs. after from the printed `tinybench` table.
- Treat results as relative before/after measurements on the same machine and same harness.
- Rerun the full perf suite before stopping.
- If one row improves and another regresses, report that tradeoff explicitly.
- Do not change the benchmark harness, fixtures, or benchmark parameters just to improve reported numbers.

Hotspot visibility is mandatory:
- Do not rely only on benchmark deltas. For every targeted perf test, capture hotspot data before changing code and again after a material performance shift.
- Save a raw `.cpuprofile` artifact and also provide a terminal summary of the hottest files, functions, and call stacks.
- Use Node’s built-in CPU profiler with `--cpu-prof --cpu-prof-dir <dir> --cpu-prof-name <name>.cpuprofile`.

How to profile without changing the measurement harness:
- Use the Rust perf harness for scoring.
- For profiling, reproduce the same generated-package setup in a retained temp directory by following `tests/support/mod.rs`: build the relevant fixture `cdylib`, generate the
package, stage `tests/benchmarks` into it, run `npm install --no-package-lock`, and execute the benchmark logic from inside that generated package.
- For hot-path suites, if you need per-benchmark visibility, create a temporary profiling driver that mirrors the exact setup and body of the specific `bench.add(...)` case
you are investigating, then run that driver under `node --cpu-prof`. Do not treat that driver as the scoring harness; it is only for hotspot diagnosis.
- For the startup suite, profile the child scripts directly rather than `benchmarks/startup-lifecycle.mjs`, because that benchmark measures fresh child processes:
  `benchmarks/children/cold-import.mjs`
  `benchmarks/children/cold-manual-load.mjs`
  `benchmarks/children/load-unload.mjs`
- Report the saved profile path for each targeted test and summarize the hottest stacks/files/functions from the profile output.

Working loop:
1. Run a baseline benchmark.
2. Capture hotspot visibility for the targeted test.
3. Make one focused performance change.
4. Rerun the same benchmark suite and compare against baseline.
5. Refresh hotspot visibility if the bottleneck moved or the result materially changed.
6. Repeat while measured results improve.
7. Rerun the full perf suite before stopping.
8. If you made code changes, run `cargo test` last.

Reporting requirements:
- Show the benchmark command(s) used.
- Show the exact env var values used for benchmark timing.
- Show before/after results for benchmark rows that materially changed.
- Show the hotspot artifact path(s) and a concise hotspot summary for each targeted test.
- Call out regressions, noisy results, and inconclusive runs explicitly.
- State whether the full perf suite was rerun.
- State whether `cargo test` was run last and whether it passed.
- If benchmarks cannot run because npm registry access is unavailable, say so explicitly and do not claim unmeasured performance improvements.

Constraints:
- Use the current perf harness as-is.
- Do not add new perf tests or redefine success criteria unless explicitly asked.
- Do not claim a performance improvement without benchmark evidence from this repo’s existing perf suite.

## Scenarios

- Basic hot-path suite
- Callback hot-path suite
- Startup/lifecycle suite, with direct child-process profiling for hotspot visibility

## Assumptions

- Node’s built-in CPU profiler is available via node --cpu-prof.
- Raw .cpuprofile artifacts plus CLI hotspot summaries are required.
- Perf claims stay scoped to this repo’s existing generated-package benchmark coverage.

## Instructions

- never ever change any PROMPT.md
- commit after each change
- use conventional commit syntax for commit messages
