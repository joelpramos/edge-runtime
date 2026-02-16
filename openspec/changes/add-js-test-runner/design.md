## Context

The edge runtime provides `start`, `bundle`, and `unbundle` CLI commands but no `test` command. The runtime is a Deno 2.1.4 fork with custom APIs that differ from vanilla Deno, making `deno test` unreliable for edge function testing.

Main workers have full Deno permissions (`Deno.readDir`, `Deno.env`, `Deno.exit`, dynamic imports, file system access), while user workers are sandboxed. The test runner targets the main worker context.

## Goals / Non-Goals

- **Goals:**
  - Enable running tests inside the actual edge runtime environment
  - Functional parity with `deno test` where portable (API surface, reporting, CI integration)
  - Familiar `Deno.test()` API matching all documented call signatures
  - `TestContext` matching Deno's API (`name`, `origin`, `parent`, all `step()` overloads)
  - CI-friendly: exit codes, JUnit XML output
  - Auto-termination via `--test` flag on `start` command

- **Non-Goals:**
  - IDE test runner integration (VS Code Deno extension)
  - Parallel test execution (sequential is safer; can be added later)
  - Resource/op/exit sanitizers (Deno V8 internals, not portable)
  - Code coverage (requires V8 integration not available in main workers)
  - Watch mode (external file watchers like `watchexec` work fine)
  - Per-test permissions (`permissions` option on `TestDefinition`) — main worker has all permissions

## Decisions

- **`--test` flag on `start` command**: Adding a boolean flag to `start` that auto-terminates the process after the main service's module evaluation completes. This eliminates the need for a minimal keepalive server and gives clean exit codes. Requires changes to `cli/src/flags.rs` (flag definition) and `cli/src/main.rs` (auto-terminate after main worker finishes).
  - *Alternative: Rely on `Deno.exit()` from JS only* — Works but requires the test runner to manage its own exit, and the `start` command still spins up the full HTTP server infrastructure unnecessarily.
  - *Alternative: Add a separate `test` CLI command* — Higher complexity, larger Rust surface area, harder to maintain. The `--test` flag achieves the same outcome with minimal changes.

- **Shims `Deno.test()` on `globalThis`**: The edge runtime doesn't expose `Deno.test`. The shim collects test definitions during import, then executes them. Matches all documented Deno `Deno.test()` signatures for portability.
  - *Alternative: Custom test API (e.g., `EdgeRuntime.test()`)* — Forces developers to learn a new API and makes test files non-portable between `deno test` and the edge runtime.

- **Full `TestContext` API**: Implements `name`, `origin`, `parent` properties and all three `step()` overloads (`step(name, fn)`, `step(fn)`, `step(definition)`) to match Deno's `TestContext`. Steps are nested — a step's `TestContext` has `parent` pointing to the enclosing context.

- **Sequential execution**: Simpler, safer for tests with shared state. Matches `deno test` default (which also runs tests within a file sequentially).

- **Environment variables for configuration**: `TEST_DIR`, `TEST_FILTER`, `TEST_FAIL_FAST`, `TEST_REPORTER` avoid the need for custom CLI flag parsing. Docker `--env` is the natural mechanism. Maps to `deno test` flags:
  - `TEST_DIR` → positional path args
  - `TEST_FILTER` → `--filter`
  - `TEST_FAIL_FAST` → `--fail-fast`
  - `TEST_REPORTER` → `--reporter` (values: `pretty` (default), `junit`)

- **JUnit XML reporter**: CI systems (GitHub Actions, GitLab CI, etc.) support JUnit XML natively. Output to stdout when `TEST_REPORTER=junit`, or to file via `TEST_JUNIT_PATH`.

## Risks / Trade-offs

- **Main worker vs user worker context** → Tests run with more permissions than deployed functions. Mitigation: Document this clearly. Future enhancement could spawn user workers for sandboxed testing.
- **`--test` flag requires Rust change** → Small surface area (flag definition + auto-terminate conditional). Low risk.
- **No sanitizers** → Deno's sanitizers (`sanitizeOps`, `sanitizeResources`, `sanitizeExit`) rely on V8 internals not exposed to JavaScript. Accept options on `TestDefinition` but ignore them (log a warning).
- **No coverage** → Deno's `--coverage` uses V8's built-in coverage support. Not portable. Users can use external tooling if needed.
- **Dynamic imports for test files** → Standard ESM behavior, no practical impact.

## Functional Parity Matrix

| `deno test` Feature | Edge Runtime Test Runner | Notes |
|---------------------|------------------------|-------|
| `Deno.test()` signatures | All 5 forms | Full parity |
| `TestContext` API | `name`, `origin`, `parent`, `step()` | Full parity |
| `TestDefinition` options | `name`, `fn`, `ignore`, `only` | `sanitize*` and `permissions` accepted but ignored |
| `--filter` | `TEST_FILTER` env var | Substring match |
| `--fail-fast` | `TEST_FAIL_FAST` env var | Stop on first failure |
| `--reporter pretty` | Default | ANSI colored output |
| `--reporter junit` / `--junit-path` | `TEST_REPORTER=junit` / `TEST_JUNIT_PATH` | JUnit XML output |
| `--parallel` | Not supported | Sequential only |
| `--shuffle` | Not supported | Deterministic order |
| `--coverage` | Not supported | V8 internal |
| `--watch` | Not supported | Use external watcher |
| Sanitizers | Accepted, ignored | V8 internal |
| Per-test permissions | Accepted, ignored | Main worker has all |
