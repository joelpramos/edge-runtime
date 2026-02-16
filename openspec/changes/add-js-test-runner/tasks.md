## 1. Rust: `--test` Flag

- [ ] 1.1 Add `--test` boolean flag to `start` subcommand in `cli/src/flags.rs`
- [ ] 1.2 Extract `--test` flag in `cli/src/main.rs` and pass to server builder
- [ ] 1.3 Implement auto-terminate: when `--test` is set, exit after main service module evaluation completes
- [ ] 1.4 Propagate exit code from main worker (`Deno.exit(code)`) to process exit code

## 2. Core Test Runner (test-runner.ts)

- [ ] 2.1 Implement `Deno.test()` shim with all 5 call signatures (name+fn, fn, options, `.ignore`, `.only`)
- [ ] 2.2 Accept `sanitizeOps`, `sanitizeResources`, `sanitizeExit`, and `permissions` on `TestDefinition` (log warning, ignore)
- [ ] 2.3 Implement `TestContext` with full API: `name`, `origin`, `parent` properties
- [ ] 2.4 Implement `TestContext.step()` with all 3 overloads (name+fn, fn, definition object)
- [ ] 2.5 Support nested steps (child step's `parent` points to enclosing `TestContext`)
- [ ] 2.6 Implement recursive test file discovery (`*_test.ts`, `*.test.ts`, `*_test.js`, `*.test.js`)
- [ ] 2.7 Implement sequential test execution with `ignore` and `only` support
- [ ] 2.8 Implement `TEST_FILTER` env var (substring match on test name)
- [ ] 2.9 Implement `TEST_FAIL_FAST` env var (stop on first failure)

## 3. Reporting & Exit

- [ ] 3.1 Implement pretty reporter: ANSI colored output (green=pass, red=fail, yellow=ignored) with durations
- [ ] 3.2 Implement failure detail output (error messages + truncated stack traces)
- [ ] 3.3 Implement summary line (`test result: <status>. N passed; M failed; K ignored (duration)`)
- [ ] 3.4 Implement JUnit XML reporter (`TEST_REPORTER=junit`, `TEST_JUNIT_PATH` for file output)
- [ ] 3.5 Exit via `Deno.exit(1)` on failure, `Deno.exit(0)` on success

## 4. Validation

- [ ] 4.1 Write sample test files exercising all `Deno.test()` signatures and `TestContext` API
- [ ] 4.2 Verify test runner works inside Docker with `start --test --main-service`
- [ ] 4.3 Verify `@std/assert` imports work (JSR support)
- [ ] 4.4 Verify all env vars: `TEST_DIR`, `TEST_FILTER`, `TEST_FAIL_FAST`, `TEST_REPORTER`, `TEST_JUNIT_PATH`
- [ ] 4.5 Verify `--test` flag auto-terminates with correct exit code
- [ ] 4.6 Verify JUnit XML output is valid and parseable by CI systems
