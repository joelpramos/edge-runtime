# Test Runner Structure Alignment

## MODIFIED Requirements

### Requirement: File layout follows `cli/src/tools/test/` convention

The test runner implementation MUST be organized under `cli/src/tools/test/` matching Deno's `cli/tools/test/` pattern:
- `cli/src/tools/mod.rs` — exports `pub mod test;`
- `cli/src/tools/test/mod.rs` — entry point, `run_tests()`, event processing
- `cli/src/tools/test/discovery.rs` — file discovery functions
- `cli/src/tools/test/types.rs` — all test-related type definitions
- `cli/src/tools/test/reporters/mod.rs` — `TestReporter` trait
- `cli/src/tools/test/reporters/pretty.rs` — `PrettyTestReporter`
- `cli/src/tools/test/reporters/junit.rs` — `JunitTestReporter`

#### Scenario: Module imports use tools path
Given the test subcommand match arm in `main.rs`
When the test runner is invoked
Then it calls `crate::tools::test::run_tests(flags)`

#### Scenario: Old flat files removed
Given the restructured code
Then `cli/src/test_runner.rs` and `cli/src/test_reporter.rs` no longer exist

### Requirement: Type names align with Deno conventions

Core types MUST use Deno's naming:

- `TestDescription` (replaces implicit JSON fields)
- `TestStepDescription` (replaces `StepResult`)
- `TestResult` as enum: `Ok | Ignored | Failed(TestFailure) | Cancelled`
- `TestStepResult` as enum: `Ok | Ignored | Failed(TestFailure)`
- `TestFailure` as enum: `JsError(String) | FailedSteps(usize)`
- `TestSummary` with fields: `total, passed, failed, ignored, passed_steps, failed_steps, ignored_steps, filtered_out, failures`
- `TestPlan` with fields: `origin, total, filtered_out, used_only`
- `TestFilter` with fields: `substring: Option<String>, regex: Option<Regex>`

#### Scenario: TestResult is an enum not a string
Given a test that passes
When the result is constructed in Rust
Then it is `TestResult::Ok`, not `TestResult { status: "passed" }`

#### Scenario: TestFailure captures error detail
Given a test that throws an error
When the failure is reported
Then `TestFailure::JsError(stack_trace_string)` contains the full stack

### Requirement: TestReporter trait uses event-method pattern

The `TestReporter` trait MUST provide per-event methods matching a subset of Deno's trait:

```rust
pub trait TestReporter {
    fn report_plan(&mut self, plan: &TestPlan);
    fn report_wait(&mut self, description: &TestDescription);
    fn report_result(&mut self, description: &TestDescription, result: &TestResult, elapsed: u64);
    fn report_step_wait(&mut self, description: &TestStepDescription);
    fn report_step_result(&mut self, description: &TestStepDescription, result: &TestStepResult, elapsed: u64);
    fn report_summary(&mut self, summary: &TestSummary, elapsed: &Duration);
    fn flush_report(&mut self, elapsed: &Duration) -> anyhow::Result<()>;
}
```

#### Scenario: Reporter receives per-test events
Given 3 tests running
Then the reporter receives `report_wait` + `report_result` for each test in sequence

#### Scenario: Reporter receives plan before tests
Given test execution starts
Then `report_plan()` is called before any `report_wait()` calls

### Requirement: Test discovery supports extended file patterns

Discovery MUST match the following patterns (Deno parity):
- `*_test.{ts,tsx,js,jsx,mts,mjs,cjs,cts}`
- `*.test.{ts,tsx,js,jsx,mts,mjs,cjs,cts}`
- `test.{ts,tsx,js,jsx,mts,mjs,cjs,cts}`
- Files under `__tests__/` directories

#### Scenario: mts test files are discovered
Given a directory containing `math_test.mts`
When discovery runs on that directory
Then the file is included in results

#### Scenario: __tests__ directory files are included
Given a directory tree containing `src/__tests__/helper.ts`
When discovery runs on the parent
Then `src/__tests__/helper.ts` is included

### Requirement: Filter supports regex in addition to substring

When `--filter` value is wrapped in `/slashes/`, it MUST be compiled as a regex.
Otherwise it remains a substring match.

#### Scenario: Regex filter
Given `--filter "/^math_/"` and tests named "math_add", "math_sub", "string_concat"
When filtering is applied
Then only "math_add" and "math_sub" run

#### Scenario: Substring filter unchanged
Given `--filter "math"` and tests named "math_add", "math_sub", "string_concat"
When filtering is applied
Then "math_add" and "math_sub" run (existing behaviour preserved)

## ADDED Requirements

### Requirement: JS API provides Deno.test.ignore() and Deno.test.only() shorthands

`test_bootstrap.js` MUST provide:
- `Deno.test.ignore(name, fn)` — equivalent to `Deno.test({ name, fn, ignore: true })`
- `Deno.test.only(name, fn)` — equivalent to `Deno.test({ name, fn, only: true })`

Both support the same overloads as `Deno.test()`.

#### Scenario: Deno.test.ignore skips test
Given a test registered via `Deno.test.ignore("skip me", () => {})`
When the test suite runs
Then the test shows as "ignored"

#### Scenario: Deno.test.only focuses tests
Given tests registered via `Deno.test.only("focus", fn)` and `Deno.test("other", fn)`
When the test suite runs
Then only "focus" runs and "other" is ignored

### Requirement: JSON result schema aligns with Rust types

The JS `__test_runner.run()` return value MUST map cleanly to the Rust `TestSummary` struct, using the same field names where possible: `total`, `passed`, `failed`, `ignored`, `filtered_out`, `passed_steps`, `failed_steps`, `ignored_steps`, `results`, `failures`.

#### Scenario: JSON includes filtered_out count
Given 10 tests with `--filter "math"` matching 3
When results are returned
Then `filtered_out: 7` is in the JSON
