# Remove SlateDB From Integration Tests

## Summary

- Delete the SlateDB-only integration path entirely rather than hiding it behind an env var.
- Keep integration coverage fully repo-owned: basic continues covering package loading, bundled/manual-load behavior, Buffer handling, and async calls; callbacks expands to
  cover callback runtime plus the richer object and bytes surface that SlateDB was exercising.
- Limit this cleanup to integration tests and integration support code.

## Key Changes

- Remove tests/slatedb_build_tests.rs, tests/slatedb_package_generation.rs, and tests/slatedb_node_smoke_tests.rs.
- Remove SlateDB-specific helpers and structs from tests/support/mod.rs so the harness only builds and packages repo fixtures.
- Expand the callbacks fixture UDL and Rust implementation to include generic replacements for the removed SlateDB smoke surface: LogLevel, LogRecord, LogCollector,
  Settings.default(), Settings.set(string key, string value_json), Settings.to_json_string(), WriteBatch.new(), WriteBatch.put(bytes, bytes), WriteBatch.delete(bytes),
  WriteBatch.operation_count(), and init_logging(LogLevel, LogCollector?).
- Update the mock koffi runtime in tests/npm-fixtures/koffi/index.js to implement fixture_callbacks end to end and delete all slatedb_uniffi symbol dispatch and behavior.
- Extend the existing package, smoke, and TypeScript integration suites to exercise the richer callback fixture instead of having dedicated SlateDB test files.
- Regenerate the callback snapshot in tests/snapshots/codegen_snapshots__callback_fixture_generated_output.snap to preserve generator regression coverage after the fixture
  expansion.

## Test Plan

- Run the updated package, smoke, Buffer, and TypeScript integration tests through cargo test.
- Regenerate and review the callback snapshot diff to ensure the new fixture output is intentional.
- Confirm rg -n "slatedb|SlateDB" tests returns no matches in integration tests, support code, or the mock koffi fixture.

## Assumptions

- SlateDB-specific integration coverage should be removed outright, not made opt-in.
- Existing non-integration unit tests that use “SlateDB” only as illustrative naming remain out of scope.
- Expanding the existing callbacks fixture is preferred to adding a third repo-owned fixture.

## Instructions

You are in a Ralph Wiggum loop. Work through the first few TODOs in the `## TODO` section below.

- update PROMPT.md with an updated TODO list after each change
- never ever change any PROMPT.md text _except_ the items in the `## TODO` section
- you may update the TODO items as you see fit--remove outdated items, add new items, mark items as completed
- commit after each completed TODO
- use conventional commit syntax for commit messages
- if there are no items left in TODO.md, append a final line in PROMPT.md that contains only the emoji: ✅

## TODO

- [x] Delete the three SlateDB integration test files.
- [x] Remove BuiltSlateDbCdylib, GeneratedSlateDbPackage, build_slatedb_cdylib(), and generate_slatedb_package() from the integration support module, along with any remaining shared-support references to the deleted SlateDB path.
- [x] Extend fixtures/callback-fixture/src/callbacks_fixture.udl with generic callback, record, enum, and object APIs that replace the removed SlateDB smoke surface.
- [x] Implement the new callback fixture APIs in fixtures/callback-fixture/src/lib.rs with simple deterministic in-memory behavior suitable for tests.
- [ ] Teach the fixture lookup helpers in the test harness and snapshot generator about the expanded callbacks fixture shape without introducing any new external dependency.
- [ ] Replace the SlateDB-specific mock runtime in tests/npm-fixtures/koffi/index.js with a fixture_callbacks runtime that handles callback registration, object handles, byte
  arguments, JSON-string settings updates, and emitted log records.
- [ ] Add a callback package-generation test to the existing package suite that verifies emitted files and local koffi installation for a compiled callback fixture package.
- [ ] Add a callback smoke test to the existing smoke suite that covers emit, last_message, Settings.default/set/to_json_string, WriteBatch.put/delete/operation_count, and in
  it_logging(LogLevel.Info, collector) plus the undefined callback path.
- [ ] Add a callback TypeScript test to the existing TS suite that typechecks Settings, WriteBatch, LogLevel, LogRecord, LogCollector, emit, last_message, and init_logging.
- [ ] Update the callback snapshot to match the expanded generated API and runtime hook registration output.
- [ ] Verify there are no remaining SlateDB references in integration tests or integration support code.
