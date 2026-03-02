# Align Test Runner with Deno Patterns

## Summary

Restructure the edge-runtime `test` subcommand implementation to follow Deno's file layout, type naming, and architectural patterns more closely. The goal is structural parity where possible, making the edge-runtime test runner familiar to anyone who has worked on `denoland/deno`'s `cli/tools/test/` module.

## Motivation

The current implementation was built as a working proof of concept. It delivers the right behaviour, but its file layout, type names, and internal communication patterns diverge significantly from upstream Deno. Aligning now — before writing integration tests in phase 7 — avoids test churn and establishes the canonical structure going forward.

## Scope

This change covers **structural and organizational alignment**. It does NOT attempt to add every upstream feature (sanitizers, multi-worker concurrency, output capture, permission scoping, etc.). Those are future work items.

### What changes

| Area | Current (edge-runtime) | Target (Deno-aligned) |
|---|---|---|
| **Folder layout** | Flat: `cli/src/test_runner.rs`, `cli/src/test_reporter.rs` | Nested: `cli/src/tools/test/mod.rs`, `cli/src/tools/test/reporters/` |
| **Type names** | `TestSummary`, `TestResult`, `StepResult` | `TestDescription`, `TestResult` (enum), `TestStepDescription`, `TestFailure`, `TestSummary` |
| **Reporter trait** | 4 methods, result-batch based | Event-method based, closer to Deno's 14-method `TestReporter` trait (subset) |
| **Test discovery** | ts/tsx/js/jsx only | Add mts/mjs/cjs/cts, `__tests__/` directories |
| **Filter** | Substring only | Substring + regex (`--filter /pattern/`) |
| **JS API** | Functional but flat | Add `Deno.test.ignore()`, `Deno.test.only()` shorthands |
| **Event types** | JSON blob from JS | Rust `TestEvent` enum (simplified subset of Deno's) |

### What stays the same

- Single-worker execution model (no concurrent test files)
- No sanitizers (ops/resources/timers)
- No output capture per test
- No hooks (beforeAll/afterAll) — future work
- No LSP integration
- eszip-based module loading via synthetic entrypoint

## Related

- Depends on: `add-test-subcommand` (current implementation)
- Informs: phase 7 integration tests (should test the aligned API surface)
