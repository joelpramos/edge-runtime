## Dependencies

- Depends on: `add-js-test-runner` (validates concept; shared JS patterns)

## 1. CLI Subcommand (Rust)

- [x] 1.1 Add `get_test_command()` to `cli/src/flags.rs` with: positional paths, `--filter`, `--worker-kind`, `--timeout`, `--fail-fast`, `--reporter`, `--junit-path`, `--disable-module-cache`
- [x] 1.2 Wire `get_test_command()` into `get_cli()` subcommand list
- [x] 1.3 Add `Some(("test", sub_matches))` match arm in `cli/src/main.rs`
- [x] 1.4 Extract and validate all flag values, construct `TestFlags` struct

## 2. File Discovery (Rust)

- [x] 2.1 Implement `discover_test_files(paths: &[String]) -> Vec<PathBuf>` in `cli/src/test_runner.rs`
- [x] 2.2 Support both file paths (pass through) and directory paths (recursive walk)
- [x] 2.3 Match patterns: `*_test.{ts,tsx,js,jsx}`, `*.test.{ts,tsx,js,jsx}`
- [x] 2.4 Skip `.` prefixed directories and `node_modules`
- [x] 2.5 Sort discovered files deterministically

## 3. Built-in Test Bootstrap (JS)

- [x] 3.1 Create `ext/runtime/js/test_bootstrap.js` with `Deno.test()` shim (all 5 signatures from proposal 001)
- [x] 3.2 Implement `TestContext` with full Deno API: `name`, `origin`, `parent`, all `step()` overloads
- [x] 3.3 Accept `sanitizeOps`, `sanitizeResources`, `sanitizeExit`, `permissions` on TestDefinition (warn + ignore)
- [x] 3.4 Implement `__test_runner.run(filter, failFast)` that executes tests and returns structured JSON results
- [x] 3.5 Return result schema: `{ passed, failed, ignored, results: [{ file, name, status, duration, error?, steps }] }`

## 4. Worker Integration (Rust)

- [x] 4.1 Add `test_mode: bool` field to `WorkerContextInitOpts` in `ext/workers/context.rs`
- [x] 4.2 In worker init (`crates/base/src/runtime/mod.rs`), when `test_mode=true`, execute `test_bootstrap.js` via `execute_script()` before main module
- [x] 4.3 Conditionally load test_bootstrap in `ext/runtime/js/bootstrap.js` (or directly from Rust via `include_str!`)

## 5. Test Orchestration (Rust)

- [x] 5.1 Create `cli/src/test_runner.rs` with `run_tests()` entry point
- [x] 5.2 Create test worker via `WorkerBuilder` with `test_mode: true`
- [x] 5.3 For each test file: load as ES module via synthetic entrypoint with relative imports bundled into eszip
- [x] 5.4 After all files loaded: call `__test_runner.run(filter, failFast)` via `execute_script_with_locker()`
- [x] 5.5 Parse JSON results from JS into Rust `TestSummary` struct
- [ ] 5.6 Implement per-test timeout enforcement (global `--timeout` value)
- [x] 5.7 Return exit code: 0 on all pass, 1 on any failure

## 6. Reporters (Rust)

- [x] 6.1 Define `TestReporter` trait in `cli/src/test_reporter.rs`
- [x] 6.2 Implement `PrettyReporter`: ANSI colored output matching `deno test` format
- [x] 6.3 Implement `JunitReporter`: XML output to stdout or file (`--junit-path`)
- [x] 6.4 Wire reporter selection from `--reporter` flag

## 7. Validation

- [ ] 7.1 Add Rust integration test: `test` subcommand discovers and runs test files
- [ ] 7.2 Add Rust integration test: `--filter` flag filters by test name
- [ ] 7.3 Add Rust integration test: `--fail-fast` stops on first failure
- [ ] 7.4 Add Rust integration test: exit code 0 on all pass, 1 on failure
- [ ] 7.5 Add Rust integration test: `--reporter junit` produces valid XML
- [ ] 7.6 Add Rust integration test: `--timeout` kills long-running tests
- [ ] 7.7 Verify `@std/assert` imports work in test files
- [ ] 7.8 Verify Docker: `docker run edge-runtime test /path/to/tests/`
- [x] 7.9 Write sample test suite exercising all `Deno.test()` signatures + `TestContext` API

## 8. Documentation

- [x] 8.1 Add `--help` text for `test` subcommand (auto-generated from clap)
- [ ] 8.2 Update README with `edge-runtime test` usage examples
