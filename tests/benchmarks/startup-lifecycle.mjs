import { Bench } from "tinybench";

import { startupBenchOptions, printBenchResults, runChildScript } from "./common.mjs";

const eagerPackageDir = process.cwd();
const manualPackageDir = process.env.UNIFFI_MANUAL_PACKAGE_DIR;

if (manualPackageDir == null || manualPackageDir === "") {
  throw new Error("UNIFFI_MANUAL_PACKAGE_DIR must be set");
}

const bench = new Bench(startupBenchOptions("Generated Package Startup And Lifecycle"));

bench.add("cold import (eager load package)", async () => {
  await runChildScript(eagerPackageDir, "benchmarks/children/cold-import.mjs");
});
bench.add("cold import + load() (manual load package)", async () => {
  await runChildScript(manualPackageDir, "benchmarks/children/cold-manual-load.mjs");
});
bench.add("load() + unload() (fresh process)", async () => {
  await runChildScript(manualPackageDir, "benchmarks/children/load-unload.mjs");
});

await bench.run();
printBenchResults(bench);
