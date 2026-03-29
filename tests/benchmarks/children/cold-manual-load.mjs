const moduleExports = await import("../../index.js");
const bindings = moduleExports.load();

if (bindings == null || typeof bindings !== "object") {
  throw new Error("expected load() to return generated ffi bindings");
}
