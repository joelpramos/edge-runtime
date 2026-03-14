## Dependencies

- Depends on: `add-test-subcommand` (must be complete — it is)
- Should be done BEFORE writing phase 7 integration tests

## 1. Restructure file layout

- [x] 1.1 Create `cli/src/tools/mod.rs` with `pub mod test;`
- [x] 1.2 Create `cli/src/tools/test/mod.rs` — move `run_tests()` orchestration here, add `TestEvent` enum
- [x] 1.3 Create `cli/src/tools/test/discovery.rs` — move `discover_test_files()`, `is_test_file()`, `walk_dir()` here
- [x] 1.4 Create `cli/src/tools/test/types.rs` — all type definitions
- [x] 1.5 Create `cli/src/tools/test/reporters/mod.rs` — `TestReporter` trait
- [x] 1.6 Create `cli/src/tools/test/reporters/pretty.rs` — move `PrettyReporter` here, rename to `PrettyTestReporter`
- [x] 1.7 Create `cli/src/tools/test/reporters/junit.rs` — move `JunitReporter` here, rename to `JunitTestReporter`
- [x] 1.8 Delete `cli/src/test_runner.rs` and `cli/src/test_reporter.rs`
- [x] 1.9 Update `cli/src/main.rs` to import from `crate::tools::test`
- [x] 1.10 Verify build: `cargo build -p cli`

## 2. Align core types

- [x] 2.1 Define `TestDescription` struct: `{ id, name, ignore, only, origin }`
- [x] 2.2 Define `TestStepDescription` struct: `{ id, name, level, parent_id, root_id, root_name }`
- [x] 2.3 Define `TestResult` enum: `Ok | Ignored | Failed(TestFailure) | Cancelled`
- [x] 2.4 Define `TestStepResult` enum: `Ok | Ignored | Failed(TestFailure)`
- [x] 2.5 Define `TestFailure` enum: `JsError(String) | FailedSteps(usize)`
- [x] 2.6 Define `TestPlan` struct: `{ origin, total, filtered_out, used_only }`
- [x] 2.7 Redefine `TestSummary`: `{ total, passed, failed, ignored, passed_steps, failed_steps, ignored_steps, filtered_out, failures }`
- [x] 2.8 Define `TestFilter` struct with `substring` and `regex` fields
- [x] 2.9 Update JSON deserialization to populate new types from JS results
- [x] 2.10 Verify build: `cargo build -p cli`

## 3. Align TestReporter trait

- [x] 3.1 Redefine `TestReporter` trait with event-method pattern: `report_plan`, `report_wait`, `report_result`, `report_step_wait`, `report_step_result`, `report_summary`, `flush_report`
- [x] 3.2 Update `PrettyTestReporter` to implement new trait methods
- [x] 3.3 Update `JunitTestReporter` to implement new trait methods
- [x] 3.4 Update `run_tests()` orchestration to call reporter methods per-event instead of batch
- [x] 3.5 Verify output matches previous behaviour: `edge-runtime test ./examples/test-suite/`

## 4. Enhance test discovery

- [x] 4.1 Add `mts`, `mjs`, `cjs`, `cts` to test file extensions
- [x] 4.2 Add `test.{ext}` bare name matching (e.g., `test.ts`)
- [x] 4.3 Include files under `__tests__/` directories (any extension in the supported set)
- [x] 4.4 Verify: create sample `__tests__/sample.ts` and `foo_test.mts`, confirm discovery

## 5. Enhance filter

- [x] 5.1 Implement `TestFilter` struct with `from_flag(s: &str)` constructor
- [x] 5.2 Detect regex pattern: if wrapped in `/slashes/`, compile as `Regex`
- [x] 5.3 Pass `TestFilter` to JS via JSON-serialized config object
- [x] 5.4 Update `test_bootstrap.js` to accept filter config with regex support
- [x] 5.5 Verify: `--filter "/^basic/"` runs only basic-prefixed tests

## 6. JS API alignment

- [x] 6.1 Add `Deno.test.ignore()` shorthand (all overloads)
- [x] 6.2 Add `Deno.test.only()` shorthand (all overloads)
- [x] 6.3 Update JSON result schema to include `total`, `filtered_out`, `passed_steps`, `failed_steps`, `ignored_steps`, `failures` array
- [x] 6.4 Verify: add sample tests using `Deno.test.ignore()` and `Deno.test.only()`

## 7. Validation

- [x] 7.1 Full test suite run: `edge-runtime test ./examples/test-suite/` produces same output as before
- [x] 7.2 Filter test: `--filter "env"` still works
- [x] 7.3 Regex filter test: `--filter "/^basic/"` works
- [x] 7.4 JUnit output: `--reporter junit` produces valid XML
- [x] 7.5 Fail-fast: `--fail-fast` stops correctly
- [x] 7.6 `Deno.test.only` works
- [x] 7.7 `Deno.test.ignore` works
