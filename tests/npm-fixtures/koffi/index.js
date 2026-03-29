import { readdirSync, readFileSync } from "node:fs";
import { basename, dirname, join } from "node:path";

const TEXT_ENCODER = new TextEncoder();
const TEXT_DECODER = new TextDecoder();

const CALL_SUCCESS = 0;
const CALL_ERROR = 1;
const CALL_UNEXPECTED_ERROR = 2;
const CALL_CANCELLED = 3;
const RUST_FUTURE_POLL_READY = 0;
const RUST_FUTURE_POLL_WAKE = 1;
const REGISTERED_CALLBACKS = new Set();
let nextAnonymousPointerId = 1;

function requireRegisteredCallback(callback, context) {
  if (!REGISTERED_CALLBACKS.has(callback)) {
    throw new Error(`${context} requires a registered callback pointer`);
  }
  return callback;
}

function normalizeBigInt(value) {
  if (typeof value === "bigint") {
    return value;
  }
  if (typeof value === "number") {
    return BigInt(value);
  }
  if (typeof value === "object" && value != null && typeof value.__addr === "bigint") {
    return value.__addr;
  }
  throw new TypeError(`expected a bigint-compatible value, got ${typeof value}`);
}

function isPointerType(type) {
  return typeof type === "object" && type != null && type.kind === "pointer";
}

function isOpaquePointerType(type) {
  return isPointerType(type)
    && typeof type.to === "object"
    && type.to != null
    && type.to.kind === "opaque";
}

function anonymousPointerType() {
  return {
    kind: "pointer",
    name: `<anonymous_${nextAnonymousPointerId++}>`,
    to: { kind: "opaque" },
  };
}

function wrapPointerValue(value, type) {
  return {
    __addr: normalizeBigInt(value),
    __koffiPointer: true,
    __type: type,
  };
}

function wrapExternalPointerValue(value) {
  return {
    __addr: normalizeBigInt(value),
    __koffiExternalPointer: true,
  };
}

function wrapPointerCast(value, type) {
  return {
    __addr: normalizeBigInt(value.__addr),
    __koffiPointerCast: true,
    __pointer: value,
    __type: type,
  };
}

function pointerTypeName(type) {
  return type?.name ?? "pointer";
}

function isExternalPointerValue(value) {
  return typeof value === "bigint"
    || typeof value === "number"
    || value?.__koffiExternalPointer === true
    || (
      typeof value === "object"
      && value != null
      && typeof value.__addr === "bigint"
      && value.__type == null
    );
}

function validatePointerArgument(value, expectedType) {
  if (!isOpaquePointerType(expectedType) || value == null) {
    return;
  }

  if (expectedType.name === "RustArcPtr") {
    if (typeof value !== "object" || value == null || typeof value.__addr !== "bigint") {
      throw new TypeError(
        `Unexpected ${typeof value} value, expected ${pointerTypeName(expectedType)} *`,
      );
    }
    const actualType = value.__type;
    if (actualType == null) {
      return;
    }
    if (!isPointerType(actualType)) {
      throw new TypeError(
        `Unexpected ${typeof value} value, expected ${pointerTypeName(expectedType)} *`,
      );
    }
    if (actualType.name !== expectedType.name) {
      throw new TypeError(
        `Unexpected ${pointerTypeName(actualType)} * value, expected ${expectedType.name}`,
      );
    }
    return;
  }

  if (isExternalPointerValue(value)) {
    return;
  }

  if (value?.__koffiPointerCast === true) {
    const actualType = value.__type;
    if (!isPointerType(actualType)) {
      throw new TypeError(
        `Unexpected ${typeof value} value, expected ${pointerTypeName(expectedType)} *`,
      );
    }
    if (expectedType.name != null && actualType.name !== expectedType.name) {
      throw new TypeError(
        `Unexpected ${pointerTypeName(actualType)} * value, expected ${expectedType.name}`,
      );
    }
    return;
  }

  const actualType = value?.__type;
  if (!isPointerType(actualType)) {
    throw new TypeError(
      `Unexpected ${typeof value} value, expected ${pointerTypeName(expectedType)} *`,
    );
  }
  if (expectedType.name != null && actualType.name !== expectedType.name) {
    throw new TypeError(
      `Unexpected ${pointerTypeName(actualType)} * value, expected ${expectedType.name}`,
    );
  }
}

function wrapReturnValue(value, returnType) {
  if (!isOpaquePointerType(returnType) || value == null) {
    return value;
  }
  if (returnType.name === "RustArcPtr") {
    return wrapExternalPointerValue(value);
  }
  return wrapPointerValue(value, returnType);
}

function setHandleValue(map, handle, value) {
  map.set(normalizeBigInt(handle), {
    refCount: 1,
    value,
  });
}

function getHandleValue(map, handle, label) {
  const normalizedHandle = normalizeBigInt(handle);
  const entry = map.get(normalizedHandle);
  if (entry == null) {
    throw new Error(`unknown ${label} handle ${normalizedHandle}`);
  }
  return entry.value;
}

function cloneHandleValue(map, handle, label) {
  const normalizedHandle = normalizeBigInt(handle);
  const entry = map.get(normalizedHandle);
  if (entry == null) {
    throw new Error(`unknown ${label} handle ${normalizedHandle}`);
  }
  entry.refCount += 1;
  return normalizedHandle;
}

function freeHandleValue(map, handle) {
  const normalizedHandle = normalizeBigInt(handle);
  const entry = map.get(normalizedHandle);
  if (entry == null) {
    return;
  }
  if (entry.refCount <= 1) {
    map.delete(normalizedHandle);
    return;
  }
  entry.refCount -= 1;
}

function emptyRustBuffer() {
  return {
    capacity: 0n,
    len: 0n,
    data: null,
  };
}

function isAllocatedValue(value) {
  return typeof value === "object" && value != null && value.__koffiAlloc === true;
}

function cloneEncodedValue(value) {
  if (typeof value !== "object" || value == null) {
    return value;
  }
  return { ...value };
}

function readEncodedValue(value) {
  return isAllocatedValue(value)
    ? value.__koffiValue
    : value;
}

function writeEncodedValue(target, value) {
  if (!isAllocatedValue(target)) {
    return false;
  }
  target.__koffiValue = cloneEncodedValue(value);
  return true;
}

function setCallStatus(status, code, errorBuffer = emptyRustBuffer()) {
  if (status == null) {
    return;
  }
  if (writeEncodedValue(status, { code, error_buf: errorBuffer })) {
    return;
  }
  status.code = code;
  status.error_buf = errorBuffer;
}

function setCallSuccess(status) {
  setCallStatus(status, CALL_SUCCESS, emptyRustBuffer());
}

function setCallError(status, errorBuffer) {
  setCallStatus(status, CALL_ERROR, errorBuffer);
}

function setCallUnexpectedError(status, message) {
  setCallStatus(
    status,
    CALL_UNEXPECTED_ERROR,
    rustBufferFromUtf8String(String(message)),
  );
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

function rustBufferFromUtf8String(value) {
  return rustBufferFromBytes(TEXT_ENCODER.encode(value));
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

function decodeRustBufferString(buffer) {
  return TEXT_DECODER.decode(rustBufferToUint8Array(buffer));
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

function byteMapEntries(value) {
  if (value instanceof Map) {
    return Array.from(value.entries());
  }
  if (Array.isArray(value)) {
    return value;
  }
  return Array.from(value ?? []);
}

function byteMapAllocationSize(value) {
  return byteMapEntries(value).reduce(
    (total, [key, entryValue]) => {
      return total + encodeString(key).byteLength + allocationSizeForBytes(entryValue);
    },
    4,
  );
}

function encodeByteMap(value) {
  const entries = byteMapEntries(value);
  const writer = new ByteWriter(byteMapAllocationSize(entries));
  writer.writeInt32(entries.length);
  for (const [key, entryValue] of entries) {
    writer.writeBytes(encodeString(key));
    writeBytesValue(writer, entryValue);
  }
  return writer.finish();
}

function decodeByteMap(bytes) {
  const reader = new ByteReader(bytes);
  const entryCount = reader.readInt32();
  const map = new Map();
  for (let index = 0; index < entryCount; index += 1) {
    map.set(decodeString(reader), readBytesValue(reader));
  }
  return map;
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

function encodeFixtureErrorParse(message) {
  const encodedMessage = encodeString(message);
  const writer = new ByteWriter(4 + encodedMessage.byteLength);
  writer.writeInt32(3);
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

function encodeOptionalString(value) {
  const writer = new ByteWriter(optionalStringAllocationSize(value));
  writeOptionalString(writer, value);
  return writer.finish();
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

function createJsonObjectNode() {
  return {
    kind: "object",
    entries: new Map(),
  };
}

function createJsonRawNode(valueJson) {
  return {
    kind: "raw",
    valueJson,
  };
}

function setJsonPathValue(root, key, valueJson) {
  if (key.length === 0) {
    throw new Error("key cannot be empty");
  }

  const parts = key.split(".");
  let current = root;
  for (let index = 0; index < parts.length; index += 1) {
    const part = parts[index];
    if (part.length === 0) {
      throw new Error("key has an empty path segment");
    }

    const isLast = index === parts.length - 1;
    if (isLast) {
      current.entries.set(part, createJsonRawNode(valueJson));
      return;
    }

    const next = current.entries.get(part);
    if (next?.kind === "object") {
      current = next;
      continue;
    }

    const child = createJsonObjectNode();
    current.entries.set(part, child);
    current = child;
  }
}

function renderJsonNode(node) {
  if (node.kind === "raw") {
    return node.valueJson;
  }

  const keys = [...node.entries.keys()].sort();
  const items = keys.map((key) => `${JSON.stringify(key)}:${renderJsonNode(node.entries.get(key))}`);
  return `{${items.join(",")}}`;
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
  const configs = new Map();
  const readers = new Map();
  const readerBuilders = new Map();
  const metadata = parseFfiMetadata(libraryPath);
  const stores = new Map();
  const futures = new Map();
  let nextConfigHandle = 10_000n;
  let nextReaderHandle = 20_000n;
  let nextReaderBuilderHandle = 30_000n;
  let nextStoreHandle = 1n;
  let nextFutureHandle = 1000n;

  function getConfig(handle) {
    return getHandleValue(configs, handle, "Config");
  }

  function getReader(handle) {
    return getHandleValue(readers, handle, "Reader");
  }

  function getReaderBuilder(handle) {
    return getHandleValue(readerBuilders, handle, "ReaderBuilder");
  }

  function getStore(handle) {
    return getHandleValue(stores, handle, "Store");
  }

  function setPointerCallError(status, errorBuffer) {
    if (!isAllocatedValue(status)) {
      return;
    }
    setCallError(status, errorBuffer);
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
      "uniffi_fixture_basic_fn_clone_config",
      (handle, status) => {
        const clonedHandle = cloneHandleValue(configs, handle, "Config");
        setCallSuccess(status);
        return clonedHandle;
      },
    ],
    [
      "uniffi_fixture_basic_fn_free_config",
      (handle, status) => {
        freeHandleValue(configs, handle);
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_fixture_basic_fn_constructor_config_from_json",
      (jsonBuffer, status) => {
        const json = decodeRustBufferString(jsonBuffer);
        if (json !== "ok") {
          setPointerCallError(
            status,
            rustBufferFromBytes(encodeFixtureErrorParse("invalid json")),
          );
          return null;
        }

        const handle = nextConfigHandle;
        nextConfigHandle += 1n;
        setHandleValue(configs, handle, { value: json });
        setCallSuccess(status);
        return handle;
      },
    ],
    [
      "uniffi_fixture_basic_fn_method_config_value",
      (handle, status) => {
        setCallSuccess(status);
        return rustBufferFromUtf8String(getConfig(handle).value);
      },
    ],
    [
      "uniffi_fixture_basic_fn_clone_reader",
      (handle, status) => {
        const clonedHandle = cloneHandleValue(readers, handle, "Reader");
        setCallSuccess(status);
        return clonedHandle;
      },
    ],
    [
      "uniffi_fixture_basic_fn_free_reader",
      (handle, status) => {
        freeHandleValue(readers, handle);
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_fixture_basic_fn_method_reader_label",
      (handle, status) => {
        setCallSuccess(status);
        return rustBufferFromUtf8String(getReader(handle).label);
      },
    ],
    [
      "uniffi_fixture_basic_fn_method_reader_label_async",
      (handle) => {
        const futureHandle = nextFutureHandle;
        nextFutureHandle += 1n;
        futures.set(futureHandle, {
          kind: "rust_buffer",
          payload: rustBufferFromUtf8String(getReader(handle).label),
        });
        return futureHandle;
      },
    ],
    [
      "uniffi_fixture_basic_fn_clone_readerbuilder",
      (handle, status) => {
        const clonedHandle = cloneHandleValue(readerBuilders, handle, "ReaderBuilder");
        setCallSuccess(status);
        return clonedHandle;
      },
    ],
    [
      "uniffi_fixture_basic_fn_free_readerbuilder",
      (handle, status) => {
        freeHandleValue(readerBuilders, handle);
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_fixture_basic_fn_constructor_readerbuilder_new",
      (valid, status) => {
        const handle = nextReaderBuilderHandle;
        nextReaderBuilderHandle += 1n;
        setHandleValue(readerBuilders, handle, { valid: Boolean(valid) });
        setCallSuccess(status);
        return handle;
      },
    ],
    [
      "uniffi_fixture_basic_fn_method_readerbuilder_build",
      (handle) => {
        const futureHandle = nextFutureHandle;
        nextFutureHandle += 1n;
        const readerBuilder = getReaderBuilder(handle);

        if (readerBuilder.valid) {
          const readerHandle = nextReaderHandle;
          nextReaderHandle += 1n;
          setHandleValue(readers, readerHandle, { label: "ready" });
          futures.set(futureHandle, {
            kind: "pointer",
            payload: readerHandle,
          });
        } else {
          futures.set(futureHandle, {
            kind: "pointer_error",
            payload: rustBufferFromBytes(
              encodeFixtureErrorInvalidState("builder rejected"),
            ),
          });
        }

        return futureHandle;
      },
    ],
    [
      "uniffi_fixture_basic_fn_clone_store",
      (handle, status) => {
        const clonedHandle = cloneHandleValue(stores, handle, "Store");
        setCallSuccess(status);
        return clonedHandle;
      },
    ],
    [
      "uniffi_fixture_basic_fn_free_store",
      (handle, status) => {
        freeHandleValue(stores, handle);
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_fixture_basic_fn_constructor_store_new",
      (seedBuffer, status) => {
        const handle = nextStoreHandle;
        nextStoreHandle += 1n;
        setHandleValue(stores, handle, decodeBlobRecord(rustBufferToUint8Array(seedBuffer)));
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
      "ffi_fixture_basic_rust_future_poll_pointer",
      (futureHandle, continuationCallback, continuationHandle) => {
        if (!futures.has(normalizeBigInt(futureHandle))) {
          throw new Error(`unknown Rust future handle ${futureHandle}`);
        }
        const registeredContinuation = requireRegisteredCallback(
          continuationCallback,
          "ffi_fixture_basic_rust_future_poll_pointer",
        );
        queueMicrotask(() => {
          registeredContinuation(continuationHandle, RUST_FUTURE_POLL_READY);
        });
      },
    ],
    [
      "ffi_fixture_basic_rust_future_complete_pointer",
      (futureHandle, status) => {
        const future = futures.get(normalizeBigInt(futureHandle));
        if (future == null) {
          throw new Error(`unknown Rust future handle ${futureHandle}`);
        }
        if (future.kind === "pointer_error") {
          setPointerCallError(status, future.payload);
          return null;
        }
        setCallSuccess(status);
        return future.payload;
      },
    ],
    [
      "ffi_fixture_basic_rust_future_free_pointer",
      (futureHandle) => {
        futures.delete(normalizeBigInt(futureHandle));
      },
    ],
    [
      "ffi_fixture_basic_rust_future_cancel_pointer",
      (futureHandle) => {
        futures.delete(normalizeBigInt(futureHandle));
      },
    ],
    [
      "ffi_fixture_basic_rust_future_poll_rust_buffer",
      (futureHandle, continuationCallback, continuationHandle) => {
        if (!futures.has(normalizeBigInt(futureHandle))) {
          throw new Error(`unknown Rust future handle ${futureHandle}`);
        }
        const registeredContinuation = requireRegisteredCallback(
          continuationCallback,
          "ffi_fixture_basic_rust_future_poll_rust_buffer",
        );
        queueMicrotask(() => {
          registeredContinuation(continuationHandle, RUST_FUTURE_POLL_READY);
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
      "uniffi_fixture_basic_fn_func_echo_duration",
      (delayBuffer, status) => {
        setCallSuccess(status);
        return rustBufferFromBytes(rustBufferToUint8Array(delayBuffer));
      },
    ],
    [
      "uniffi_fixture_basic_fn_func_echo_record",
      (recordBuffer, status) => {
        setCallSuccess(status);
        return rustBufferFromBytes(rustBufferToUint8Array(recordBuffer));
      },
    ],
    [
      "uniffi_fixture_basic_fn_func_echo_timestamp",
      (whenBuffer, status) => {
        setCallSuccess(status);
        return rustBufferFromBytes(rustBufferToUint8Array(whenBuffer));
      },
    ],
    [
      "uniffi_fixture_basic_fn_func_echo_byte_map",
      (valueBuffer, status) => {
        setCallSuccess(status);
        return rustBufferFromBytes(
          encodeByteMap(decodeByteMap(rustBufferToUint8Array(valueBuffer))),
        );
      },
    ],
  ]);

  for (const [checksumFunctionName, checksum] of metadata.checksums.entries()) {
    handlers.set(checksumFunctionName, () => checksum);
  }

  return handlers;
}

function createCallbacksFixtureRuntime(libraryPath) {
  const metadata = parseFfiMetadata(libraryPath);
  const rustFutures = new Map();
  const settings = new Map();
  const writeBatches = new Map();
  let nextSettingsHandle = 1n;
  let nextWriteBatchHandle = 1000n;
  let nextRustFutureHandle = 10_000n;
  let asyncLogSinkVtable = null;
  let logCollectorVtable = null;
  let logSinkVtable = null;

  function getSettings(handle) {
    return getHandleValue(settings, handle, "Settings");
  }

  function getWriteBatch(handle) {
    return getHandleValue(writeBatches, handle, "WriteBatch");
  }

  function getRustFuture(handle) {
    const normalizedHandle = normalizeBigInt(handle);
    const future = rustFutures.get(normalizedHandle);
    if (future == null) {
      throw new Error(`unknown Rust future handle ${normalizedHandle}`);
    }
    return future;
  }

  function requireCallbackMethod(vtable, interfaceName, methodName, context) {
    const method = vtable?.[methodName];
    if (typeof method !== "function") {
      throw new Error(
        `${interfaceName} vtable method ${methodName} was not registered before ${context}`,
      );
    }
    return method;
  }

  function invokeLogCollectorCallback(handle, recordBuffer) {
    const callbackStatus = {
      code: CALL_SUCCESS,
      error_buf: emptyRustBuffer(),
    };
    requireCallbackMethod(
      logCollectorVtable,
      "LogCollector",
      "log",
      "invoking log collector callbacks",
    )(normalizeBigInt(handle), recordBuffer, undefined, callbackStatus);
    return callbackStatus;
  }

  function invokeLogSinkWriteCallback(handle, messageBuffer) {
    const callbackStatus = {
      code: CALL_SUCCESS,
      error_buf: emptyRustBuffer(),
    };
    requireCallbackMethod(
      logSinkVtable,
      "LogSink",
      "write",
      "invoking log sink write callbacks",
    )(normalizeBigInt(handle), messageBuffer, undefined, callbackStatus);
    return callbackStatus;
  }

  function invokeLogSinkLatestCallback(handle) {
    const callbackStatus = {
      code: CALL_SUCCESS,
      error_buf: emptyRustBuffer(),
    };
    const outReturn = {};
    requireCallbackMethod(
      logSinkVtable,
      "LogSink",
      "latest",
      "invoking log sink latest callbacks",
    )(normalizeBigInt(handle), outReturn, callbackStatus);
    return { callbackStatus, outReturn };
  }

  function invokeAsyncLogSinkCallback(handle, methodName, args, callbackData, completionCallback) {
    const outReturn = {};
    requireCallbackMethod(
      asyncLogSinkVtable,
      "AsyncLogSink",
      methodName,
      `invoking async log sink ${methodName} callbacks`,
    )(
      normalizeBigInt(handle),
      ...args,
      completionCallback,
      normalizeBigInt(callbackData),
      outReturn,
    );
    if (outReturn.handle == null || typeof outReturn.free !== "function") {
      throw new Error(
        `AsyncLogSink.${methodName} did not populate a ForeignFuture return handle`,
      );
    }
    return {
      free: outReturn.free,
      handle: normalizeBigInt(outReturn.handle),
    };
  }

  function freeCallbackHandle(vtable, interfaceName, handle) {
    requireCallbackMethod(
      vtable,
      interfaceName,
      "uniffi_free",
      `freeing ${interfaceName} handles`,
    )(normalizeBigInt(handle));
  }

  function freeForeignFuture(foreignFuture) {
    if (foreignFuture == null || typeof foreignFuture.free !== "function") {
      return;
    }
    foreignFuture.free(foreignFuture.handle);
  }

  function wakeRustFuture(future) {
    if (future.continuation == null) {
      return;
    }
    const continuation = future.continuation;
    future.continuation = null;
    queueMicrotask(() => {
      continuation.callback(continuation.handle, RUST_FUTURE_POLL_READY);
    });
  }

  function finishRustFuture(handle, result) {
    const future = rustFutures.get(normalizeBigInt(handle));
    if (future == null) {
      return;
    }

    freeForeignFuture(future.foreignFuture);
    future.foreignFuture = null;
    future.ready = true;
    future.statusCode = result?.call_status?.code ?? CALL_SUCCESS;
    future.errorBuffer = result?.call_status?.error_buf ?? emptyRustBuffer();
    if (future.kind === "rust_buffer") {
      future.returnValue = result?.return_value ?? emptyRustBuffer();
    }
    wakeRustFuture(future);
  }

  function failRustFuture(handle, error) {
    const future = rustFutures.get(normalizeBigInt(handle));
    if (future == null) {
      return;
    }

    future.ready = true;
    future.statusCode = CALL_UNEXPECTED_ERROR;
    future.errorBuffer = rustBufferFromUtf8String(String(error));
    if (future.kind === "rust_buffer") {
      future.returnValue = emptyRustBuffer();
    }
    wakeRustFuture(future);
  }

  function createRustFuture(kind, start) {
    const handle = nextRustFutureHandle;
    nextRustFutureHandle += 1n;
    rustFutures.set(handle, {
      continuation: null,
      errorBuffer: emptyRustBuffer(),
      foreignFuture: null,
      handle,
      kind,
      ready: false,
      returnValue: kind === "rust_buffer" ? emptyRustBuffer() : undefined,
      statusCode: CALL_SUCCESS,
    });

    try {
      const future = getRustFuture(handle);
      future.foreignFuture = start(handle);
    } catch (error) {
      failRustFuture(handle, error);
    }

    return handle;
  }

  const handlers = new Map([
    [
      metadata.contractVersionFunction,
      () => metadata.contractVersion,
    ],
    [
      "ffi_fixture_callbacks_rustbuffer_alloc",
      (size, status) => {
        setCallSuccess(status);
        return rustBufferFromBytes(new Uint8Array(Number(size)));
      },
    ],
    [
      "ffi_fixture_callbacks_rustbuffer_from_bytes",
      (foreignBytes, status) => {
        setCallSuccess(status);
        return rustBufferFromBytes(foreignBytesToUint8Array(foreignBytes));
      },
    ],
    [
      "ffi_fixture_callbacks_rustbuffer_free",
      (_buffer, status) => {
        setCallSuccess(status);
      },
    ],
    [
      "ffi_fixture_callbacks_rustbuffer_reserve",
      (buffer, additional, status) => {
        const current = rustBufferToUint8Array(buffer);
        const reserved = new Uint8Array(current.byteLength + Number(additional));
        reserved.set(current);
        setCallSuccess(status);
        return rustBufferFromBytes(reserved);
      },
    ],
    [
      "ffi_fixture_callbacks_rust_future_poll_rust_buffer",
      (futureHandle, continuationCallback, continuationHandle) => {
        const future = getRustFuture(futureHandle);
        const registeredContinuation = requireRegisteredCallback(
          continuationCallback,
          "ffi_fixture_callbacks_rust_future_poll_rust_buffer",
        );
        if (future.ready) {
          queueMicrotask(() => {
            registeredContinuation(continuationHandle, RUST_FUTURE_POLL_READY);
          });
          return;
        }
        future.continuation = {
          callback: registeredContinuation,
          handle: continuationHandle,
        };
      },
    ],
    [
      "ffi_fixture_callbacks_rust_future_complete_rust_buffer",
      (futureHandle, status) => {
        const future = getRustFuture(futureHandle);
        if (future.statusCode !== CALL_SUCCESS) {
          setCallStatus(status, future.statusCode, future.errorBuffer ?? emptyRustBuffer());
          return emptyRustBuffer();
        }
        setCallSuccess(status);
        return future.returnValue ?? emptyRustBuffer();
      },
    ],
    [
      "ffi_fixture_callbacks_rust_future_free_rust_buffer",
      (futureHandle) => {
        const normalizedHandle = normalizeBigInt(futureHandle);
        const future = rustFutures.get(normalizedHandle);
        if (future == null) {
          return;
        }
        freeForeignFuture(future.foreignFuture);
        rustFutures.delete(normalizedHandle);
      },
    ],
    [
      "ffi_fixture_callbacks_rust_future_cancel_rust_buffer",
      (futureHandle) => {
        const normalizedHandle = normalizeBigInt(futureHandle);
        const future = rustFutures.get(normalizedHandle);
        if (future == null) {
          return;
        }
        freeForeignFuture(future.foreignFuture);
        rustFutures.delete(normalizedHandle);
      },
    ],
    [
      "ffi_fixture_callbacks_rust_future_poll_void",
      (futureHandle, continuationCallback, continuationHandle) => {
        const future = getRustFuture(futureHandle);
        const registeredContinuation = requireRegisteredCallback(
          continuationCallback,
          "ffi_fixture_callbacks_rust_future_poll_void",
        );
        if (future.ready) {
          queueMicrotask(() => {
            registeredContinuation(continuationHandle, RUST_FUTURE_POLL_READY);
          });
          return;
        }
        future.continuation = {
          callback: registeredContinuation,
          handle: continuationHandle,
        };
      },
    ],
    [
      "ffi_fixture_callbacks_rust_future_complete_void",
      (futureHandle, status) => {
        const future = getRustFuture(futureHandle);
        if (future.statusCode !== CALL_SUCCESS) {
          setCallStatus(status, future.statusCode, future.errorBuffer ?? emptyRustBuffer());
          return;
        }
        setCallSuccess(status);
      },
    ],
    [
      "ffi_fixture_callbacks_rust_future_free_void",
      (futureHandle) => {
        const normalizedHandle = normalizeBigInt(futureHandle);
        const future = rustFutures.get(normalizedHandle);
        if (future == null) {
          return;
        }
        freeForeignFuture(future.foreignFuture);
        rustFutures.delete(normalizedHandle);
      },
    ],
    [
      "ffi_fixture_callbacks_rust_future_cancel_void",
      (futureHandle) => {
        const normalizedHandle = normalizeBigInt(futureHandle);
        const future = rustFutures.get(normalizedHandle);
        if (future == null) {
          return;
        }
        freeForeignFuture(future.foreignFuture);
        rustFutures.delete(normalizedHandle);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_clone_settings",
      (handle, status) => {
        const clonedHandle = cloneHandleValue(settings, handle, "Settings");
        setCallSuccess(status);
        return clonedHandle;
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_free_settings",
      (handle, status) => {
        freeHandleValue(settings, handle);
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_constructor_settings_default",
      (status) => {
        const handle = nextSettingsHandle;
        nextSettingsHandle += 1n;
        setHandleValue(settings, handle, createJsonObjectNode());
        setCallSuccess(status);
        return handle;
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_method_settings_set",
      (handle, keyBuffer, valueJsonBuffer, status) => {
        const settingsValue = getSettings(handle);
        const key = decodeRustBufferString(keyBuffer);
        const valueJson = decodeRustBufferString(valueJsonBuffer);
        setJsonPathValue(settingsValue, key, valueJson);
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_method_settings_to_json_string",
      (handle, status) => {
        setCallSuccess(status);
        return rustBufferFromUtf8String(renderJsonNode(getSettings(handle)));
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_clone_writebatch",
      (handle, status) => {
        const clonedHandle = cloneHandleValue(writeBatches, handle, "WriteBatch");
        setCallSuccess(status);
        return clonedHandle;
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_free_writebatch",
      (handle, status) => {
        freeHandleValue(writeBatches, handle);
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_constructor_writebatch_new",
      (status) => {
        const handle = nextWriteBatchHandle;
        nextWriteBatchHandle += 1n;
        setHandleValue(writeBatches, handle, []);
        setCallSuccess(status);
        return handle;
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_method_writebatch_put",
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
      "uniffi_fixture_callbacks_fn_method_writebatch_delete",
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
      "uniffi_fixture_callbacks_fn_method_writebatch_operation_count",
      (handle, status) => {
        setCallSuccess(status);
        return getWriteBatch(handle).length;
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_init_callback_vtable_logcollector",
      (vtable) => {
        logCollectorVtable = readEncodedValue(vtable);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_init_callback_vtable_asynclogsink",
      (vtable) => {
        asyncLogSinkVtable = readEncodedValue(vtable);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_clone_asynclogsink",
      (handle, status) => {
        setCallSuccess(status);
        return normalizeBigInt(handle);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_free_asynclogsink",
      (handle, status) => {
        freeCallbackHandle(asyncLogSinkVtable, "AsyncLogSink", handle);
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_clone_logcollector",
      (handle, status) => {
        setCallSuccess(status);
        return normalizeBigInt(handle);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_free_logcollector",
      (handle, status) => {
        freeCallbackHandle(logCollectorVtable, "LogCollector", handle);
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_method_logcollector_log",
      (handle, recordBuffer, status) => {
        const callbackStatus = invokeLogCollectorCallback(handle, recordBuffer);
        if (callbackStatus.code !== CALL_SUCCESS) {
          setCallError(status, callbackStatus.error_buf ?? emptyRustBuffer());
          return;
        }
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_init_callback_vtable_logsink",
      (vtable) => {
        logSinkVtable = readEncodedValue(vtable);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_clone_logsink",
      (handle, status) => {
        setCallSuccess(status);
        return normalizeBigInt(handle);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_free_logsink",
      (handle, status) => {
        freeCallbackHandle(logSinkVtable, "LogSink", handle);
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_method_logsink_write",
      (handle, messageBuffer, status) => {
        const callbackStatus = invokeLogSinkWriteCallback(handle, messageBuffer);
        if (callbackStatus.code !== CALL_SUCCESS) {
          setCallError(status, callbackStatus.error_buf ?? emptyRustBuffer());
          return;
        }
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_method_logsink_latest",
      (handle, status) => {
        const { callbackStatus, outReturn } = invokeLogSinkLatestCallback(handle);
        if (callbackStatus.code !== CALL_SUCCESS) {
          setCallError(status, callbackStatus.error_buf ?? emptyRustBuffer());
          return emptyRustBuffer();
        }
        setCallSuccess(status);
        return outReturn;
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_func_emit",
      (sinkHandle, messageBuffer, status) => {
        const callbackStatus = invokeLogSinkWriteCallback(sinkHandle, messageBuffer);
        if (callbackStatus.code !== CALL_SUCCESS) {
          setCallError(status, callbackStatus.error_buf ?? emptyRustBuffer());
          return;
        }
        setCallSuccess(status);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_func_emit_async",
      (sinkHandle, messageBuffer) =>
        createRustFuture("rust_buffer", (futureHandle) =>
          invokeAsyncLogSinkCallback(
            sinkHandle,
            "write",
            [messageBuffer],
            futureHandle,
            (callbackData, result) => {
              finishRustFuture(callbackData, result);
            },
          )),
    ],
    [
      "uniffi_fixture_callbacks_fn_func_emit_async_fallible",
      (sinkHandle, messageBuffer) =>
        createRustFuture("rust_buffer", (futureHandle) =>
          invokeAsyncLogSinkCallback(
            sinkHandle,
            "write_fallible",
            [messageBuffer],
            futureHandle,
            (callbackData, result) => {
              finishRustFuture(callbackData, result);
            },
          )),
    ],
    [
      "uniffi_fixture_callbacks_fn_func_flush_async",
      (sinkHandle) =>
        createRustFuture("void", (futureHandle) =>
          invokeAsyncLogSinkCallback(
            sinkHandle,
            "flush",
            [],
            futureHandle,
            (callbackData, result) => {
              finishRustFuture(callbackData, result);
            },
          )),
    ],
    [
      "uniffi_fixture_callbacks_fn_func_cancel_emit_async",
      (sinkHandle, messageBuffer, status) => {
        try {
          const foreignFuture = invokeAsyncLogSinkCallback(
            sinkHandle,
            "write",
            [messageBuffer],
            0n,
            () => {},
          );
          freeForeignFuture(foreignFuture);
          setCallSuccess(status);
        } catch (error) {
          setCallUnexpectedError(status, error);
        }
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_func_init_logging",
      (levelBuffer, collectorBuffer, status) => {
        const level = decodeLogLevel(rustBufferToUint8Array(levelBuffer));
        const collectorHandle = decodeOptionalHandle(
          rustBufferToUint8Array(collectorBuffer),
        );

        if (collectorHandle != null) {
          const callbackStatus = invokeLogCollectorCallback(
            collectorHandle,
            rustBufferFromBytes(
              encodeLogRecord({
                level,
                target: "callbacks_fixture",
                message: "logging initialized",
                module_path: "callbacks_fixture::logging",
                file: undefined,
                line: undefined,
              }),
            ),
          );

          if (callbackStatus.code !== CALL_SUCCESS) {
            setCallError(status, callbackStatus.error_buf ?? emptyRustBuffer());
            return;
          }
        }

        setCallSuccess(status);
      },
    ],
    [
      "uniffi_fixture_callbacks_fn_func_last_message",
      (sinkBuffer, status) => {
        const sinkHandle = decodeOptionalHandle(rustBufferToUint8Array(sinkBuffer));
        if (sinkHandle == null) {
          setCallSuccess(status);
          return rustBufferFromBytes(encodeOptionalString(undefined));
        }

        const { callbackStatus, outReturn } = invokeLogSinkLatestCallback(sinkHandle);
        if (callbackStatus.code !== CALL_SUCCESS) {
          setCallError(status, callbackStatus.error_buf ?? emptyRustBuffer());
          return emptyRustBuffer();
        }

        setCallSuccess(status);
        return outReturn;
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
  if (libraryName.includes("fixture_callbacks")) {
    return createCallbacksFixtureRuntime(libraryPath);
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
      func(name, returnType, argumentTypes = []) {
        const handler =
          handlers.get(name)
          ?? (name.endsWith("_generic_abi")
            ? handlers.get(name.slice(0, -"_generic_abi".length))
            : undefined);
        if (handler != null) {
          return (...args) => {
            for (let index = 0; index < argumentTypes.length; index += 1) {
              validatePointerArgument(args[index], argumentTypes[index]);
            }
            return wrapReturnValue(handler(...args), returnType);
          };
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
  alloc(type, length) {
    return {
      __koffiAlloc: true,
      __koffiLength: length,
      __koffiType: type,
      __koffiValue: null,
    };
  },
  as(value, type) {
    if (isOpaquePointerType(type)) {
      if (typeof value === "object" && value != null && value.__koffiPointerCast === true) {
        return wrapPointerCast(value.__pointer, type);
      }
      if (typeof value === "object" && value != null && value.__koffiPointer === true) {
        return wrapPointerCast(value, type);
      }
      if (isExternalPointerValue(value)) {
        return wrapPointerCast(wrapExternalPointerValue(value), type);
      }
      throw new TypeError("Invalid argument");
    }
    return normalizeBigInt(value);
  },
  address(pointer) {
    if (typeof pointer === "object" && pointer != null && pointer.__koffiPointerCast === true) {
      throw new TypeError(
        `Unexpected ${pointerTypeName(pointer.__type)} value for ptr, expected external pointer`,
      );
    }
    if (typeof pointer === "object" && pointer != null && pointer.__koffiPointer === true) {
      return normalizeBigInt(pointer.__addr);
    }
    return normalizeBigInt(pointer);
  },
  view(pointer, length) {
    return toUint8Array(pointer, length);
  },
  call(pointer, _type, ...args) {
    if (typeof pointer === "function") {
      return pointer(...args);
    }
    return requireRegisteredCallback(pointer, "koffi.call")(...args);
  },
  register(thisArgOrCallback, callbackOrType, _maybeType) {
    const callback =
      typeof thisArgOrCallback === "function"
        ? thisArgOrCallback
        : callbackOrType;
    const thisArg =
      typeof thisArgOrCallback === "function"
        ? undefined
        : thisArgOrCallback;
    const wrapped = (...args) => callback.apply(thisArg, args);
    REGISTERED_CALLBACKS.add(wrapped);
    return wrapped;
  },
  unregister(callback) {
    REGISTERED_CALLBACKS.delete(callback);
  },
  registeredCallbackCount() {
    return REGISTERED_CALLBACKS.size;
  },
  decode(value, type) {
    if (value instanceof BigUint64Array && isOpaquePointerType(type)) {
      return wrapPointerCast(wrapExternalPointerValue(value[0]), type);
    }
    return readEncodedValue(value);
  },
  encode(target, _type, value) {
    if (target == null) {
      return;
    }
    if (writeEncodedValue(target, value)) {
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
