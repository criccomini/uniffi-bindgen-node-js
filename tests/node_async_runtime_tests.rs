mod support;

use self::support::{
    generate_fixture_package, install_fixture_package_dependencies, remove_dir_all, run_node_script,
};

#[test]
fn generated_async_runtime_keeps_poll_promises_alive_until_callback_settles() {
    let generated = generate_fixture_package("basic");
    let package_dir = &generated.package_dir;

    install_fixture_package_dependencies(package_dir);
    run_node_script(
        package_dir,
        "async-runtime-smoke.mjs",
        r#"
import assert from "node:assert/strict";
import { pollRustFuture, RUST_FUTURE_POLL_READY } from "./runtime/async-rust-call.js";

const realSetTimeout = globalThis.setTimeout;
const realClearTimeout = globalThis.clearTimeout;
const timeoutCalls = [];
const clearedTimeouts = [];

try {
  globalThis.setTimeout = (_callback, delay, ...args) => {
    const token = { delay, args };
    timeoutCalls.push(token);
    return token;
  };
  globalThis.clearTimeout = (token) => {
    clearedTimeouts.push(token);
  };

  let seenContinuationHandle;
  const pollResult = await pollRustFuture(
    "fixture-future",
    (_rustFuture, continuationCallback, continuationHandle) => {
      seenContinuationHandle = continuationHandle;
      continuationCallback(continuationHandle, RUST_FUTURE_POLL_READY);
    },
  );

  assert.equal(pollResult, RUST_FUTURE_POLL_READY);
  assert.equal(typeof seenContinuationHandle, "bigint");
  assert.equal(timeoutCalls.length, 1);
  assert.equal(timeoutCalls[0].delay, 0x7fffffff);
  assert.deepStrictEqual(clearedTimeouts, [timeoutCalls[0]]);
} finally {
  globalThis.setTimeout = realSetTimeout;
  globalThis.clearTimeout = realClearTimeout;
}
"#,
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}
