mod support;

use self::support::{
    generate_fixture_package, install_fixture_package_dependencies_with_real_koffi, remove_dir_all,
    run_node_script,
};

#[test]
#[ignore = "requires npm registry access to install real koffi"]
fn runs_real_koffi_callback_smoke_script_against_generated_callback_fixture_package() {
    let generated = generate_fixture_package("callbacks");
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
import {{ foreignFutureHandleCount }} from "./runtime/callbacks.js";

{}
"#,
            real_koffi_callback_fixture_api_smoke_body()
        ),
    );

    remove_dir_all(&generated.built_fixture.workspace_dir);
    remove_dir_all(package_dir);
}

fn real_koffi_callback_fixture_api_smoke_body() -> &'static str {
    r#"function deferred() {
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

class SyncSink {
  constructor(prefix = "sync") {
    this.prefix = prefix;
    this.messages = [];
  }

  write(message) {
    this.messages.push(`${this.prefix}:${message}`);
  }

  latest() {
    return this.messages.at(-1);
  }
}

class LogCollector {
  constructor() {
    this.records = [];
  }

  log(record) {
    this.records.push(record);
  }
}

const sink = new SyncSink();
assert.equal(last_message(undefined), undefined);
emit(sink, "first");
emit(sink, "second");
assert.deepStrictEqual(sink.messages, ["sync:first", "sync:second"]);
assert.equal(last_message(sink), "sync:second");

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

const collector = new LogCollector();
init_logging(LogLevel.Info, collector);
assert.deepStrictEqual(collector.records, [
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

assert.equal(foreignFutureHandleCount(), 0);

const successWrite = deferred();
class AsyncSuccessSink {
  constructor() {
    this.messages = [];
  }

  async write(message) {
    this.messages.push({ message, self: this });
    return successWrite.promise;
  }

  async write_fallible(message) {
    return `fallible:${message}`;
  }

  async flush() {}
}

const successSink = new AsyncSuccessSink();
const successPromise = emit_async(successSink, "async-success");
assert.equal(foreignFutureHandleCount(), 1);
successWrite.resolve("async-success:done");
assert.equal(await successPromise, "async-success:done");
assert.deepStrictEqual(
  successSink.messages.map(({ message }) => message),
  ["async-success"],
);
assert.strictEqual(successSink.messages[0].self, successSink);
assert.equal(foreignFutureHandleCount(), 0);

const flushCompletion = deferred();
class FlushSink {
  constructor() {
    this.flushCalls = 0;
  }

  async write(message) {
    return message;
  }

  async write_fallible(message) {
    return message;
  }

  async flush() {
    this.flushCalls += 1;
    await flushCompletion.promise;
  }
}

const flushSink = new FlushSink();
const flushPromise = flush_async(flushSink);
assert.equal(flushSink.flushCalls, 1);
assert.equal(foreignFutureHandleCount(), 1);
flushCompletion.resolve();
await flushPromise;
assert.equal(foreignFutureHandleCount(), 0);

const typedFailure = deferred();
class TypedErrorSink {
  async write(message) {
    return message;
  }

  async write_fallible(message) {
    assert.equal(message, "typed-error");
    return typedFailure.promise;
  }

  async flush() {}
}

const typedErrorSink = new TypedErrorSink();
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
class UnexpectedErrorSink {
  async write(message) {
    return message;
  }

  async write_fallible(message) {
    assert.equal(message, "unexpected-error");
    return unexpectedFailure.promise;
  }

  async flush() {}
}

const unexpectedErrorSink = new UnexpectedErrorSink();
const unexpectedPromise = emit_async_fallible(unexpectedErrorSink, "unexpected-error");
assert.equal(foreignFutureHandleCount(), 1);
unexpectedFailure.reject("unexpected async rejection");
await assert.rejects(unexpectedPromise, (error) => {
  assert.equal(error.name, "RustPanic");
  assert.ok(error.message.includes("unexpected async rejection"));
  return true;
});
assert.equal(foreignFutureHandleCount(), 0);

const cancelledWrite = deferred();
class CancellationSink {
  constructor() {
    this.messages = [];
  }

  async write(message) {
    this.messages.push({ message, self: this });
    return cancelledWrite.promise;
  }

  async write_fallible(message) {
    return message;
  }

  async flush() {}
}

const cancellationSink = new CancellationSink();
cancel_emit_async(cancellationSink, "cancelled");
assert.deepStrictEqual(
  cancellationSink.messages.map(({ message }) => message),
  ["cancelled"],
);
assert.strictEqual(cancellationSink.messages[0].self, cancellationSink);
assert.equal(foreignFutureHandleCount(), 0);
cancelledWrite.resolve("ignored after cancellation");
await cancelledWrite.promise;
await nextTick();
assert.equal(foreignFutureHandleCount(), 0);"#
}
