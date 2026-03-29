import { once } from "node:events";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

export const MiB = 1024 * 1024;

export function parseProbeArgs(argv = process.argv.slice(2), extraArguments = {}) {
  const options = {
    baselineOnly: false,
    pause: false,
    batches: 100,
    opsPerBatch: 1000,
    childIterations: 10,
    help: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    switch (arg) {
      case "--baseline-only":
        options.baselineOnly = true;
        break;

      case "--pause":
        options.pause = true;
        break;

      case "--batches":
        options.batches = readPositiveInt("--batches", argv, ++index);
        break;

      case "--ops-per-batch":
        options.opsPerBatch = readPositiveInt("--ops-per-batch", argv, ++index);
        break;

      case "--child-iterations":
        options.childIterations = readPositiveInt("--child-iterations", argv, ++index);
        break;

      case "--help":
      case "-h":
        options.help = true;
        break;

      default:
        if (Object.hasOwn(extraArguments, arg)) {
          const handler = extraArguments[arg];
          if (typeof handler !== "function") {
            throw new Error(`Invalid handler for extra argument ${JSON.stringify(arg)}.`);
          }
          index = handler(options, argv, index);
          break;
        }
        throw new Error(`Unknown argument ${JSON.stringify(arg)}.`);
    }
  }

  return options;
}

export function requirePackageDir() {
  const packageDir = process.env.UNIFFI_LEAK_PACKAGE_DIR;
  if (packageDir == null || packageDir === "") {
    throw new Error("UNIFFI_LEAK_PACKAGE_DIR must be set.");
  }
  return resolve(packageDir);
}

export async function importPackageModule(packageDir, relativePath) {
  return import(pathToFileURL(resolve(packageDir, relativePath)).href);
}

export function warnIfNodeVersionExceeds(maxSupportedMajor = 22) {
  const major = Number.parseInt(process.versions.node.split(".")[0], 10);
  if (Number.isInteger(major) && major > maxSupportedMajor) {
    console.warn(
      `[warn] Node ${process.versions.node} is newer than the recommended major ${maxSupportedMajor} for real Koffi leak probes.`,
    );
  }
}

export async function forceGc(cycles = 2) {
  if (typeof globalThis.gc !== "function") {
    throw new Error("Run the leak probe with Node's --expose-gc flag.");
  }

  for (let index = 0; index < cycles; index += 1) {
    globalThis.gc();
    await nextTick();
  }
}

export function nextTick() {
  return new Promise((resolve) => setImmediate(resolve));
}

export function snapshotMemory(label, phase, batch) {
  const usage = process.memoryUsage();
  return Object.freeze({
    label,
    phase,
    batch,
    heapUsed: usage.heapUsed,
    external: usage.external,
    rss: usage.rss,
    arrayBuffers: usage.arrayBuffers,
  });
}

export function printMemorySnapshot(snapshot) {
  console.log(
    `[memory] ${snapshot.label} heapUsed=${formatBytes(snapshot.heapUsed)} external=${formatBytes(snapshot.external)} rss=${formatBytes(snapshot.rss)} arrayBuffers=${formatBytes(snapshot.arrayBuffers)}`,
  );
}

export function printMemorySummary(label, samples) {
  if (samples.length === 0) {
    return;
  }

  const first = samples[0];
  const last = samples.at(-1);
  console.log(
    `[summary] ${label} heapUsed=${formatDelta(last.heapUsed - first.heapUsed)} external=${formatDelta(last.external - first.external)} rss=${formatDelta(last.rss - first.rss)} arrayBuffers=${formatDelta(last.arrayBuffers - first.arrayBuffers)}`,
  );
}

export function findLeakCandidates(samples, thresholds = defaultThresholds(), window = 10) {
  if (samples.length < window) {
    return [];
  }

  const tail = samples.slice(-window);
  const candidates = [];
  for (const [field, minDelta] of Object.entries(thresholds)) {
    let strictlyIncreasing = true;
    for (let index = 1; index < tail.length; index += 1) {
      if (tail[index][field] <= tail[index - 1][field]) {
        strictlyIncreasing = false;
        break;
      }
    }
    const delta = tail.at(-1)[field] - tail[0][field];
    if (strictlyIncreasing && delta >= minDelta) {
      candidates.push({ field, delta });
    }
  }

  return candidates;
}

export function assertNoLeakCandidates(label, samples, thresholds = defaultThresholds()) {
  const candidates = findLeakCandidates(samples, thresholds);
  if (candidates.length === 0) {
    return;
  }

  throw new Error(
    `${label} showed sustained growth: ${candidates.map(
      ({ field, delta }) => `${field}=${formatBytes(delta)}`,
    ).join(", ")}`,
  );
}

export function medianOfTail(samples, field, count = 10) {
  if (samples.length === 0) {
    return 0;
  }

  const values = samples
    .slice(-Math.min(count, samples.length))
    .map((sample) => sample[field])
    .sort((left, right) => left - right);
  const middle = Math.floor(values.length / 2);
  if (values.length % 2 === 1) {
    return values[middle];
  }
  return Math.round((values[middle - 1] + values[middle]) / 2);
}

export function assertPlateauClose(
  label,
  leftSamples,
  rightSamples,
  field,
  maxDelta,
) {
  if (leftSamples.length === 0 || rightSamples.length === 0) {
    return;
  }

  const leftMedian = medianOfTail(leftSamples, field);
  const rightMedian = medianOfTail(rightSamples, field);
  const delta = Math.abs(rightMedian - leftMedian);
  if (delta > maxDelta) {
    throw new Error(
      `${label} diverged for ${field}: ${formatBytes(delta)} > ${formatBytes(maxDelta)}`,
    );
  }
}

export async function maybePauseForInspection(enabled, label) {
  if (!enabled) {
    return;
  }

  console.error(
    `[pause] ${label}\n[pause] pid=${process.pid}\n[pause] Run: leaks ${process.pid}\n[pause] Press Enter to continue.`,
  );
  process.stdin.resume();
  await once(process.stdin, "data");
  process.stdin.pause();
}

function defaultThresholds() {
  return Object.freeze({
    heapUsed: 8 * MiB,
    external: 8 * MiB,
    rss: 32 * MiB,
    arrayBuffers: 8 * MiB,
  });
}

function formatDelta(bytes) {
  const prefix = bytes < 0 ? "-" : "+";
  return `${prefix}${formatBytes(Math.abs(bytes))}`;
}

export function formatBytes(bytes) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < MiB) {
    return `${(bytes / 1024).toFixed(1)} KiB`;
  }
  return `${(bytes / MiB).toFixed(2)} MiB`;
}

function readPositiveInt(flag, argv, index) {
  const raw = argv[index];
  if (raw == null) {
    throw new Error(`${flag} requires an integer value.`);
  }

  const value = Number.parseInt(raw, 10);
  if (!Number.isInteger(value) || value <= 0) {
    throw new Error(`${flag} must be a positive integer, got ${JSON.stringify(raw)}.`);
  }
  return value;
}

export function readRequiredString(flag, argv, index) {
  const raw = argv[index];
  if (raw == null || raw === "") {
    throw new Error(`${flag} requires a non-empty value.`);
  }
  return raw;
}
