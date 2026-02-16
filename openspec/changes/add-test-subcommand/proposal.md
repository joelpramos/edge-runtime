# Change: Add Native `test` CLI Subcommand

## Why

Proposal 001 (`add-js-test-runner`) validates that testing inside the edge runtime works and provides an immediate workaround, but it has fundamental limitations:

1. Not a first-class CLI experience (`start --test --main-service test-runner.ts` vs `edge-runtime test`)
2. Tests run in main worker context only — no way to test in the user worker sandbox that matches production
3. File discovery, filtering, and timeout are managed via env vars rather than CLI flags
4. The test runner is a standalone `.ts` file the user must obtain and manage
5. Not upstreamable as a proper Supabase feature

This proposal adds a native `test` subcommand to the edge-runtime CLI, bringing `deno test`-equivalent functionality built into the binary.

See: https://github.com/supabase/edge-runtime/issues/661

**Depends on**: `add-js-test-runner` (validates the concept; shared JS test infrastructure patterns)

## What Changes

### Rust — CLI (`cli/src/`)
- Add `test` subcommand to `get_cli()` in `flags.rs` with flags: paths, `--filter`, `--worker-kind`, `--timeout`, `--fail-fast`, `--reporter`, `--junit-path`, `--disable-module-cache`
- Add `Some(("test", ...))` match arm in `main.rs`
- Implement Rust-side test file discovery: recursive walk with `*_test.ts`, `*.test.ts`, etc.

### Rust — Test Orchestration (`cli/src/test_runner.rs`)
- Create a main worker programmatically (reusing `WorkerBuilder` from `crates/base/src/worker/`)
- Inject the test bootstrap JS into the worker before loading test files
- Load each test file as an ES module
- Collect structured results from the worker
- Report results and exit with appropriate code

### JavaScript — Built-in Test Bootstrap (`ext/runtime/js/test_bootstrap.js`)
- Embed the `Deno.test()` shim + `TestContext` + test runner as a built-in JS module
- Loaded conditionally when the worker is in test mode
- Reuses patterns validated in proposal 001's `test-runner.ts`

### Rust — Worker Integration (`crates/base/`)
- Add test mode flag to `WorkerContextInitOpts` or `ServerFlags`
- Conditionally load `test_bootstrap.js` during worker init when in test mode
- Support both main worker and user worker contexts for test execution

## Impact

- Affected specs: `test-cli` (new capability)
- Affected Rust code: `cli/src/flags.rs`, `cli/src/main.rs`, new `cli/src/test_runner.rs`
- Affected Rust crate: `crates/base/src/worker/`, `crates/base/src/runtime/mod.rs` (test mode flag)
- Affected JS: new `ext/runtime/js/test_bootstrap.js`, minor change to `ext/runtime/js/bootstrap.js`
- Relationship to `test-runner` (proposal 001): The built-in test bootstrap reuses the same `Deno.test()` shim, `TestContext` API, and execution patterns. The CLI subcommand replaces the `start --test --main-service` invocation pattern.
