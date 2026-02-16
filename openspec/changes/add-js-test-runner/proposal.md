# Change: Add JS-Based Test Runner for Edge Runtime

## Why

Developers writing Supabase Edge Functions have no way to run tests against the actual runtime environment. Running `deno test` is insufficient because the edge runtime is a Deno 2.1.4 fork with custom API surface (`EdgeRuntime.*`, `Supabase.*`, patched `process.env`, `Deno.env` via `SupaEnv`, blocklisted FS APIs in user workers). This causes false positives/negatives in tests.

See: https://github.com/supabase/edge-runtime/issues/661

## What Changes

### Rust (minimal)
- Add `--test` boolean flag to the `start` command (`cli/src/flags.rs`)
- When `--test` is set, auto-terminate the process after the main service's initial module evaluation completes (`cli/src/main.rs`)

### TypeScript (test-runner.ts)
- Add a pure TypeScript test runner that runs as a main service via `start --test --main-service`
- Shim `Deno.test()` onto `globalThis.Deno` with full API parity to Deno's `Deno.test()`
- Implement `TestContext` matching Deno's API (`name`, `origin`, `parent`, all `step()` overloads)
- Discover test files using Deno conventions (`*_test.ts`, `*.test.ts`, etc.)
- Execute collected tests sequentially with colored console output
- Support test steps, filtering, `ignore`, `only`, and fail-fast
- JUnit XML reporter for CI integration
- Exit with appropriate code via `Deno.exit()`

## Impact

- Affected specs: `test-runner` (new capability)
- Affected Rust code: `cli/src/flags.rs` (add flag), `cli/src/main.rs` (auto-terminate logic)
- Affected TypeScript code: New file `test-runner.ts`
- Strives for functional parity with `deno test` where portable
