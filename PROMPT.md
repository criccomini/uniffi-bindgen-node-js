# Support Async Callback-Interface Methods

## Summary

- Implement UniFFI 0.29 async callback-interface support through ForeignFuture, not the Rust-future polling path.
- Remove the current v1 diagnostic rejecting async callback-interface methods.
- Keep generated Node callback interface typings strict: async methods return Promise<T> / Promise<void>.
- Apply the change to both explicit callback interfaces and object-backed foreign traits, since this generator already models both through the same callback-interface path.

## Public API / Interface Changes

- Generated .d.ts callback interfaces render async methods as method(...): Promise<T> / Promise<void>.
- Generated JS proxy classes render corresponding lifted callback methods as async.
- No new public Node callback API parameters are added; cancellation remains internal via UniFFI foreign-future free(handle).

## Implementation Changes

- Generator/model:
    - Remove the unsupported-feature validation for async callback-interface methods.
    - Extend callback-method metadata with async foreign-future completion identifiers, result-struct shape, and a default lowered return value for async error completion.
    - Render callback proxy methods with the existing async object-method path when method.is_async.
    - Split callback vtable generation into sync and async branches.
- Async callback vtable generation:
    - Emit Koffi callback signatures as handle, args..., futureCallback, callbackData, outReturn for async callback methods.
    - In the async branch, synchronously allocate and write a ForeignFuture into outReturn, then complete it later through the supplied UniFFI completion callback.
    - Build completion payloads with the existing ForeignFutureStruct* FFI types for the method’s return family.
- Runtime:
    - Add async callback helpers to templates/runtime/callbacks.js.j2 and declarations to templates/runtime/callbacks.d.ts.j2.
    - Maintain a pending foreign-future handle map holding promise lifecycle state, completion callback data, and cancellation state.
    - Reuse the existing callback error lowering rules for rejected promises: typed error -> CALL_ERROR; other rejection -> CALL_UNEXPECTED_ERROR with lowered string message.
    - Suppress late completion after free(handle) or runtime unload.
    - Expose a handle-count helper for tests, mirroring the existing rust-future runtime test hook.
- FFI defaults:
    - Add generator-side rendering for default lowered return values required by ForeignFutureResult<T> on error paths for non-void async callback methods.
    - Use zero/empty defaults appropriate to the UniFFI 0.29 FFI family used here: numeric 0, handles/pointers 0n, and buffers EMPTY_RUST_BUFFER.
- Fixtures and mocks:
    - Extend the callback fixture with async callback-interface coverage for success, typed error, unexpected error, void, and cancellation.
    - Extend the mock koffi runtime to simulate async callback invocation, ForeignFuture completion callbacks, and cancellation cleanup.

## Test Plan

- Rust unit tests:
    - async callback interfaces no longer fail validation,
    - callback interface DTS renders Promise<...> methods,
    - public API JS renders async proxy methods for async callback interfaces,
    - FFI JS renders async callback signatures with ForeignFutureComplete* and ForeignFutureStruct*.
- Snapshot tests:
    - regenerate callback fixture snapshots for component JS, DTS, and FFI output.
- Node smoke tests:
    - async callback success with return value,
    - async callback void completion,
    - typed rejection mapped back to the generated typed Rust error,
    - unexpected rejection mapped to the unexpected callback error path,
    - cancellation where Rust drops the foreign future before the JS promise settles,
    - handle-count cleanup after success, error, and cancellation.
- TypeScript tests:
    - strict Promise<...> callback interface typing,
    - callback implementations typecheck for the async fixture surface.

## Assumptions

- Generated Node callback interfaces require Promise<T> / Promise<void> for async methods.
- Cancellation is internal only; no new public AbortSignal or options object is added to generated callback method signatures.
- Promise settlement after cancellation or unload is ignored.
- This work targets the repo’s pinned UniFFI 0.29.5 ABI.

## Instructions

You are in a Ralph Wiggum loop. Work through the first few TODOs in the `## TODO` section below.

- update PROMPT.md with an updated TODO list after each change
- never ever change any PROMPT.md text _except_ the items in the `## TODO` section
- you may update the TODO items as you see fit--remove outdated items, add new items, mark items as completed
- commit after each completed TODO
- use conventional commit syntax for commit messages
- if there are no items left in TODO.md, append a final line in PROMPT.md that contains only the emoji: ✅

## TODO

- [x] Remove the async callback-interface unsupported-feature validation from component model building.
- [x] Extend callback method metadata to capture foreign-future completion identifiers and async result-struct requirements.
- [x] Add generator support for default lowered FFI return values used in async callback error completion structs.
- [x] Render callback proxy methods as async when the callback-interface method is async.
- [x] Split callback vtable registration code into synchronous and asynchronous callback generation paths.
- [x] Generate async callback Koffi callback signatures with futureCallback, callbackData, and outReturn ForeignFuture.
- [x] Add runtime helpers for pending foreign futures in templates/runtime/callbacks.js.j2.
- [ ] Add runtime type declarations for async callback helpers in templates/runtime/callbacks.d.ts.j2.
- [ ] Implement async callback invocation that writes ForeignFuture immediately and completes later from promise settlement.
- [ ] Reuse existing callback error lowering for typed and unexpected promise rejections.
- [ ] Ignore late async callback completions after foreign-future free or runtime unload.
- [ ] Add a test-only foreign-future handle-count helper for async callback runtime state.
- [ ] Extend the callback fixture with async callback-interface methods covering success, error, void, and cancellation cases.
- [ ] Extend the mock Koffi runtime to emulate async callback invocation, completion callbacks, and cancellation cleanup.
- [ ] Add Rust unit tests for async callback model validation and rendered public API output.
- [ ] Update snapshot tests for generated callback fixture output.
- [ ] Add Node smoke tests for async callback success, typed error, unexpected error, void completion, and cancellation.
- [ ] Add TypeScript tests that enforce Promise<...> async callback interface method typing.
- [ ] Run the full relevant test suite after implementation.
