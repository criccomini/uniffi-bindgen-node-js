import assert from "node:assert/strict";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const [, , packageDirArg, iterationsArg = "1", mode = "workload"] = process.argv;

if (packageDirArg == null) {
  throw new Error("package directory argument is required");
}

const packageDir = resolve(packageDirArg);
const iterations = Number.parseInt(iterationsArg, 10);
if (!Number.isInteger(iterations) || iterations <= 0) {
  throw new Error(`iterations must be positive, got ${JSON.stringify(iterationsArg)}`);
}

const api = await import(pathToFileURL(resolve(packageDir, "index.js")).href);
const ffi = await import(pathToFileURL(resolve(packageDir, "fixture-ffi.js")).href);

for (let index = 0; index < iterations; index += 1) {
  api.load();
  assert.equal(ffi.isLoaded(), true);

  if (mode === "workload") {
    const store = new api.Store({
      name: `child-${index}`,
      value: new Uint8Array([1, index % 256]),
      maybe_value: undefined,
      chunks: [new Uint8Array([2, 3])],
    });
    await store.fetch_async(true);
    store.dispose();
    api.echo_bytes(new Uint8Array([4, 5, index % 256]));
  }

  assert.equal(api.unload(), true);
  assert.equal(ffi.isLoaded(), false);
}

if (typeof globalThis.gc === "function") {
  globalThis.gc();
}
