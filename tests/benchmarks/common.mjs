import { execFile } from "node:child_process";
import { resolve } from "node:path";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const retainedValues = [];

export function readIntEnv(name, fallback) {
  const raw = process.env[name];
  if (raw == null || raw === "") {
    return fallback;
  }

  const value = Number.parseInt(raw, 10);
  if (!Number.isFinite(value) || value < 0) {
    throw new Error(`${name} must be a non-negative integer, got ${JSON.stringify(raw)}`);
  }

  return value;
}

export function hotPathBenchOptions(name) {
  return {
    name,
    throws: true,
    time: readIntEnv("UNIFFI_BENCH_TIME_MS", 250),
    iterations: readIntEnv("UNIFFI_BENCH_ITERATIONS", 10),
    warmup: true,
    warmupTime: readIntEnv("UNIFFI_BENCH_WARMUP_TIME_MS", 100),
    warmupIterations: readIntEnv("UNIFFI_BENCH_WARMUP_ITERATIONS", 3),
  };
}

export function startupBenchOptions(name) {
  return {
    name,
    throws: true,
    time: readIntEnv("UNIFFI_BENCH_STARTUP_TIME_MS", 50),
    iterations: readIntEnv("UNIFFI_BENCH_STARTUP_ITERATIONS", 3),
    warmup: false,
  };
}

export function blackBox(value) {
  retainedValues[0] = value;
  return value;
}

export function maybeGc() {
  globalThis.gc?.();
}

export function printBenchResults(bench) {
  console.log(`\n## ${bench.name}`);
  console.table(bench.table());
}

export async function runChildScript(packageDir, scriptRelativePath) {
  const scriptPath = resolve(packageDir, scriptRelativePath);
  await execFileAsync(process.execPath, [scriptPath], {
    cwd: packageDir,
    env: process.env,
  });
}
