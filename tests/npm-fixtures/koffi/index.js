import { readdirSync, readFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";

const TEXT_ENCODER = new TextEncoder();
const TEXT_DECODER = new TextDecoder();

const CALL_SUCCESS = 0;
const CALL_ERROR = 1;
const RUST_FUTURE_POLL_READY = 0;

function normalizeBigInt(value) {
  if (typeof value === "bigint") {
    return value;
  }
  if (typeof value === "number") {
    return BigInt(value);
  }
  throw new TypeError(`expected a bigint-compatible value, got ${typeof value}`);
}

function emptyRustBuffer() {
  return {
    capacity: 0n,
    len: 0n,
    data: null,
  };
}

function setCallSuccess(status) {
  if (status == null) {
    return;
  }
  status.code = CALL_SUCCESS;
  status.error_buf = emptyRustBuffer();
}

function setCallError(status, errorBuffer) {
  if (status == null) {
    return;
  }
  status.code = CALL_ERROR;
  status.error_buf = errorBuffer;
}

function toUint8Array(value, length) {
  if (value == null) {
    return new Uint8Array();
  }
  if (value instanceof Uint8Array) {
    return value.slice(0, length ?? value.byteLength);
  }
  if (ArrayBuffer.isView(value)) {
    const bytes = new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
    return bytes.slice(0, length ?? bytes.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    const bytes = new Uint8Array(value);
    return bytes.slice(0, length ?? bytes.byteLength);
  }
  if (Array.isArray(value)) {
    const bytes = Uint8Array.from(value);
    return bytes.slice(0, length ?? bytes.byteLength);
  }
  throw new TypeError(`unsupported byte source ${Object.prototype.toString.call(value)}`);
}

function foreignBytesToUint8Array(bytes) {
  const length = Number(bytes?.len ?? 0);
  return toUint8Array(bytes?.data, length);
}

function rustBufferToUint8Array(buffer) {
  const length = Number(buffer?.len ?? 0n);
  return toUint8Array(buffer?.data, length);
}

function rustBufferFromBytes(bytes) {
  const copied = Uint8Array.from(bytes);
  return {
    capacity: BigInt(copied.byteLength),
    len: BigInt(copied.byteLength),
    data: copied,
  };
}

class ByteWriter {
  constructor(length) {
    this.bytes = new Uint8Array(length);
    this.view = new DataView(this.bytes.buffer);
    this.offset = 0;
  }

  writeInt8(value) {
    this.view.setInt8(this.offset, value);
    this.offset += 1;
  }

  writeInt32(value) {
    this.view.setInt32(this.offset, value);
    this.offset += 4;
  }

  writeUInt32(value) {
    this.view.setUint32(this.offset, value);
    this.offset += 4;
  }

  writeUInt64(value) {
    this.view.setBigUint64(this.offset, normalizeBigInt(value));
    this.offset += 8;
  }

  writeBytes(value) {
    const bytes = toUint8Array(value);
    this.bytes.set(bytes, this.offset);
    this.offset += bytes.byteLength;
  }

  finish() {
    return this.bytes;
  }
}

class ByteReader {
  constructor(bytes) {
    this.bytes = toUint8Array(bytes);
    this.view = new DataView(
      this.bytes.buffer,
      this.bytes.byteOffset,
      this.bytes.byteLength,
    );
    this.offset = 0;
  }

  readInt8() {
    const value = this.view.getInt8(this.offset);
    this.offset += 1;
    return value;
  }

  readInt32() {
    const value = this.view.getInt32(this.offset);
    this.offset += 4;
    return value;
  }

  readUInt32() {
    const value = this.view.getUint32(this.offset);
    this.offset += 4;
    return value;
  }

  readUInt64() {
    const value = this.view.getBigUint64(this.offset);
    this.offset += 8;
    return value;
  }

  readBytes(length) {
    const end = this.offset + length;
    const slice = this.bytes.slice(this.offset, end);
    this.offset = end;
    return slice;
  }
}

function encodeString(value) {
  const bytes = TEXT_ENCODER.encode(value);
  const writer = new ByteWriter(4 + bytes.byteLength);
  writer.writeInt32(bytes.byteLength);
  writer.writeBytes(bytes);
  return writer.finish();
}

function decodeString(reader) {
  return TEXT_DECODER.decode(reader.readBytes(reader.readInt32()));
}

function decodeSerializedString(buffer) {
  return decodeString(new ByteReader(buffer));
}

function allocationSizeForBytes(value) {
  return 4 + toUint8Array(value).byteLength;
}

function writeBytesValue(writer, value) {
  const bytes = toUint8Array(value);
  writer.writeInt32(bytes.byteLength);
  writer.writeBytes(bytes);
}

function readBytesValue(reader) {
  return reader.readBytes(reader.readInt32());
}

function decodeSerializedBytes(buffer) {
  return readBytesValue(new ByteReader(buffer));
}

function encodeSerializedBytes(bytes) {
  const writer = new ByteWriter(allocationSizeForBytes(bytes));
  writeBytesValue(writer, bytes);
  return writer.finish();
}

function optionalBytesAllocationSize(value) {
  return value == null ? 1 : 1 + allocationSizeForBytes(value);
}

function writeOptionalBytes(writer, value) {
  if (value == null) {
    writer.writeInt8(0);
    return;
  }
  writer.writeInt8(1);
  writeBytesValue(writer, value);
}

function readOptionalBytes(reader) {
  const tag = reader.readInt8();
  if (tag === 0) {
    return undefined;
  }
  if (tag === 1) {
    return readBytesValue(reader);
  }
  throw new Error(`unexpected optional tag ${tag}`);
}

function blobRecordAllocationSize(record) {
  return (
    encodeString(record.name).byteLength +
    allocationSizeForBytes(record.value) +
    optionalBytesAllocationSize(record.maybe_value) +
    4 +
    record.chunks.reduce((total, chunk) => total + allocationSizeForBytes(chunk), 0)
  );
}

function encodeBlobRecord(record) {
  const writer = new ByteWriter(blobRecordAllocationSize(record));
  writer.writeBytes(encodeString(record.name));
  writeBytesValue(writer, record.value);
  writeOptionalBytes(writer, record.maybe_value);
  writer.writeInt32(record.chunks.length);
  for (const chunk of record.chunks) {
    writeBytesValue(writer, chunk);
  }
  return writer.finish();
}

function decodeBlobRecord(bytes) {
  const reader = new ByteReader(bytes);
  const chunkCountStart = {
    name: decodeString(reader),
    value: readBytesValue(reader),
    maybe_value: readOptionalBytes(reader),
  };
  const chunkCount = reader.readInt32();
  const chunks = new Array(chunkCount);
  for (let index = 0; index < chunkCount; index += 1) {
    chunks[index] = readBytesValue(reader);
  }
  return {
    ...chunkCountStart,
    chunks,
  };
}

function encodeFlavor(value) {
  const writer = new ByteWriter(4);
  writer.writeInt32(value === "chocolate" ? 2 : 1);
  return writer.finish();
}

function encodeScanResult(taggedValue) {
  if (taggedValue.tag === "Miss") {
    const writer = new ByteWriter(4);
    writer.writeInt32(2);
    return writer.finish();
  }
  const writer = new ByteWriter(4 + allocationSizeForBytes(taggedValue.value));
  writer.writeInt32(1);
  writeBytesValue(writer, taggedValue.value);
  return writer.finish();
}

function encodeFixtureErrorMissing() {
  const writer = new ByteWriter(4);
  writer.writeInt32(1);
  return writer.finish();
}

function encodeFixtureErrorInvalidState(message) {
  const encodedMessage = encodeString(message);
  const writer = new ByteWriter(4 + encodedMessage.byteLength);
  writer.writeInt32(2);
  writer.writeBytes(encodedMessage);
  return writer.finish();
}

function optionalStringAllocationSize(value) {
  return value == null ? 1 : 1 + encodeString(value).byteLength;
}

function writeOptionalString(writer, value) {
  if (value == null) {
    writer.writeInt8(0);
    return;
  }
  writer.writeInt8(1);
  writer.writeBytes(encodeString(value));
}

function writeOptionalUInt32(writer, value) {
  if (value == null) {
    writer.writeInt8(0);
    return;
  }
  writer.writeInt8(1);
  writer.writeUInt32(value);
}

function decodeOptionalHandle(bytes) {
  const reader = new ByteReader(bytes);
  const tag = reader.readInt8();
  if (tag === 0) {
    return undefined;
  }
  if (tag === 1) {
    return reader.readUInt64();
  }
  throw new Error(`unexpected optional handle tag ${tag}`);
}

function writeLogLevel(writer, level) {
  switch (level) {
    case "Off":
      writer.writeInt32(1);
      return;
    case "Error":
      writer.writeInt32(2);
      return;
    case "Warn":
      writer.writeInt32(3);
      return;
    case "Info":
      writer.writeInt32(4);
      return;
    case "Debug":
      writer.writeInt32(5);
      return;
    case "Trace":
      writer.writeInt32(6);
      return;
    default:
      throw new Error(`unexpected LogLevel ${String(level)}`);
  }
}

function decodeLogLevel(bytes) {
  const reader = new ByteReader(bytes);
  const tag = reader.readInt32();
  switch (tag) {
    case 1:
      return "Off";
    case 2:
      return "Error";
    case 3:
      return "Warn";
    case 4:
      return "Info";
    case 5:
      return "Debug";
    case 6:
      return "Trace";
    default:
      throw new Error(`unexpected LogLevel tag ${tag}`);
  }
}

function logRecordAllocationSize(record) {
  return (
    4 +
    encodeString(record.target).byteLength +
    encodeString(record.message).byteLength +
    optionalStringAllocationSize(record.module_path) +
    optionalStringAllocationSize(record.file) +
    (record.line == null ? 1 : 5)
  );
}

function encodeLogRecord(record) {
  const writer = new ByteWriter(logRecordAllocationSize(record));
  writeLogLevel(writer, record.level);
  writer.writeBytes(encodeString(record.target));
  writer.writeBytes(encodeString(record.message));
  writeOptionalString(writer, record.module_path);
  writeOptionalString(writer, record.file);
  writeOptionalUInt32(writer, record.line);
  return writer.finish();
}

function applyDottedJsonPath(root, key, value) {
  if (key.length === 0) {
    throw new Error("key cannot be empty");
  }

  let current = root;
  const parts = key.split(".");
  for (let index = 0; index < parts.length; index += 1) {
    const part = parts[index];
    if (part.length === 0) {
      throw new Error("key has an empty path segment");
    }

    const isLast = index === parts.length - 1;
    if (isLast) {
      current[part] = value;
      return;
    }

    const next = current[part];
    if (next == null) {
      current[part] = {};
      current = current[part];
      continue;
    }
    if (typeof next !== "object" || Array.isArray(next)) {
      throw new Error(`segment '${part}' is not an object`);
    }
    current = next;
  }
}

function cloneBlobRecord(record) {
  return {
    name: record.name,
    value: Uint8Array.from(record.value),
    maybe_value: record.maybe_value == null ? undefined : Uint8Array.from(record.maybe_value),
    chunks: record.chunks.map((chunk) => Uint8Array.from(chunk)),
  };
}

function parseFfiMetadata(libraryPath) {
  let packageDir = dirname(libraryPath);
  let ffiFileName = null;

  while (ffiFileName == null) {
    ffiFileName = readdirSync(packageDir).find((entry) => entry.endsWith("-ffi.js")) ?? null;
    if (ffiFileName != null) {
      break;
    }

    const parentDir = dirname(packageDir);
    if (parentDir === packageDir) {
      throw new Error(`failed to locate generated ffi module for ${libraryPath}`);
    }
    packageDir = parentDir;
  }

  const ffiSource = readFileSync(join(packageDir, ffiFileName), "utf8");
  const contractVersionMatch = ffiSource.match(/expectedContractVersion:\s*(\d+)/);
  const contractFunctionMatch = ffiSource.match(/contractVersionFunction:\s*"([^"]+)"/);
  if (contractVersionMatch == null || contractFunctionMatch == null) {
    throw new Error(`failed to parse ffi integrity metadata from ${ffiFileName}`);
  }

  const checksums = new Map();
  for (const match of ffiSource.matchAll(/"([^"]+)":\s*(\d+)/g)) {
    checksums.set(match[1], Number(match[2]));
  }

  return {
    contractVersion: Number(contractVersionMatch[1]),
    contractVersionFunction: contractFunctionMatch[1],
    checksums,
  };
}

function createBasicFixtureRuntime(libraryPath) {
  const metadata = parseFfiMetadata(libraryPath);
  const stores = new Map();
  const futures = new Map();
  let nextStoreHandle = 1n;
  let nextFutureHandle = 1000n;

  function getStore(handle) {
    const normalizedHandle = normalizeBigInt(handle);
    const store = stores.get(normalizedHandle);
    if (store == null) {
      throw new Error(`unknown Store handle ${normalizedHandle}`);
    }
    return store;
  }

  const handlers = new Map([
    [
      metadata.contractVersionFunction,
      () => metadata.contractVersion,
    ],
    [
      "ffi_fixture_basic_rustbuffer_alloc",
      (size, status) => {
        setCallSuccess(status);
        return rustBufferFromBytes(new Uint8Array(Number(size)));
      },
    ],
    [
      "ffi_fixture_basic_rustbuffer_from_bytes",
      (foreignBytes, status) => {
        setCallSuccess(status);
        return rustBufferFromBytes(foreignBytesToUint8Array(foreignBytes));
      },
    ],
    [
      "ffi_fixture_basic_rustbuffer_free",
      (_buffer, status) => {
        setCallSuccess(status);
      },
    ],
    [
      "ffi_fixture_basic_rustbuffer_reserve",
      (buffer, additional, status) => {
        const current = rustBufferToUint8Array(buffer);
        const reserved = new Uint8Array(current.byteLength + Number(additional));
        reserved.set(current);
        setCallSuccess(status);
        return rustBufferFromBytes(reserved);
      },
    ],
    [
      "uniffi_fixture_basic_fn_clone_store",
      (handle, status) => {
        setCallSuccess(status);
        return normalizeBigInt(handle);
      },
    ],
    [
      "uniffi_fixture_basic_fn_free_store",
      (handle, status) => {
        stores.delete(normalizeBigInt(handle));
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_fixture_basic_fn_constructor_store_new",
      (seedBuffer, status) => {
        const handle = nextStoreHandle;
        nextStoreHandle += 1n;
        stores.set(handle, decodeBlobRecord(rustBufferToUint8Array(seedBuffer)));
        setCallSuccess(status);
        return handle;
      },
    ],
    [
      "uniffi_fixture_basic_fn_method_store_current",
      (handle, status) => {
        setCallSuccess(status);
        return rustBufferFromBytes(encodeBlobRecord(cloneBlobRecord(getStore(handle))));
      },
    ],
    [
      "uniffi_fixture_basic_fn_method_store_replace",
      (handle, nextValueBuffer, status) => {
        const store = getStore(handle);
        const nextValue = decodeSerializedBytes(rustBufferToUint8Array(nextValueBuffer));
        const previous = Uint8Array.from(store.value);
        store.value = Uint8Array.from(nextValue);
        store.maybe_value = Uint8Array.from(nextValue);
        store.chunks = [...store.chunks, Uint8Array.from(nextValue)];
        setCallSuccess(status);
        return rustBufferFromBytes(encodeSerializedBytes(previous));
      },
    ],
    [
      "uniffi_fixture_basic_fn_method_store_flavor",
      (handle, status) => {
        const store = getStore(handle);
        const flavor = store.value.byteLength % 2 === 0 ? "vanilla" : "chocolate";
        setCallSuccess(status);
        return rustBufferFromBytes(encodeFlavor(flavor));
      },
    ],
    [
      "uniffi_fixture_basic_fn_method_store_inspect",
      (handle, includePayload, status) => {
        const store = getStore(handle);
        const result = includePayload
          ? { tag: "Hit", value: Uint8Array.from(store.value) }
          : { tag: "Miss" };
        setCallSuccess(status);
        return rustBufferFromBytes(encodeScanResult(result));
      },
    ],
    [
      "uniffi_fixture_basic_fn_method_store_require_value",
      (handle, present, status) => {
        if (present) {
          setCallSuccess(status);
          return rustBufferFromBytes(
            encodeSerializedBytes(Uint8Array.from(getStore(handle).value)),
          );
        }
        setCallError(status, rustBufferFromBytes(encodeFixtureErrorMissing()));
        return emptyRustBuffer();
      },
    ],
    [
      "uniffi_fixture_basic_fn_method_store_fetch_async",
      (handle, succeed) => {
        const futureHandle = nextFutureHandle;
        nextFutureHandle += 1n;
        if (succeed) {
          futures.set(futureHandle, {
            kind: "ok",
            payload: rustBufferFromBytes(encodeBlobRecord(cloneBlobRecord(getStore(handle)))),
          });
        } else {
          futures.set(futureHandle, {
            kind: "error",
            payload: rustBufferFromBytes(encodeFixtureErrorInvalidState("fetch failed")),
          });
        }
        return futureHandle;
      },
    ],
    [
      "ffi_fixture_basic_rust_future_poll_rust_buffer",
      (futureHandle, continuationCallback, continuationHandle) => {
        if (!futures.has(normalizeBigInt(futureHandle))) {
          throw new Error(`unknown Rust future handle ${futureHandle}`);
        }
        queueMicrotask(() => {
          continuationCallback(continuationHandle, RUST_FUTURE_POLL_READY);
        });
      },
    ],
    [
      "ffi_fixture_basic_rust_future_complete_rust_buffer",
      (futureHandle, status) => {
        const future = futures.get(normalizeBigInt(futureHandle));
        if (future == null) {
          throw new Error(`unknown Rust future handle ${futureHandle}`);
        }
        if (future.kind === "error") {
          setCallError(status, future.payload);
          return emptyRustBuffer();
        }
        setCallSuccess(status);
        return future.payload;
      },
    ],
    [
      "ffi_fixture_basic_rust_future_free_rust_buffer",
      (futureHandle) => {
        futures.delete(normalizeBigInt(futureHandle));
      },
    ],
    [
      "ffi_fixture_basic_rust_future_cancel_rust_buffer",
      (futureHandle) => {
        futures.delete(normalizeBigInt(futureHandle));
      },
    ],
    [
      "uniffi_fixture_basic_fn_func_echo_bytes",
      (valueBuffer, status) => {
        setCallSuccess(status);
        return rustBufferFromBytes(
          encodeSerializedBytes(decodeSerializedBytes(rustBufferToUint8Array(valueBuffer))),
        );
      },
    ],
    [
      "uniffi_fixture_basic_fn_func_echo_record",
      (recordBuffer, status) => {
        setCallSuccess(status);
        return rustBufferFromBytes(rustBufferToUint8Array(recordBuffer));
      },
    ],
  ]);

  for (const [checksumFunctionName, checksum] of metadata.checksums.entries()) {
    handlers.set(checksumFunctionName, () => checksum);
  }

  return handlers;
}

function createSlateDbRuntime(libraryPath) {
  const metadata = parseFfiMetadata(libraryPath);
  const settings = new Map();
  const writeBatches = new Map();
  let nextSettingsHandle = 1n;
  let nextWriteBatchHandle = 1000n;
  let logCallbackVtable = null;
  let mergeOperatorVtable = null;

  function getSettings(handle) {
    const normalizedHandle = normalizeBigInt(handle);
    const value = settings.get(normalizedHandle);
    if (value == null) {
      throw new Error(`unknown Settings handle ${normalizedHandle}`);
    }
    return value;
  }

  function getWriteBatch(handle) {
    const normalizedHandle = normalizeBigInt(handle);
    const value = writeBatches.get(normalizedHandle);
    if (value == null) {
      throw new Error(`unknown WriteBatch handle ${normalizedHandle}`);
    }
    return value;
  }

  const handlers = new Map([
    [
      metadata.contractVersionFunction,
      () => metadata.contractVersion,
    ],
    [
      "ffi_slatedb_uniffi_rustbuffer_alloc",
      (size, status) => {
        setCallSuccess(status);
        return rustBufferFromBytes(new Uint8Array(Number(size)));
      },
    ],
    [
      "ffi_slatedb_uniffi_rustbuffer_from_bytes",
      (foreignBytes, status) => {
        setCallSuccess(status);
        return rustBufferFromBytes(foreignBytesToUint8Array(foreignBytes));
      },
    ],
    [
      "ffi_slatedb_uniffi_rustbuffer_free",
      (_buffer, status) => {
        setCallSuccess(status);
      },
    ],
    [
      "ffi_slatedb_uniffi_rustbuffer_reserve",
      (buffer, additional, status) => {
        const current = rustBufferToUint8Array(buffer);
        const reserved = new Uint8Array(current.byteLength + Number(additional));
        reserved.set(current);
        setCallSuccess(status);
        return rustBufferFromBytes(reserved);
      },
    ],
    [
      "uniffi_slatedb_uniffi_fn_init_callback_vtable_logcallback",
      (vtable) => {
        logCallbackVtable = vtable;
      },
    ],
    [
      "uniffi_slatedb_uniffi_fn_init_callback_vtable_mergeoperator",
      (vtable) => {
        mergeOperatorVtable = vtable;
      },
    ],
    [
      "uniffi_slatedb_uniffi_fn_constructor_settings_default",
      (status) => {
        const handle = nextSettingsHandle;
        nextSettingsHandle += 1n;
        settings.set(handle, {
          flush_interval: "100ms",
        });
        setCallSuccess(status);
        return handle;
      },
    ],
    [
      "uniffi_slatedb_uniffi_fn_clone_settings",
      (handle, status) => {
        getSettings(handle);
        setCallSuccess(status);
        return normalizeBigInt(handle);
      },
    ],
    [
      "uniffi_slatedb_uniffi_fn_free_settings",
      (handle, status) => {
        settings.delete(normalizeBigInt(handle));
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_slatedb_uniffi_fn_method_settings_set",
      (handle, keyBuffer, valueJsonBuffer, status) => {
        const settingsValue = getSettings(handle);
        const key = decodeSerializedString(rustBufferToUint8Array(keyBuffer));
        const value = JSON.parse(
          decodeSerializedString(rustBufferToUint8Array(valueJsonBuffer)),
        );
        applyDottedJsonPath(settingsValue, key, value);
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_slatedb_uniffi_fn_method_settings_to_json_string",
      (handle, status) => {
        setCallSuccess(status);
        return rustBufferFromBytes(
          encodeString(JSON.stringify(getSettings(handle))),
        );
      },
    ],
    [
      "uniffi_slatedb_uniffi_fn_constructor_writebatch_new",
      (status) => {
        const handle = nextWriteBatchHandle;
        nextWriteBatchHandle += 1n;
        writeBatches.set(handle, []);
        setCallSuccess(status);
        return handle;
      },
    ],
    [
      "uniffi_slatedb_uniffi_fn_clone_writebatch",
      (handle, status) => {
        getWriteBatch(handle);
        setCallSuccess(status);
        return normalizeBigInt(handle);
      },
    ],
    [
      "uniffi_slatedb_uniffi_fn_free_writebatch",
      (handle, status) => {
        writeBatches.delete(normalizeBigInt(handle));
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_slatedb_uniffi_fn_method_writebatch_put",
      (handle, keyBuffer, valueBuffer, status) => {
        const batch = getWriteBatch(handle);
        batch.push({
          kind: "put",
          key: decodeSerializedBytes(rustBufferToUint8Array(keyBuffer)),
          value: decodeSerializedBytes(rustBufferToUint8Array(valueBuffer)),
        });
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_slatedb_uniffi_fn_method_writebatch_delete",
      (handle, keyBuffer, status) => {
        const batch = getWriteBatch(handle);
        batch.push({
          kind: "delete",
          key: decodeSerializedBytes(rustBufferToUint8Array(keyBuffer)),
        });
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_slatedb_uniffi_fn_func_init_logging",
      (levelBuffer, callbackBuffer, status) => {
        const level = decodeLogLevel(rustBufferToUint8Array(levelBuffer));
        const callbackHandle = decodeOptionalHandle(
          rustBufferToUint8Array(callbackBuffer),
        );

        if (callbackHandle != null) {
          if (logCallbackVtable?.log == null) {
            throw new Error("LogCallback vtable was not registered before init_logging");
          }

          const callbackStatus = {
            code: CALL_SUCCESS,
            error_buf: emptyRustBuffer(),
          };
          logCallbackVtable.log(
            callbackHandle,
            rustBufferFromBytes(
              encodeLogRecord({
                level,
                target: "slatedb",
                message: "logging initialized",
                module_path: "slatedb::logging",
                file: undefined,
                line: undefined,
              }),
            ),
            undefined,
            callbackStatus,
          );

          if (callbackStatus.code !== CALL_SUCCESS) {
            setCallError(status, callbackStatus.error_buf ?? emptyRustBuffer());
            return;
          }
        }

        if (mergeOperatorVtable?.merge == null && mergeOperatorVtable != null) {
          throw new Error("MergeOperator vtable was registered without a merge callback");
        }

        setCallSuccess(status);
      },
    ],
  ]);

  for (const [checksumFunctionName, checksum] of metadata.checksums.entries()) {
    handlers.set(checksumFunctionName, () => checksum);
  }

  return handlers;
}

function createFixtureHandlers(libraryPath) {
  const libraryName = basename(libraryPath);
  if (libraryName.includes("fixture_basic")) {
    return createBasicFixtureRuntime(libraryPath);
  }
  if (libraryName.includes("slatedb_uniffi")) {
    return createSlateDbRuntime(libraryPath);
  }
  return {
    get(name) {
      return (..._args) => {
        throw new Error(`mock koffi function ${name} is not implemented for ${libraryName}`);
      };
    },
  };
}

const koffi = {
  opaque() {
    return { kind: "opaque" };
  },
  pointer(typeOrName, maybeType) {
    return {
      kind: "pointer",
      name: maybeType == null ? null : typeOrName,
      to: maybeType ?? typeOrName,
    };
  },
  struct(name, fields) {
    return {
      kind: "struct",
      name,
      fields,
    };
  },
  proto(name, returnType, argumentTypes) {
    return {
      kind: "proto",
      name,
      returnType,
      argumentTypes,
    };
  },
  load(libraryPath) {
    const handlers = createFixtureHandlers(libraryPath);
    return {
      func(name) {
        const handler = handlers.get(name);
        if (handler != null) {
          return handler;
        }
        return (..._args) => {
          throw new Error(
            `mock koffi function ${name} is not implemented for ${basename(libraryPath)}`,
          );
        };
      },
      unload() {},
    };
  },
  as(value, _type) {
    return normalizeBigInt(value);
  },
  address(pointer) {
    return normalizeBigInt(pointer);
  },
  view(pointer, length) {
    return toUint8Array(pointer, length);
  },
  register(callback, _type) {
    return callback;
  },
  unregister(_callback) {},
  decode(value, _type) {
    return value;
  },
  encode(target, _type, value) {
    if (target == null) {
      return;
    }
    if (typeof value === "object" && value != null) {
      const source = target === value ? { ...value } : value;
      for (const key of Object.keys(target)) {
        delete target[key];
      }
      Object.assign(target, source);
      return;
    }
    target.value = value;
  },
};

export default koffi;
