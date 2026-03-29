const moduleExports = await import("../../index.js");

if (typeof moduleExports.echo_bytes !== "function") {
  throw new Error("expected generated eager-load package to export echo_bytes");
}
