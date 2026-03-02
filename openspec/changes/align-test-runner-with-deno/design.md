# Design: Align Test Runner with Deno Patterns

## Structural Comparison

### Deno upstream (`denoland/deno/cli/`)

```
cli/
  tools/
    test/
      mod.rs              # Core orchestration, types, TestDescription, TestResult, TestSummary
      channel.rs          # TestEvent enum, TestEventSender/Receiver
      fmt.rs              # Error formatting, sanitizer diff formatting
      sanitizers.rs       # Op/resource/timer leak detection
      reporters/
        mod.rs            # TestReporter trait
        pretty.rs         # ANSI colored console output
        junit.rs          # JUnit XML output
        dot.rs            # Single-char-per-test output
        tap.rs            # TAP protocol output
        compound.rs       # Multi-reporter wrapper
        common.rs         # Shared utilities
  ops/
    testing.rs            # V8 ops bridge (op_register_test, etc.)
  js/
    40_test.js            # Deno.test() API, TestContext
```

### Current edge-runtime (`cli/src/`)

```
cli/src/
  test_runner.rs          # Discovery + orchestration (mixed concerns)
  test_reporter.rs        # Types + reporters (mixed concerns)
  flags.rs                # TestFlags, enums
  main.rs                 # test match arm
ext/runtime/js/
  test_bootstrap.js       # Deno.test() shim + __test_runner
```

### Proposed edge-runtime

```
cli/src/
  tools/
    test/
      mod.rs              # Re-exports, run_tests() entry point, TestEvent enum
      discovery.rs         # discover_test_files(), is_test_file(), walk_dir()
      types.rs            # TestDescription, TestStepDescription, TestResult, TestFailure, TestSummary, TestFilter
      reporters/
        mod.rs            # TestReporter trait
        pretty.rs         # PrettyTestReporter
        junit.rs          # JunitTestReporter
  tools/
    mod.rs                # pub mod test;
  flags.rs                # TestFlags, TestWorkerKind, TestReporterKind (unchanged)
  main.rs                 # test match arm (unchanged)
ext/runtime/js/
  test_bootstrap.js       # Updated with Deno.test.ignore/only, TestEvent communication
```

## Type Alignment

### Core types (Deno → edge-runtime mapping)

| Deno type | Current edge-runtime | Proposed |
|---|---|---|
| `TestDescription` | (implicit in JSON) | `TestDescription { id, name, ignore, only, origin, location }` |
| `TestStepDescription` | (implicit in JSON) | `TestStepDescription { id, name, origin, level, parent_id, root_id }` |
| `TestResult` (enum) | `status: String` | `TestResult::Ok \| Ignored \| Failed(TestFailure) \| Cancelled` |
| `TestFailure` (enum) | `error: Option<String>` | `TestFailure::JsError(String) \| FailedSteps(usize)` |
| `TestSummary` | `TestSummary { passed, failed, ignored, results }` | `TestSummary { total, passed, failed, ignored, passed_steps, failed_steps, ignored_steps, filtered_out, failures }` |
| `TestPlan` | (none) | `TestPlan { origin, total, filtered_out, used_only }` |
| `TestFilter` | `filter: Option<String>` | `TestFilter { substring, regex }` |
| `TestLocation` | (none) | `TestLocation { file_name, line_number, column_number }` — optional, can be added later |
| `TestEvent` (enum) | (JSON blob) | `TestEvent::Plan \| Wait \| Result \| StepWait \| StepResult \| Completed` |

### Reporter trait alignment

**Deno has 14+ methods.** We adopt a meaningful subset:

```rust
pub trait TestReporter {
    fn report_plan(&mut self, plan: &TestPlan);
    fn report_wait(&mut self, description: &TestDescription);
    fn report_result(&mut self, description: &TestDescription, result: &TestResult, elapsed: u64);
    fn report_step_wait(&mut self, description: &TestStepDescription);
    fn report_step_result(
        &mut self,
        description: &TestStepDescription,
        result: &TestStepResult,
        elapsed: u64,
        tests: &IndexMap<usize, TestDescription>,
        test_steps: &IndexMap<usize, TestStepDescription>,
    );
    fn report_summary(&mut self, summary: &TestSummary, elapsed: &Duration);
    fn flush_report(&mut self, elapsed: &Duration) -> anyhow::Result<()>;
}
```

**Not adopted** (not needed without multi-worker concurrency):
- `report_register` — tests discovered inline
- `report_slow` — no slow-test timer yet
- `report_output` — no per-test output capture
- `report_uncaught_error` — handled as `TestFailure::JsError`
- `report_sigint` — no graceful Ctrl+C yet
- `report_completed` — implicit in `flush_report`

## Test Discovery Alignment

### Additional file patterns

Deno supports more patterns than our current implementation:

| Pattern | Current | Proposed |
|---|---|---|
| `*_test.{ts,tsx,js,jsx}` | Yes | Yes |
| `*.test.{ts,tsx,js,jsx}` | Yes | Yes |
| `*_test.{mts,mjs,cjs,cts}` | No | Yes |
| `*.test.{mts,mjs,cjs,cts}` | No | Yes |
| `test.{ts,tsx,js,jsx}` | No | Yes |
| `**/__tests__/*` | No | Yes |

### Filter enhancement

Deno supports regex filter via `--filter /pattern/`:
- If filter starts and ends with `/`, treat as regex
- Otherwise, substring match (current behaviour)

## JS API Alignment

### `Deno.test.ignore()` and `Deno.test.only()`

Deno provides shorthand methods:
```javascript
Deno.test.ignore("skipped test", () => { ... });  // same as { ignore: true }
Deno.test.only("focus test", () => { ... });       // same as { only: true }
```

These should be added to `test_bootstrap.js`.

### TestEvent communication (future consideration)

Deno uses ops (`op_register_test`, `op_test_event_step_wait`, etc.) for real-time JS→Rust event streaming. Our current approach of running all tests in JS and returning a JSON blob works but doesn't support:
- Real-time output during test execution
- Per-test timeouts enforced from Rust
- Progress reporting during long test suites

For this proposal, we keep the JSON approach but structure the JS to emit the same information. The event-based approach is a future enhancement.

## Decisions

1. **No `channel.rs`**: Without multi-worker concurrency, event channels are unnecessary. The orchestration in `mod.rs` drives reporters directly.

2. **No `sanitizers.rs`**: Sanitizers require deep integration with the op system. Out of scope.

3. **No `fmt.rs`**: Error formatting is simple enough to stay inline in reporters. Can be extracted later.

4. **No `dot.rs` / `tap.rs`**: Only pretty and junit reporters for now. The trait makes adding more trivial.

5. **`types.rs` separate from `mod.rs`**: Keeps `mod.rs` focused on orchestration (unlike Deno which mixes types into mod.rs at 2700+ lines).

6. **`discovery.rs` separate from `mod.rs`**: Deno puts discovery in mod.rs. We separate it for clarity since our discovery is simpler (no doc-test extraction, no LSP, no specifier resolution).
