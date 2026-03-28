# Hybrid Askama Refactor For Public API Codegen

## Summary

Keep the Node-specific adapter/model layer and Rust-side validation/expression logic, but move the large public API JS and .d.ts string emitters behind Askama templates.
Preserve render_public_api() and keep generated output behaviorally identical.

## Interfaces

- Keep ComponentModel::from_ci(...).render_public_api() as the entrypoint.
- Keep RenderedComponentApi { js, dts, requires_async_rust_future_hooks } unchanged.
- Keep src/bindings/mod.rs consuming public_api_js / public_api_dts unchanged.
- Add only internal rendering types and Askama templates. No public API or config changes.

## Implementation Changes

- Split the current src/bindings/api.rs responsibilities into three internal layers under src/bindings/api/: model construction, render support, and template rendering.
- Keep the model layer responsible for all Node-specific normalization from UniFFI into ComponentModel: enum/error partitioning, callback-interface extraction, object
  filtering, FFI symbol capture, async scaffolding capture, and renderability validation.
- Move pure helper logic into the support layer: public type rendering, JS identifier and symbol sanitization, string literal quoting, converter naming, object/callback
  helper naming, lower/lift expression rendering, Koffi type rendering, allocation-size helpers, property-access helpers, and async-complete helper selection.
- Add a render layer that owns Askama integration only. It should expose a small renderer/view API to templates instead of passing raw ComponentInterface or relying on
  templates to call low-level helper functions directly.
- Make render_public_api() construct a renderer from &ComponentModel, compute requires_async_rust_future_hooks, render the DTS template, render the JS template, and return
  RenderedComponentApi. It should stop manually assembling Vec<String> sections once the migration is complete.
- Use ComponentModel as the template context boundary. Do not pass raw UniFFI types into the new templates.
- Introduce renderer/view structs for the major template loops so templates can stay structural rather than algorithmic. At minimum, expose typed views for functions,
  objects, methods, constructors, records, enums, errors, callback interfaces, and callback methods.
- Put all non-trivial Type-matching logic behind renderer/view methods. Templates should ask for things like params, return types, lower/lift expressions, converter
  expressions, allocation-size expressions, callback symbol names, or async helper names, rather than reconstructing those decisions inline.
- Treat templates as owners of declaration structure and formatting. They should own section ordering, declaration shells, statement ordering, if/for layout, punctuation,
  and blank-line placement.
- Treat Rust helpers as owners of leaf expressions and branch-heavy computation. They should own any logic that depends on Type, async FFI metadata, object-vs-callback
  distinctions, generic ABI symbol selection, or generated identifier rules.
- Add two top-level body templates under templates/api/: one for JS and one for DTS. These templates should replace the current high-level section assembly performed in
  render_public_api().
- Add fragment templates grouped by output kind rather than by file: DTS fragments for records, callback interfaces, enums, errors, functions, and objects; JS fragments for
  functions, objects, enums, errors, converters, callback registration, runtime helpers, and runtime hooks.
- Keep the JS top-level template responsible for the existing section order: unimplemented helper, runtime helpers, async rust-future helpers, type declarations/converters,
  runtime hooks, functions, then objects. Preserve the current conditional inclusion rules for each section.
- Keep the DTS top-level template responsible for the existing declaration order: records, flat enums, tagged enums, errors, callback interfaces, functions, then objects.
- Migrate DTS first because it is mostly declarative and will validate the template/view boundary with low risk.
- After DTS is stable, migrate JS functions and objects next. These are the most readable wins and will prove that templates can own method/class structure while Rust still
  owns lower/lift and async scaffolding expressions.
- Migrate converters after functions/objects. Keep converter internals template-backed, but keep allocation-size math, property access, and converter expression generation
  in Rust helper methods.
- Migrate callback vtable registration and runtime hooks last. These are the most branch-heavy pieces and should use the same renderer/view boundary established by earlier
  phases.
- Do not change src/bindings/ffi.rs, the package templates, or the runtime templates in this refactor. The goal is only to restructure public API body generation.
- Delete old string-builder functions only after the equivalent template path is wired, covered by the existing tests, and producing stable output.

## Test Plan

- Keep current assertions and snapshots as the acceptance contract.
- Treat whitespace-only drift as a template formatting issue; fix templates rather than changing expected output.
- Run the existing bindings::api tests, package generator tests, snapshot tests, and ffi_initialization_codegen as the final verification pass.

## Assumptions

- This is a structural refactor only; emitted API shape and generated file contents should remain effectively identical.
- ComponentModel, not raw ComponentInterface, remains the boundary passed into the public API renderer.
- The outer component templates remain responsible only for the package shell and interpolation of the rendered public API body.

## Instructions

You are in a Ralph Wiggum loop. You are making progress on the plan defined above. Work through the first few TODOs in the `## TODO` section below.

- update PROMPT.md with an updated TODO list after each change
- never ever change any PROMPT.md text _except_ the items in the `## TODO` section
- you may update the TODO items as you see fit--remove outdated items, add new items, mark items as completed
- commit after each completed TODO
- use conventional commit syntax for commit messages
- if there are no items left in TODO.md, append a final line in PROMPT.md that contains only the emoji: ✅

## TODO

- [x] Create src/bindings/api/ and introduce internal model, support, and render modules while preserving the current public module surface.
- [x] Move ComponentModel, RenderedComponentApi, and the existing *Model adapter structs into the new model module without changing behavior.
- [x] Move validation helpers, identifier helpers, naming helpers, and expression-rendering helpers into the support module without changing call sites yet.
- [x] Add Askama template wrapper types for public API JS and DTS rendering and keep render_public_api() as the stable orchestration entrypoint.
- [x] Create templates/api/public-api.d.ts.j2 as the top-level DTS body template.
- [x] Extract DTS record rendering into a template fragment and wire it through the new DTS template wrapper.
- [x] Extract DTS callback-interface rendering into a template fragment and wire it through the new DTS template wrapper.
- [x] Extract DTS enum and error rendering into template fragments and wire them through the new DTS template wrapper.
- [x] Extract DTS function and object rendering into template fragments and wire them through the new DTS template wrapper.
- [x] Remove the old DTS render_* string builders once the DTS template path is passing existing tests unchanged.
- [x] Create templates/api/public-api.js.j2 as the top-level JS body template, preserving the current section ordering.
- [x] Extract JS function rendering into a template fragment and keep body-level expressions delegated to Rust helper methods.
- [x] Extract JS object/class and constructor rendering into template fragments and keep async/sync body logic delegated to Rust helper methods.
- [x] Extract JS flat enum, tagged enum, and error declarations into template fragments.
- [x] Extract JS converter rendering for records, enums, errors, and callback interfaces into template fragments while keeping allocation-size math and converter-expression
  generation in Rust.
- [x] Extract JS callback vtable registration fragments and keep sync/async branching plus symbol lookup in Rust support methods.
- [x] Extract JS runtime helper and runtime hook fragments into templates without changing their emitted contents.
- [x] Remove the obsolete JS Vec<String> emitter functions after all JS fragments are template-backed and snapshot-stable.
- [x] Run cargo test bindings::api::tests and fix any regressions without changing intended output.
- [x] Run cargo test bindings::tests and fix any generator-level regressions.
- [ ] Run cargo test --test codegen_snapshots and resolve any whitespace/control-flow drift in templates.
- [ ] Run cargo test --test ffi_initialization_codegen as the final regression check around adjacent codegen behavior.
