const moduleExports = await import("../../index.js");

moduleExports.load();
if (moduleExports.unload() !== true) {
  throw new Error("expected unload() to report a loaded library");
}
