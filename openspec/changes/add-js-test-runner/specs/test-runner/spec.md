## ADDED Requirements

### Requirement: Auto-Terminate Flag

The `start` command SHALL accept a `--test` boolean flag. When set, the runtime SHALL auto-terminate the process after the main service's module evaluation completes, using the exit code set by the main worker via `Deno.exit()`. This eliminates the need for the main service to start an HTTP server.

#### Scenario: Start with --test flag and tests pass
- **WHEN** the `start` command is invoked with `--test --main-service /path/to/test-runner.ts`
- **AND** the main service calls `Deno.exit(0)`
- **THEN** the process exits with code 0

#### Scenario: Start with --test flag and tests fail
- **WHEN** the `start` command is invoked with `--test --main-service /path/to/test-runner.ts`
- **AND** the main service calls `Deno.exit(1)`
- **THEN** the process exits with code 1

#### Scenario: Start without --test flag
- **WHEN** the `start` command is invoked without `--test`
- **THEN** the runtime behaves as before (keeps the process alive for HTTP serving)

---

### Requirement: Test Registration via Deno.test Shim

The test runner SHALL shim `Deno.test()` onto `globalThis.Deno` before importing test files. The shim SHALL support all documented `Deno.test()` call signatures:

1. `Deno.test(name: string, fn: (t: TestContext) => void | Promise<void>)` — name + function
2. `Deno.test(fn: (t: TestContext) => void | Promise<void>)` — named function (name inferred from `fn.name`)
3. `Deno.test(options: TestDefinition)` — options object with `name`, `fn`, `ignore?`, `only?`
4. `Deno.test.ignore(name, fn)` — ignored test shorthand
5. `Deno.test.only(name, fn)` — only test shorthand

The `TestDefinition` options `sanitizeOps`, `sanitizeResources`, `sanitizeExit`, and `permissions` SHALL be accepted without error but ignored (with a one-time warning logged to console).

#### Scenario: Register test with name and function
- **WHEN** a test file calls `Deno.test("my test", () => { ... })`
- **THEN** the test is collected with name "my test" and the provided function

#### Scenario: Register test with named function
- **WHEN** a test file calls `Deno.test(function myTest() { ... })`
- **THEN** the test is collected with name "myTest" inferred from the function name

#### Scenario: Register test with options object
- **WHEN** a test file calls `Deno.test({ name: "opts test", fn: () => {}, ignore: true })`
- **THEN** the test is collected with name "opts test", the provided function, and `ignore: true`

#### Scenario: Register ignored test via shorthand
- **WHEN** a test file calls `Deno.test.ignore("skipped", () => { ... })`
- **THEN** the test is collected with `ignore: true`

#### Scenario: Register only test via shorthand
- **WHEN** a test file calls `Deno.test.only("focused", () => { ... })`
- **THEN** the test is collected with `only: true`

#### Scenario: Unsupported options produce warning
- **WHEN** a test file calls `Deno.test({ name: "x", fn: () => {}, sanitizeOps: true })`
- **THEN** a warning is logged once: "sanitizeOps is not supported in edge runtime test runner"
- **AND** the test is collected and runs normally

---

### Requirement: TestContext API

Each test function SHALL receive a `TestContext` object with the following API, matching Deno's `TestContext`:

- `name: string` — the current test or step name
- `origin: string` — the URL of the test file
- `parent?: TestContext` — the parent context if this is a step
- `step(name: string, fn: (t: TestContext) => void | Promise<void>): Promise<boolean>` — run a named sub-step
- `step(fn: (t: TestContext) => void | Promise<void>): Promise<boolean>` — run a sub-step (name from `fn.name`)
- `step(definition: { name: string, fn: (t: TestContext) => void | Promise<void>, ignore?: boolean }): Promise<boolean>` — run a sub-step from a definition object

`step()` SHALL return `true` if the step passed, `false` if it failed or was ignored. Steps SHALL be nestable (a step can contain further steps).

#### Scenario: TestContext has correct name
- **WHEN** `Deno.test("my test", (t) => { ... })` is executed
- **THEN** `t.name` equals `"my test"`

#### Scenario: TestContext has correct origin
- **WHEN** a test from file `/home/deno/tests/env_test.ts` is executed
- **THEN** `t.origin` equals `"file:///home/deno/tests/env_test.ts"` (or the import URL of the file)

#### Scenario: Step has parent context
- **WHEN** a test calls `await t.step("child", (childT) => { ... })`
- **THEN** `childT.parent` is the same object as `t`
- **AND** `childT.name` equals `"child"`

#### Scenario: Step with named function
- **WHEN** a test calls `await t.step(function myStep() { ... })`
- **THEN** the step name is `"myStep"`

#### Scenario: Step with definition object
- **WHEN** a test calls `await t.step({ name: "sub", fn: () => {}, ignore: true })`
- **THEN** the step is skipped and `step()` returns `false`

#### Scenario: Nested steps
- **WHEN** a step contains another `t.step()` call
- **THEN** the inner step executes with its own `TestContext` whose `parent` is the outer step's context

---

### Requirement: Test File Discovery

The test runner SHALL recursively discover test files under the directory specified by the `TEST_DIR` environment variable (defaulting to `./functions/tests`). Files matching the pattern `*_test.ts`, `*.test.ts`, `*_test.js`, or `*.test.js` SHALL be included. Directories starting with `.` and `node_modules` SHALL be excluded.

#### Scenario: Discover test files in default directory
- **WHEN** `TEST_DIR` is not set
- **THEN** the runner discovers test files under `./functions/tests`

#### Scenario: Discover test files in custom directory
- **WHEN** `TEST_DIR` is set to `/home/deno/my-tests`
- **THEN** the runner discovers test files under `/home/deno/my-tests`

#### Scenario: No test files found
- **WHEN** the test directory contains no matching files
- **THEN** the runner prints a warning message and exits with code 0

#### Scenario: Skip hidden and node_modules directories
- **WHEN** the test directory contains `.hidden/` or `node_modules/` subdirectories
- **THEN** the runner does not recurse into those directories

---

### Requirement: Test Execution

The test runner SHALL execute collected tests sequentially in file-discovery order. If any `only` tests exist, only those SHALL run. Tests with `ignore: true` SHALL be skipped.

#### Scenario: Sequential execution
- **WHEN** multiple tests are registered across multiple files
- **THEN** tests run sequentially in file-discovery order, one at a time

#### Scenario: Failed step fails parent test
- **WHEN** a step within a test throws an error
- **THEN** the parent test is marked as failed

#### Scenario: Only modifier
- **WHEN** one or more tests have `only: true`
- **THEN** only those tests execute; all others are skipped

#### Scenario: Ignore modifier
- **WHEN** a test has `ignore: true`
- **THEN** the test is reported as "ignored" and its function is not executed

---

### Requirement: Test Filtering

The test runner SHALL support filtering tests by name using the `TEST_FILTER` environment variable. When set, only tests whose name contains the filter string SHALL execute.

#### Scenario: Filter matches subset
- **WHEN** `TEST_FILTER` is set to "process.env"
- **THEN** only tests whose name includes "process.env" are executed

#### Scenario: No filter set
- **WHEN** `TEST_FILTER` is not set or empty
- **THEN** all discovered tests are executed

---

### Requirement: Fail-Fast Mode

The test runner SHALL support stopping execution on the first failure when the `TEST_FAIL_FAST` environment variable is set to a truthy value (`1`, `true`).

#### Scenario: Fail-fast stops on first failure
- **WHEN** `TEST_FAIL_FAST` is set to `1`
- **AND** the second test fails
- **THEN** remaining tests are not executed
- **AND** the summary reflects the partial run

#### Scenario: Fail-fast not set
- **WHEN** `TEST_FAIL_FAST` is not set
- **THEN** all tests run regardless of individual failures

---

### Requirement: Pretty Reporter

The test runner SHALL output results to the console with ANSI colors by default (or when `TEST_REPORTER=pretty`): green for passed, red for failed, yellow for ignored. Each file SHALL be introduced with a header line. Each test SHALL show its name, status, and duration. Steps SHALL be indented under their parent test. A summary line SHALL appear at the end.

#### Scenario: Passing test output
- **WHEN** a test passes
- **THEN** the output includes the test name followed by "ok" in green and the duration

#### Scenario: Failed test output
- **WHEN** a test fails
- **THEN** the output includes the test name followed by "FAILED" in red, and a failure details section with the error message and truncated stack trace

#### Scenario: Step output
- **WHEN** a test has steps
- **THEN** each step is printed indented under the parent test with its own status and duration

#### Scenario: Summary line
- **WHEN** all tests have finished
- **THEN** a summary line is printed: `test result: <status>. N passed; M failed; K ignored (duration)`

---

### Requirement: JUnit XML Reporter

The test runner SHALL support JUnit XML output when `TEST_REPORTER=junit`. The XML SHALL be written to stdout, or to the path specified by `TEST_JUNIT_PATH`. The format SHALL be compatible with CI systems (GitHub Actions, GitLab CI).

#### Scenario: JUnit output to stdout
- **WHEN** `TEST_REPORTER` is set to `junit`
- **AND** `TEST_JUNIT_PATH` is not set
- **THEN** JUnit XML is written to stdout

#### Scenario: JUnit output to file
- **WHEN** `TEST_REPORTER` is set to `junit`
- **AND** `TEST_JUNIT_PATH` is set to `/tmp/results.xml`
- **THEN** JUnit XML is written to `/tmp/results.xml`

#### Scenario: JUnit XML structure
- **WHEN** tests produce results
- **THEN** the XML contains `<testsuites>` root, one `<testsuite>` per test file, and one `<testcase>` per test with `name`, `time`, and optional `<failure>` elements

---

### Requirement: CI-Compatible Exit Code

The test runner SHALL call `Deno.exit(1)` when one or more tests fail, and `Deno.exit(0)` when all tests pass or are ignored. Combined with the `--test` flag on `start`, this produces CI-compatible exit codes.

#### Scenario: All tests pass
- **WHEN** all tests pass
- **THEN** the runner calls `Deno.exit(0)`

#### Scenario: Any test fails
- **WHEN** one or more tests fail
- **THEN** the runner calls `Deno.exit(1)`

#### Scenario: No test files found
- **WHEN** no test files are discovered
- **THEN** the runner calls `Deno.exit(0)`
