## ADDED Requirements

### Requirement: Test Subcommand

The edge-runtime binary SHALL accept a `test` subcommand with the following interface:

```
edge-runtime test [OPTIONS] [paths...]
```

Positional `paths` SHALL accept one or more file or directory paths (default: `.`). Options:
- `--filter <PATTERN>` — run only tests whose name contains the pattern
- `--worker-kind <KIND>` — worker context: `main` (default) or `user`
- `--timeout <MS>` — per-test timeout in milliseconds (default: `30000`)
- `--fail-fast` — stop execution after the first test failure
- `--reporter <FORMAT>` — output format: `pretty` (default) or `junit`
- `--junit-path <PATH>` — write JUnit XML to file (implies `--reporter junit` for that output)
- `--disable-module-cache` — disable module cache

#### Scenario: Run tests in a directory
- **WHEN** the user runs `edge-runtime test ./functions/tests/`
- **THEN** the runner discovers and executes all test files in that directory

#### Scenario: Run a specific test file
- **WHEN** the user runs `edge-runtime test ./functions/tests/env_test.ts`
- **THEN** the runner executes only that test file

#### Scenario: Default path
- **WHEN** the user runs `edge-runtime test` with no paths
- **THEN** the runner discovers test files starting from the current directory

#### Scenario: Docker usage
- **WHEN** the user runs `docker run --rm -v $(pwd):/home/deno supabase/edge-runtime test /home/deno/functions/tests/`
- **THEN** the runner discovers and executes test files inside the container

---

### Requirement: Rust-Side Test File Discovery

The `test` subcommand SHALL discover test files by recursively walking directories provided as positional arguments. Files matching the patterns `*_test.{ts,tsx,js,jsx}` or `*.test.{ts,tsx,js,jsx}` SHALL be included. Directories starting with `.` and `node_modules` SHALL be excluded. When a positional argument is a file (not a directory), it SHALL be included directly without pattern matching.

#### Scenario: Discover test files recursively
- **WHEN** the path `./functions/tests/` is provided and contains `env_test.ts`, `handler.test.ts`, and `utils.ts`
- **THEN** only `env_test.ts` and `handler.test.ts` are discovered

#### Scenario: Direct file path
- **WHEN** the path `./functions/tests/env_test.ts` is provided
- **THEN** that file is included directly

#### Scenario: Skip hidden and node_modules
- **WHEN** a directory contains `.cache/` and `node_modules/` subdirectories
- **THEN** those directories are not recursed into

#### Scenario: No test files found
- **WHEN** no matching test files are found in the provided paths
- **THEN** the runner prints `No test files found` to stderr and exits with code 1

#### Scenario: Deterministic order
- **WHEN** multiple test files are discovered
- **THEN** they are sorted lexicographically by path

---

### Requirement: Test Worker Creation

The `test` subcommand SHALL create a worker programmatically using the existing `WorkerBuilder` infrastructure. The worker SHALL be initialized with `test_mode: true` in its `WorkerContextInitOpts`, which causes the test bootstrap JS to be loaded before any user modules. The worker SHALL have the same extensions, module loader, and bootstrap as workers created by the `start` command.

#### Scenario: Worker has edge runtime APIs
- **WHEN** a test worker is created
- **THEN** the worker has access to `EdgeRuntime.*`, `Supabase.*`, patched `process.env`, and `Deno.env` via SupaEnv

#### Scenario: Worker has test bootstrap
- **WHEN** a test worker is created with `test_mode: true`
- **THEN** `Deno.test()` is available on `globalThis.Deno` before any test files are loaded

#### Scenario: Worker without test mode
- **WHEN** a worker is created without `test_mode` (normal `start` behavior)
- **THEN** `Deno.test()` is NOT available (no behavioral change to existing workers)

---

### Requirement: Worker Kind Selection

The `test` subcommand SHALL support a `--worker-kind` flag with values `main` (default) and `user`. When `main` is selected, the test worker SHALL be created as a `MainWorker` with full Deno permissions. When `user` is selected, the test worker SHALL be created as a `UserWorker` with the same sandbox restrictions as production user workers (limited `Deno.sys`, no `Deno.run`, restricted FS access).

#### Scenario: Main worker context (default)
- **WHEN** the user runs `edge-runtime test ./tests/` without `--worker-kind`
- **THEN** tests execute in a main worker with full permissions

#### Scenario: User worker context
- **WHEN** the user runs `edge-runtime test --worker-kind user ./tests/`
- **THEN** tests execute in a user worker with production-equivalent sandbox restrictions

#### Scenario: User worker API surface
- **WHEN** tests run with `--worker-kind user`
- **THEN** `EdgeRuntime.waitUntil` is available but `EdgeRuntime.userWorkers` is NOT

---

### Requirement: Built-in Test Bootstrap

The edge runtime SHALL include a built-in test bootstrap JS module (`ext/runtime/js/test_bootstrap.js`) that is loaded via `execute_script()` when `test_mode` is true. The bootstrap SHALL provide:

1. `Deno.test()` shim with all documented call signatures (matching `test-runner` capability from proposal 001)
2. `TestContext` with full Deno API: `name`, `origin`, `parent`, all `step()` overloads
3. A `__test_runner.run(filter, failFast)` function callable from Rust that executes collected tests and returns structured JSON results

The bootstrap SHALL accept `sanitizeOps`, `sanitizeResources`, `sanitizeExit`, and `permissions` on `TestDefinition` without error (log warning, ignore).

#### Scenario: Deno.test available after bootstrap
- **WHEN** the test bootstrap has been executed
- **THEN** `Deno.test` is a function on `globalThis.Deno`

#### Scenario: Test collection via dynamic import
- **WHEN** a test file is loaded as an ES module after bootstrap
- **AND** the file calls `Deno.test("my test", fn)`
- **THEN** the test is collected in the internal registry

#### Scenario: Run returns structured results
- **WHEN** `__test_runner.run(null, false)` is called after loading test files
- **THEN** it returns a JSON string with `{ passed, failed, ignored, results: [...] }`

#### Scenario: Filter applied during run
- **WHEN** `__test_runner.run("process.env", false)` is called
- **THEN** only tests whose name contains "process.env" are executed

---

### Requirement: Test Orchestration

The `test` subcommand SHALL orchestrate test execution by: (1) creating a test worker, (2) loading each discovered test file as an ES module, (3) calling `__test_runner.run()` to execute collected tests, (4) parsing the structured JSON results, and (5) reporting results via the selected reporter. Test files SHALL be loaded in discovery order (lexicographic).

#### Scenario: Multiple test files
- **WHEN** three test files are discovered
- **THEN** all three are loaded as modules, and all collected tests are executed sequentially

#### Scenario: Test file with import error
- **WHEN** a test file fails to load (syntax error, missing import)
- **THEN** the error is reported for that file and remaining files continue to load

#### Scenario: Structured result parsing
- **WHEN** `__test_runner.run()` returns JSON results
- **THEN** the Rust orchestrator parses them into a `TestSummary` struct for reporting

---

### Requirement: Per-Test Timeout

The `test` subcommand SHALL enforce a per-test timeout specified by `--timeout` (default: 30000ms). When a test exceeds the timeout, it SHALL be marked as failed with a timeout error message.

#### Scenario: Test within timeout
- **WHEN** a test completes in 500ms and the timeout is 30000ms
- **THEN** the test passes normally

#### Scenario: Test exceeds timeout
- **WHEN** a test runs for more than 30000ms
- **THEN** the test is marked as failed with message "test exceeded timeout of 30000ms"

#### Scenario: Custom timeout
- **WHEN** the user specifies `--timeout 5000`
- **THEN** tests that run longer than 5000ms are failed

---

### Requirement: Fail-Fast Mode

The `test` subcommand SHALL support a `--fail-fast` flag. When set, test execution SHALL stop after the first test failure. The summary SHALL reflect the partial run (tests not executed are not counted).

#### Scenario: Fail-fast stops on first failure
- **WHEN** `--fail-fast` is set and the second test fails
- **THEN** remaining tests are not executed
- **AND** the exit code is 1

#### Scenario: Fail-fast not set
- **WHEN** `--fail-fast` is not set
- **THEN** all tests run regardless of individual failures

---

### Requirement: Pretty Reporter

The `test` subcommand SHALL output results using a pretty reporter by default (or when `--reporter pretty`). The format SHALL match `deno test` conventions: file headers, indented test names with colored status (green=ok, red=FAILED, yellow=ignored), durations, step indentation, failure details with stack traces, and a summary line.

#### Scenario: File header
- **WHEN** tests from `./functions/tests/env_test.ts` are about to run
- **THEN** the output includes `running N tests from ./functions/tests/env_test.ts`

#### Scenario: Passing test
- **WHEN** a test passes
- **THEN** the output includes `  test_name ... ok (Nms)` with green coloring

#### Scenario: Failed test with details
- **WHEN** a test fails
- **THEN** the output includes `  test_name ... FAILED (Nms)` with red coloring
- **AND** a `FAILURES` section at the end with error message and stack trace

#### Scenario: Summary line
- **WHEN** all tests have finished
- **THEN** a summary line is printed: `test result: <status>. N passed; M failed; K ignored (duration)`

---

### Requirement: JUnit XML Reporter

The `test` subcommand SHALL support JUnit XML output via `--reporter junit` or `--junit-path <PATH>`. The XML SHALL follow the JUnit schema with `<testsuites>`, `<testsuite>` per file, and `<testcase>` per test. Failed tests SHALL include `<failure>` elements with the error message.

#### Scenario: JUnit to stdout
- **WHEN** `--reporter junit` is specified without `--junit-path`
- **THEN** JUnit XML is written to stdout

#### Scenario: JUnit to file
- **WHEN** `--junit-path /tmp/results.xml` is specified
- **THEN** JUnit XML is written to `/tmp/results.xml`
- **AND** the pretty reporter still outputs to stdout (both reporters active)

#### Scenario: Valid XML structure
- **WHEN** test results are available
- **THEN** the XML contains `<testsuites>` root with `tests`, `failures`, `time` attributes
- **AND** one `<testsuite>` per test file with `<testcase>` elements

---

### Requirement: CI-Compatible Exit Code

The `test` subcommand SHALL exit with code 0 when all tests pass or are ignored, and code 1 when any test fails or no test files are found.

#### Scenario: All tests pass
- **WHEN** all tests pass
- **THEN** exit code is 0

#### Scenario: Any test fails
- **WHEN** one or more tests fail
- **THEN** exit code is 1

#### Scenario: No test files found
- **WHEN** no test files are discovered
- **THEN** exit code is 1
