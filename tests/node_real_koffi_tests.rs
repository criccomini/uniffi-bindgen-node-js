mod support;

use self::support::{
    generate_fixture_package, install_fixture_package_dependencies_with_real_koffi, remove_dir_all,
    run_node_script, stage_fixture_package_native_library,
};

#[test]
#[ignore = "requires npm registry access to install real koffi"]
fn runs_real_koffi_callback_smoke_script_against_generated_callback_fixture_package() {
    let generated = generate_fixture_package("callbacks");
    stage_fixture_package_native_library(&generated);
    let package_dir = &generated.package_dir;

    install_fixture_package_dependencies_with_real_koffi(package_dir);
    run_node_script(
        package_dir,
        "real-koffi-callback-smoke.mjs",
        &format!(
            r#"
import assert from "node:assert/strict";
import {{
  AsyncLogErrorRejected,
  LogLevel,
  Settings,
  WriteBatch,
  cancel_emit_async,
  emit,
  emit_async,
  emit_async_fallible,
  flush_async,
  init_logging,
  last_message,
}} from "./index.js";
import {{
  UniffiCallbackRegistry,
  foreignFutureHandleCount,
  invokeAsyncCallbackMethod,
  writeCallbackError,
}} from "./runtime/callbacks.js";
import {{ EMPTY_RUST_BUFFER }} from "./runtime/ffi-types.js";
import {{
  CALL_ERROR,
  CALL_UNEXPECTED_ERROR,
  createRustCallStatus,
}} from "./runtime/rust-call.js";

{}
"#,
            real_koffi_callback_fixture_api_smoke_body()
        ),
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

fn real_koffi_callback_fixture_api_smoke_body() -> &'static str {
    r#"const messages = [];
const sink = {
  write(message) {
    messages.push(message);
  },
  latest() {
    return messages.at(-1);
  },
};

assert.equal(last_message(undefined), undefined);
emit(sink, "first");
emit(sink, "second");
assert.deepStrictEqual(messages, ["first", "second"]);
assert.equal(last_message(sink), "second");

const settings = Settings.default();
settings.set("writer.cache_size", "1024");
settings.set("logging.level", "\"debug\"");
settings.set("writer.enabled", "true");
assert.equal(
  settings.to_json_string(),
  "{\"logging\":{\"level\":\"debug\"},\"writer\":{\"cache_size\":1024,\"enabled\":true}}",
);

const batch = new WriteBatch();
batch.put(Buffer.from([1, 2]), Buffer.from([3, 4]));
batch.delete(Buffer.from([5]));
batch.put(Buffer.from("k"), Buffer.from("value"));
assert.equal(batch.operation_count(), 3);

const records = [];
const collector = {
  log(record) {
    records.push(record);
  },
};

init_logging(LogLevel.Info, collector);
assert.deepStrictEqual(records, [
  {
    level: LogLevel.Info,
    target: "callbacks_fixture",
    message: "logging initialized",
    module_path: "callbacks_fixture::logging",
    file: undefined,
    line: undefined,
  },
]);

init_logging(LogLevel.Info, undefined);

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

async function nextTick() {
  await new Promise((resolve) => setImmediate(resolve));
}

async function withTimeout(promise, message) {
  let timeoutId;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timeoutId = setTimeout(() => reject(new Error(message)), 1_000);
      }),
    ]);
  } finally {
    clearTimeout(timeoutId);
  }
}

assert.equal(foreignFutureHandleCount(), 0);

const callbackLowererErrors = [];
const callbackStatus = createRustCallStatus();
const normalizedCallbackError = writeCallbackError(callbackStatus, "sync string failure", {
  lowerError(error) {
    callbackLowererErrors.push(error);
    assert.ok(error instanceof Error);
    assert.equal(error.message, "sync string failure");
    return EMPTY_RUST_BUFFER;
  },
  lowerString(message) {
    assert.fail(`unexpected lowerString call for lowered sync error: ${message}`);
  },
});
assert.equal(callbackStatus.code, CALL_ERROR);
assert.ok(normalizedCallbackError instanceof Error);
assert.equal(normalizedCallbackError.message, "sync string failure");
assert.deepStrictEqual(
  callbackLowererErrors.map((error) => error.message),
  ["sync string failure"],
);

const unexpectedCallbackMessages = [];
const unexpectedCallbackStatus = createRustCallStatus();
const unexpectedCallbackError = writeCallbackError(
  unexpectedCallbackStatus,
  "sync unexpected failure",
  {
    lowerError(error) {
      assert.ok(error instanceof Error);
      assert.equal(error.message, "sync unexpected failure");
      return null;
    },
    lowerString(message) {
      unexpectedCallbackMessages.push(message);
      return EMPTY_RUST_BUFFER;
    },
  },
);
assert.equal(unexpectedCallbackStatus.code, CALL_UNEXPECTED_ERROR);
assert.ok(unexpectedCallbackError instanceof Error);
assert.equal(unexpectedCallbackError.message, "sync unexpected failure");
assert.deepStrictEqual(unexpectedCallbackMessages, ["sync unexpected failure"]);

const runtimeLowererErrors = [];
const runtimeCompletion = deferred();
const runtimeRegistry = new UniffiCallbackRegistry({
  interfaceName: "RuntimeAsyncSink",
});
const runtimeHandle = runtimeRegistry.register({
  write(message) {
    assert.equal(message, "runtime-string-failure");
    return Promise.reject("runtime string failure");
  },
});

const runtimeFutureHandle = invokeAsyncCallbackMethod({
  registry: runtimeRegistry,
  handle: runtimeHandle,
  methodName: "write",
  args: ["runtime-string-failure"],
  callbackData: 99n,
  complete(callbackData, result) {
    runtimeCompletion.resolve({ callbackData, result });
  },
  lowerError(error) {
    runtimeLowererErrors.push(error);
    assert.ok(error instanceof Error);
    assert.equal(error.message, "runtime string failure");
    return EMPTY_RUST_BUFFER;
  },
  lowerString(message) {
    assert.fail(`unexpected lowerString call for lowered async error: ${message}`);
  },
});
assert.equal(typeof runtimeFutureHandle, "bigint");
assert.equal(foreignFutureHandleCount(), 1);
const completedRuntimeFailure = await withTimeout(
  runtimeCompletion.promise,
  "timed out waiting for async callback failure completion",
);
assert.equal(completedRuntimeFailure.callbackData, 99n);
assert.equal(completedRuntimeFailure.result.call_status.code, CALL_ERROR);
assert.equal(foreignFutureHandleCount(), 0);
assert.deepStrictEqual(
  runtimeLowererErrors.map((error) => error.message),
  ["runtime string failure"],
);
runtimeRegistry.remove(runtimeHandle);

const successWrite = deferred();
const successSink = {
  write(message) {
    assert.equal(message, "async-success");
    return successWrite.promise;
  },
  write_fallible(message) {
    return Promise.resolve(`fallible:${message}`);
  },
  flush() {
    return Promise.resolve();
  },
};

const successPromise = emit_async(successSink, "async-success");
assert.equal(foreignFutureHandleCount(), 1);
successWrite.resolve("async-success:done");
assert.equal(await successPromise, "async-success:done");
assert.equal(foreignFutureHandleCount(), 0);

const flushCompletion = deferred();
let flushStarted = false;
const flushSink = {
  write(message) {
    return Promise.resolve(message);
  },
  write_fallible(message) {
    return Promise.resolve(message);
  },
  flush() {
    flushStarted = true;
    return flushCompletion.promise;
  },
};

const flushPromise = flush_async(flushSink);
assert.equal(flushStarted, true);
assert.equal(foreignFutureHandleCount(), 1);
flushCompletion.resolve();
await flushPromise;
assert.equal(foreignFutureHandleCount(), 0);

const typedFailure = deferred();
const typedErrorSink = {
  write(message) {
    return Promise.resolve(message);
  },
  write_fallible(message) {
    assert.equal(message, "typed-error");
    return typedFailure.promise;
  },
  flush() {
    return Promise.resolve();
  },
};

const typedPromise = emit_async_fallible(typedErrorSink, "typed-error");
assert.equal(foreignFutureHandleCount(), 1);
typedFailure.reject(new AsyncLogErrorRejected("typed rejection"));
await assert.rejects(typedPromise, (error) => {
  assert.ok(error instanceof AsyncLogErrorRejected);
  assert.equal(error.name, "AsyncLogErrorRejected");
  assert.equal(error.tag, "Rejected");
  assert.equal(error.message, "typed rejection");
  return true;
});
assert.equal(foreignFutureHandleCount(), 0);

const unexpectedFailure = deferred();
const unexpectedErrorSink = {
  write(message) {
    return Promise.resolve(message);
  },
  write_fallible(message) {
    assert.equal(message, "unexpected-error");
    return unexpectedFailure.promise;
  },
  flush() {
    return Promise.resolve();
  },
};

const unexpectedPromise = emit_async_fallible(unexpectedErrorSink, "unexpected-error");
assert.equal(foreignFutureHandleCount(), 1);
unexpectedFailure.reject("unexpected async rejection");
await assert.rejects(unexpectedPromise, (error) => {
  assert.equal(error.name, "RustPanic");
  assert.equal(
    error.message,
    'UnexpectedUniFFICallbackError(reason: "unexpected async rejection")',
  );
  return true;
});
assert.equal(foreignFutureHandleCount(), 0);

const cancelledWrite = deferred();
const cancellationWrites = [];
const cancellationSink = {
  write(message) {
    cancellationWrites.push(message);
    return cancelledWrite.promise;
  },
  write_fallible(message) {
    return Promise.resolve(message);
  },
  flush() {
    return Promise.resolve();
  },
};

cancel_emit_async(cancellationSink, "cancelled");
assert.deepStrictEqual(cancellationWrites, ["cancelled"]);
assert.equal(foreignFutureHandleCount(), 0);
cancelledWrite.resolve("ignored after cancellation");
await cancelledWrite.promise;
await nextTick();
assert.equal(foreignFutureHandleCount(), 0);"#
}
