# Public Node Temporal Types

## Summary

- Expose UniFFI timestamp as Date and duration as number in the public Node API, matching the runtime aliases that already exist in templates/runtime/ffi-
  converters.d.ts.j2:1.
- Keep scope limited to public API wiring, tests, snapshots, and docs. Do not redesign the existing runtime or wire representation in this pass.

## Key Changes

- Update src/bindings/api/support.rs:93 so render_public_type returns Date for Type::Timestamp and number for Type::Duration instead of rejecting them.
- Let that shared mapping flow through every public surface that already depends on render_public_type: function params and returns, object constructors and methods, record
  fields, enum and error payloads, and callback interface signatures.
- Replace the rejection coverage in src/bindings/api/mod.rs:1029 with positive unit tests for direct and nested timestamp/duration rendering, plus at least one
  render_public_api() case that proves the generated API imports and uses FfiConverterTimestamp and FfiConverterDuration.
- Extend the existing basic fixture rather than creating a new fixture. Add a minimal temporal API surface that covers direct echo functions, one record carrying temporal
  fields, and one object path that accepts or returns temporal values.
- Update README.md:211 to remove timestamps and durations from current limitations and document the new public Node mappings.

## Test Plan

- Rust unit tests:
    - render_public_type accepts timestamp and duration directly.
    - Nested public types containing them render correctly for optional, sequence, and map shapes.
    - render_public_api() emits Date and number signatures and pulls in the temporal converters for both normal API and callback paths.
- Snapshot coverage:
    - refresh the existing basic fixture snapshot after adding temporal APIs.
- TypeScript declaration coverage:
    - typecheck direct, optional, nested, record, and object-method temporal usage in the generated package.
- Node smoke coverage:
    - roundtrip valid Date and non-negative duration values through the generated package and fixture cdylib.
    - verify invalid Date inputs and negative durations still fail at the runtime converter boundary.

## Assumptions

- Use the existing Node-facing representation already implied by the runtime: Date for timestamps and millisecond number for durations.
- Do not introduce Temporal, bigint-based durations, or structured wrapper types in this work.
- Preserve current runtime semantics and document their limits rather than changing them here: timestamps remain millisecond-precision via Date, durations remain
  millisecond-based numbers, and inherited UniFFI edge cases stay out of scope.

## Instructions

You are in a Ralph Wiggum loop. Work through the first few TODOs in the `## TODO` section below.

- update PROMPT.md with an updated TODO list after each change
- never ever change any PROMPT.md text _except_ the items in the `## TODO` section
- you may update the TODO items as you see fit--remove outdated items, add new items, mark items as completed
- commit after each completed TODO
- use conventional commit syntax for commit messages
- if you run for more than 10 minutes, take a step back, break up the TODO into more parts, and stop
- if there are no items left in the TODO, append a new line to the end of the file that simply contains ✅

## TODO

- [x] Change render_public_type to map Type::Timestamp to Date.
- [x] Change render_public_type to map Type::Duration to number.
- [x] Replace the timestamp rejection unit test with positive timestamp rendering assertions.
- [x] Add unit tests for nested optional and sequence shapes containing timestamps.
- [x] Add unit tests for nested map shapes containing durations.
- [x] Add a render_public_api() test that exercises timestamp converter imports.
- [x] Add a render_public_api() test that exercises duration converter imports.
- [x] Add timestamp echo APIs to the basic fixture Rust source.
- [x] Add duration echo APIs to the basic fixture Rust source.
- [x] Add temporal declarations to the basic fixture UDL.
- [x] Add a fixture record that includes timestamp and duration fields.
- [x] Add one object constructor or method in the fixture that accepts or returns temporal values.
- [x] Update the TypeScript smoke test to typecheck Date timestamp usage.
- [x] Update the TypeScript smoke test to typecheck numeric duration usage.
- [ ] Add a JS smoke assertion that timestamp roundtrips return Date instances.
- [ ] Add a JS smoke assertion that duration roundtrips return numeric values.
- [ ] Add a JS smoke assertion that invalid Date inputs throw.
- [ ] Add a JS smoke assertion that negative duration inputs throw.
- [ ] Refresh the basic fixture snapshot to capture the new API surface.
- [ ] Update the README public API conventions to document timestamp and duration mappings.
- [ ] Remove timestamp and duration entries from the README limitations list.
