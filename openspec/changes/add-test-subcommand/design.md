## Context

Proposal 001 (`add-js-test-runner`) adds a JS test runner that works as a main service via `start --test --main-service`. It validates that testing inside the edge runtime is viable but has UX and architectural limitations. This proposal elevates testing to a first-class CLI subcommand.

The edge runtime is a Deno 2.1.4 fork. Upstream Deno's `deno test` is implemented via:
- Rust-side file discovery and test orchestration (`cli/tools/test/mod.rs`)
- A JS bootstrap (`cli/js/40_test.js`) that shims `Deno.test()` and hooks into Rust ops (`op_register_test`, `op_register_test_step`)
- An event-driven channel system (`TestEvent` enum) for result collection
- A reporter trait with pretty, dot, TAP, and JUnit implementations

This proposal takes a **pragmatic middle ground**: Rust for the CLI and orchestration, but JS for test registration and execution (avoiding the complexity of Deno's op-based test registration and channel system).

### Reference Architecture: Upstream Deno vs This Proposal

| Component | Upstream Deno | This Proposal |
|-----------|--------------|---------------|
| File discovery | Rust (`is_supported_test_path`) | Rust (same patterns) |
| Test registration | Rust ops (`op_register_test`) | JS shim (array collection) |
| Test execution | Rust event loop + JS calls | JS sequential execution |
| Result collection | Rust channels (`TestEvent`) | JS→Rust structured JSON |
| Reporting | Rust trait (`TestReporter`) | Rust trait (same pattern) |
| Worker isolation | Per-file worker | Single worker, per-file import |
| Hooks (beforeAll etc) | Yes (`op_register_test_hook`) | Future work |

## Goals / Non-Goals

- **Goals:**
  - First-class `edge-runtime test` CLI command
  - CLI flags matching `deno test` conventions: `--filter`, `--fail-fast`, `--reporter`, `--junit-path`
  - Worker kind selection: `--worker-kind main|user` to test in either context
  - Per-test timeout enforcement via `--timeout`
  - Built-in test bootstrap (no external file to manage)
  - Functional parity with proposal 001's `Deno.test()` API and `TestContext`
  - Upstreamable to `supabase/edge-runtime`

- **Non-Goals:**
  - Per-file worker isolation (Deno's pattern — too complex for v1)
  - Parallel test execution
  - Resource/op/exit sanitizers (Deno V8 internals, not portable)
  - Code coverage
  - Watch mode (external watchers work)
  - Test hooks (`beforeAll`, `afterAll`) — future work
  - Doc tests (`--doc`)

## Decisions

### 1. Reuse `WorkerBuilder` for test worker creation

Create a main or user worker programmatically using the existing `WorkerBuilder` from `crates/base/src/worker/worker_inner.rs`. This reuses the proven worker initialization pipeline (module loader, permissions, extensions, bootstrap).

- *Alternative: Create `JsRuntime` directly* — More control but requires reimplementing extension registration, module loading, and bootstrap. The `WorkerBuilder` already handles all of this.
- *Alternative: Reuse `TestBed` from test_utils.rs* — `TestBed` is designed for integration tests (creates full server + worker pool). Too heavy for the test CLI.

The approach is to build a worker with `eager_module_init: true` and inject the test bootstrap JS before loading test files.

### 2. JS test execution, Rust reporting

Tests are collected and executed in JS (sequential, same as proposal 001). Results are returned to Rust as structured JSON via a global `__test_runner.run()` call. Rust handles reporting (pretty, JUnit) via a `TestReporter` trait.

- *Alternative: Full Rust op-based approach (like upstream Deno)* — Requires implementing `op_register_test`, `op_register_test_step`, `TestEvent` channels, and the full event-driven system. Significant complexity for marginal benefit.
- *Alternative: All-JS reporting* — Simpler but harder to extend (adding new reporters means modifying JS). Rust reporting is more maintainable and matches upstream Deno's pattern.

### 3. Worker kind selection

The `--worker-kind` flag selects between `WorkerRuntimeOpts::MainWorker` and `WorkerRuntimeOpts::UserWorker` when creating the test worker. User worker mode applies the same sandbox restrictions as production (limited `Deno.sys`, no `Deno.run`, etc.) while still injecting the test bootstrap.

For user worker mode, we need a dummy `worker_pool_tx` channel (user workers can't spawn other workers in test mode).

### 4. Test bootstrap as a built-in extension JS

The test bootstrap JS (`ext/runtime/js/test_bootstrap.js`) is loaded via `execute_script()` during worker initialization when a test mode flag is set. This approach:
- Doesn't require modifying the module loader
- Runs before any user code
- Can set up `Deno.test()` on `globalThis` before test files are imported

The bootstrap is a compile-time embedded string (`include_str!`).

### 5. File discovery in Rust

Rust handles test file discovery (recursive directory walk with pattern matching), matching Deno's conventions. Paths are passed as CLI positional args. This gives us:
- Proper error handling for missing paths
- Glob support (future)
- Consistent behavior across platforms

### 6. Per-test timeout via `tokio::time::timeout`

Wrap each test execution in a Rust-side timeout. The JS test runner signals test start/end to Rust, which enforces the timeout. If a test exceeds the timeout, the worker is terminated and the test is reported as failed.

For v1, the timeout is a single global value (`--timeout`). Per-test `timeout` in `TestDefinition` is future work.

## Risks / Trade-offs

- **Rust surface area** → More code to maintain than proposal 001. Mitigation: Keep the Rust layer thin (CLI + orchestration), delegate test logic to JS.
- **Worker bootstrap changes** → Modifying worker init to support test mode could affect existing behavior. Mitigation: Test mode is opt-in (flag-gated), existing code paths unchanged.
- **User worker sandbox gaps** → User workers may not have access to APIs needed for test assertions. Mitigation: The test bootstrap runs before sandbox restrictions; `@std/assert` uses standard JS only.
- **Merge conflicts with upstream** → New files in `cli/` and `ext/runtime/js/`. Mitigation: Keep changes in new files where possible; minimize modifications to existing files.
- **JSON result passing** → Passing test results as JSON from JS to Rust has a serialization cost. Mitigation: Negligible for typical test suites; can be optimized later with ops if needed.

## File Changes

| File | Change | Complexity |
|------|--------|------------|
| `cli/src/flags.rs` | Add `get_test_command()` + wire into `get_cli()` | Low |
| `cli/src/main.rs` | Add `Some(("test", ...))` match arm | Medium |
| `cli/src/test_runner.rs` | **New**: `run_tests()`, `discover_test_files()`, `TestReporter` trait | High |
| `cli/src/test_reporter.rs` | **New**: `PrettyReporter`, `JunitReporter` | Medium |
| `ext/runtime/js/test_bootstrap.js` | **New**: `Deno.test` shim + TestContext + runner | Medium |
| `ext/runtime/js/bootstrap.js` | Conditionally load test_bootstrap when in test mode | Low |
| `crates/base/src/worker/worker_inner.rs` | Add test mode support to `WorkerBuilder` | Low |
| `ext/workers/context.rs` | Add `test_mode: bool` to `WorkerContextInitOpts` | Trivial |
| `Cargo.toml` (cli) | Add `regex` dep if needed | Trivial |

## Phased Delivery

### v1 (this proposal)
- `test` subcommand with path args and `--filter`
- Main worker context only (`--worker-kind main`)
- Pretty reporter + JUnit XML
- `--fail-fast`, `--timeout`
- Built-in test bootstrap with `Deno.test()` shim + `TestContext`

### v2 (future)
- `--worker-kind user` for sandbox testing
- `--watch` mode
- Test hooks (`beforeAll`, `afterAll`, `beforeEach`, `afterEach`)
- Glob patterns for file selection
- Per-test timeout from `TestDefinition`

### v3 (future)
- Per-file worker isolation
- Parallel execution (`--parallel`)
- DOT and TAP reporters
- `--coverage` (if V8 coverage can be wired through)
- IDE integration (LSP test discovery)
