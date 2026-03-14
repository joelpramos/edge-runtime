# Proposal 002: Native `test` CLI Subcommand

| Field         | Value                                           |
|---------------|-------------------------------------------------|
| **Status**    | Draft                                           |
| **Author**    | @joelpramos                                     |
| **Created**   | 2026-02-16                                      |
| **Requires**  | Rust changes to `cli/`, `ext/runtime/`, `crates/base/` |
| **Complexity**| High                                            |
| **Risk**      | Medium                                          |
| **Depends On**| Proposal 001 (validates the concept)             |

## Problem Statement

Same as [Proposal 001](./001-js-test-runner.md). Developers need to run tests against
the real Supabase Edge Runtime environment.

Proposal 001 provides an immediate workaround, but it has fundamental limitations:

1. It's not a first-class CLI experience (`start --main-service test-runner.ts` vs
   `edge-runtime test`)
2. Tests run in main worker context, not user worker context (different sandbox,
   different API restrictions)
3. No file discovery flags, no `--filter`, no `--watch`
4. No integration path with CI tooling or IDE test runners
5. Requires a dummy HTTP server to keep the process alive

This proposal adds a native `test` subcommand to the edge-runtime CLI, bringing
`deno test`-equivalent functionality to the Supabase ecosystem.

## Proposed Solution

Add a `test` subcommand to the edge-runtime binary that:

1. Discovers test files from specified paths
2. Boots a worker (main or user, configurable) with `Deno.test()` exposed
3. Loads and executes each test file
4. Collects results and reports to stdout
5. Exits with appropriate code

### Usage

```bash
# Run all tests in a directory
edge-runtime test ./functions/tests/

# Run a specific test file
edge-runtime test ./functions/tests/env_test.ts

# Filter tests by name
edge-runtime test --filter "process.env" ./functions/tests/

# Run in user worker context (matching production sandbox)
edge-runtime test --worker-kind user ./functions/tests/

# With verbose output
edge-runtime test -v ./functions/tests/

# Docker equivalent
docker run --rm -v $(pwd):/home/deno supabase/edge-runtime:v1.70.0 \
  test /home/deno/functions/tests/
```

### Console Output

```
running 3 tests from ./functions/tests/env_test.ts
  process.env has own properties ... ok (2ms)
  process.env.get works ... ok (1ms)
  Deno.env matches process.env ... ok (1ms)

running 2 tests from ./functions/tests/handler_test.ts
  handler returns 200 ... ok (15ms)
  handler returns JSON ... ok (12ms)

test result: ok. 5 passed; 0 failed; 0 ignored (30ms)
```

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│ edge-runtime test ./functions/tests/                     │
│                                                          │
│ ┌──────────────┐    ┌──────────────────────────────────┐ │
│ │ CLI Layer    │    │ Test Runner                      │ │
│ │ (flags.rs)   │───>│ (main.rs test subcommand)        │ │
│ │              │    │                                  │ │
│ │ --filter     │    │ 1. Discover test files           │ │
│ │ --worker-kind│    │ 2. For each file:                │ │
│ │ paths...     │    │    a. Create worker              │ │
│ └──────────────┘    │    b. Inject Deno.test shim      │ │
│                     │    c. Load test file              │ │
│                     │    d. Collect & run tests         │ │
│                     │    e. Report results              │ │
│                     │ 3. Aggregate & exit               │ │
│                     └──────────────────────────────────┘ │
│                              │                           │
│                              v                           │
│                     ┌──────────────────────────────────┐ │
│                     │ Worker (JsRuntime)               │ │
│                     │                                  │ │
│                     │ Same bootstrap as start command: │ │
│                     │ - Deno.env (SupaEnv)             │ │
│                     │ - process.env (Proxy)            │ │
│                     │ - EdgeRuntime namespace          │ │
│                     │ - Node.js compat layer           │ │
│                     │ + Deno.test() (shim)             │ │
│                     └──────────────────────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

## Implementation Plan

### Phase 1: CLI Subcommand (Rust)

#### 1a. Add `test` subcommand to `cli/src/flags.rs`

```rust
// Add to get_cli() function, after .subcommand(get_unbundle_command())
.subcommand(get_test_command())
```

```rust
fn get_test_command() -> Command {
  Command::new("test")
    .about("Run tests against the Edge Runtime environment")
    .arg(
      arg!([paths] ... "Paths to test files or directories")
        .default_value(".")
        .value_parser(value_parser!(String)),
    )
    .arg(
      arg!(--"filter" <PATTERN>)
        .help("Run only tests matching this string/pattern"),
    )
    .arg(
      arg!(--"worker-kind" <KIND>)
        .help("Worker context to run tests in")
        .default_value("main")
        .value_parser(["main", "user"]),
    )
    .arg(
      arg!(--"disable-module-cache")
        .help("Disable using module cache")
        .default_value("false")
        .value_parser(FalseyValueParser::new()),
    )
    .arg(
      arg!(--"timeout" <MILLISECONDS>)
        .help("Per-test timeout in milliseconds")
        .default_value("30000")
        .value_parser(value_parser!(u64)),
    )
}
```

#### 1b. Add `test` match arm to `cli/src/main.rs`

```rust
Some(("test", sub_matches)) => {
  deno_telemetry::init(
    deno::versions::otel_runtime_config(),
    OtelConfig::default(),
  )?;

  let paths: Vec<String> = sub_matches
    .get_many::<String>("paths")
    .unwrap()
    .cloned()
    .collect();

  let filter = sub_matches.get_one::<String>("filter").cloned();

  let worker_kind = sub_matches
    .get_one::<String>("worker-kind")
    .map(|s| s.as_str())
    .unwrap_or("main");

  let no_module_cache = sub_matches
    .get_one::<bool>("disable-module-cache")
    .cloned()
    .unwrap();

  let timeout_ms = sub_matches
    .get_one::<u64>("timeout")
    .cloned()
    .unwrap();

  // Discover test files
  let test_files = discover_test_files(&paths)?;

  if test_files.is_empty() {
    eprintln!("No test files found");
    return Ok(ExitCode::FAILURE);
  }

  // Run tests using a test runner main service
  let test_runner_result = run_tests(
    test_files,
    filter,
    worker_kind,
    no_module_cache,
    timeout_ms,
  ).await?;

  if test_runner_result.failed > 0 {
    ExitCode::FAILURE
  } else {
    ExitCode::SUCCESS
  }
}
```

### Phase 2: Test Runner Core (Rust)

The test runner creates a worker that executes a built-in test harness JS module.

#### Approach A: Reuse `TestBed` (simpler, main worker only)

Leverage the existing `TestBedBuilder` from `crates/base/src/utils/test_utils.rs`:

```rust
async fn run_tests(
  test_files: Vec<PathBuf>,
  filter: Option<String>,
  worker_kind: &str,
  no_module_cache: bool,
  timeout_ms: u64,
) -> Result<TestSummary, Error> {
  // Create a temporary main service that is the test runner
  let test_runner_code = generate_test_runner_module(&test_files, &filter);

  // Write to temp file
  let temp_dir = tempfile::tempdir()?;
  let runner_path = temp_dir.path().join("__test_runner.ts");
  std::fs::write(&runner_path, test_runner_code)?;

  // Create a worker and run the test runner module
  let worker_surface = worker::WorkerSurfaceBuilder::new()
    .init_opts(WorkerContextInitOpts {
      service_path: runner_path,
      no_module_cache,
      env_vars: std::env::vars().collect(),
      conf: WorkerRuntimeOpts::MainWorker(MainWorkerRuntimeOpts {
        // No worker pool needed for test runner
        worker_pool_tx: dummy_pool_tx,
        shared_metric_src: None,
        event_worker_metric_src: None,
        context: None,
      }),
      ..Default::default()
    })
    .build()
    .await?;

  // Wait for the worker to complete (it exits after running tests)
  // Parse results from worker output
  // ...
}
```

#### Approach B: Direct `JsRuntime` (more control, both worker kinds)

For full control, create a `JsRuntime` directly:

```rust
async fn run_tests(/* ... */) -> Result<TestSummary, Error> {
  let mut runtime = JsRuntime::new(RuntimeOptions {
    extensions: get_test_extensions(worker_kind),
    module_loader: Some(Rc::new(module_loader)),
    ..Default::default()
  });

  // Inject Deno.test shim + test runner
  runtime.execute_script(
    "test_bootstrap",
    include_str!("../js/test_bootstrap.js"),
  )?;

  // Load each test file as a module
  for file in &test_files {
    let module_id = runtime.load_main_module(
      &ModuleSpecifier::from_file_path(file)?,
      None,
    ).await?;
    runtime.mod_evaluate(module_id).await?;
  }

  // Execute the test runner
  let result = runtime.execute_script(
    "run_tests",
    "__test_runner.run()",
  )?;

  // ... parse results
}
```

**Recommendation**: Start with Approach A. It reuses proven infrastructure and avoids
reimplementing the complex extension registration. Approach B can be pursued later if
user worker context testing is needed.

### Phase 3: Test Bootstrap JS

#### File: `ext/runtime/js/test_bootstrap.js`

This is the JS module injected into the worker before test files are loaded. It
provides the `Deno.test()` shim and test execution logic.

```javascript
// This module is loaded BEFORE user test files.
// It installs Deno.test() and provides a run() function.

const __test_definitions = [];

function denoTest(nameOrOptOrFn, maybeFn) {
  let def;

  if (typeof nameOrOptOrFn === "function") {
    def = { name: nameOrOptOrFn.name || "<anonymous>", fn: nameOrOptOrFn };
  } else if (typeof nameOrOptOrFn === "string") {
    def = { name: nameOrOptOrFn, fn: maybeFn };
  } else {
    def = {
      name: nameOrOptOrFn.name,
      fn: nameOrOptOrFn.fn,
      ignore: nameOrOptOrFn.ignore ?? false,
      only: nameOrOptOrFn.only ?? false,
    };
  }

  __test_definitions.push(def);
}

denoTest.ignore = (name, fn) => {
  __test_definitions.push({ name, fn, ignore: true });
};

denoTest.only = (name, fn) => {
  __test_definitions.push({ name, fn, only: true });
};

// Install on Deno namespace
Object.defineProperty(Deno, "test", {
  value: denoTest,
  writable: false,
  configurable: true,
});

// Test context for steps
class TestContext {
  #steps = [];

  get steps() { return this.#steps; }

  async step(name, fn) {
    const start = performance.now();
    try {
      await fn();
      this.#steps.push({ name, ok: true, duration: performance.now() - start });
      return true;
    } catch (error) {
      this.#steps.push({
        name,
        ok: false,
        duration: performance.now() - start,
        error: String(error),
      });
      return false;
    }
  }
}

// Runner - called from Rust after all test files are loaded
globalThis.__test_runner = {
  async run(filter) {
    let tests = __test_definitions;
    const onlyTests = tests.filter(t => t.only);
    if (onlyTests.length > 0) tests = onlyTests;
    if (filter) tests = tests.filter(t => t.name.includes(filter));

    let passed = 0, failed = 0, ignored = 0;
    const failures = [];

    for (const test of tests) {
      if (test.ignore) {
        console.log(`  ${test.name} ... \x1b[33mignored\x1b[0m`);
        ignored++;
        continue;
      }

      const ctx = new TestContext();
      const start = performance.now();

      try {
        await test.fn(ctx);
        const failedSteps = ctx.steps.filter(s => !s.ok);
        if (failedSteps.length > 0) throw new Error(`Step "${failedSteps[0].name}" failed`);

        const dur = performance.now() - start;
        console.log(`  ${test.name} ... \x1b[32mok\x1b[0m \x1b[2m(${Math.round(dur)}ms)\x1b[0m`);
        passed++;
      } catch (error) {
        const dur = performance.now() - start;
        console.log(`  ${test.name} ... \x1b[31mFAILED\x1b[0m \x1b[2m(${Math.round(dur)}ms)\x1b[0m`);
        failures.push({ name: test.name, error: String(error) });
        failed++;
      }
    }

    // Return structured result for Rust to parse
    return JSON.stringify({ passed, failed, ignored, failures });
  }
};
```

### Phase 4: File Discovery (Rust)

```rust
fn discover_test_files(paths: &[String]) -> Result<Vec<PathBuf>, Error> {
  let test_pattern = regex::Regex::new(r"(?:_test|\.test)\.[tj]sx?$")?;
  let mut files = Vec::new();

  for path in paths {
    let path = PathBuf::from(path);
    if path.is_file() {
      files.push(path);
    } else if path.is_dir() {
      walk_dir(&path, &test_pattern, &mut files)?;
    }
  }

  files.sort();
  Ok(files)
}

fn walk_dir(
  dir: &Path,
  pattern: &regex::Regex,
  files: &mut Vec<PathBuf>,
) -> Result<(), Error> {
  for entry in std::fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();

    if path.is_dir() {
      let name = path.file_name().unwrap().to_str().unwrap_or("");
      if !name.starts_with('.') && name != "node_modules" {
        walk_dir(&path, pattern, files)?;
      }
    } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
      if pattern.is_match(name) {
        files.push(path);
      }
    }
  }
  Ok(())
}
```

## Files Changed

| File | Change | Complexity |
|------|--------|------------|
| `cli/src/flags.rs` | Add `get_test_command()` + wire into `get_cli()` | Low |
| `cli/src/main.rs` | Add `Some(("test", ...))` match arm | Medium |
| `cli/src/test_runner.rs` | New file: `run_tests()`, `discover_test_files()` | High |
| `ext/runtime/js/test_bootstrap.js` | New file: `Deno.test` shim + runner | Medium |
| `ext/runtime/js/bootstrap.js` | Conditionally load test bootstrap | Low |
| `crates/base/src/worker/mod.rs` | Possibly: add test mode to worker creation | Low-Medium |
| `Cargo.toml` | Add `regex` dep (if not already present) | Trivial |

## Comparison: Upstream Deno vs This Proposal

| Aspect | Upstream Deno `test` | This Proposal |
|--------|---------------------|---------------|
| File discovery | Built-in with glob patterns | Same patterns, simpler implementation |
| `Deno.test()` | Native op (`op_register_test`) | JS shim (collects in array) |
| Test execution | Event-driven via channels | Sequential in JS |
| Reporters | Multiple (pretty, dot, junit, tap) | Pretty only (extensible later) |
| Sanitizers | Op, resource, exit sanitizers | None (not applicable to edge runtime) |
| `--watch` mode | Built-in file watcher | Not included (future work) |
| `--parallel` | Multiple worker threads | Not included (future work) |
| `--coverage` | V8 coverage integration | Not included (future work) |
| Worker context | Always "main" Deno process | Configurable: main or user worker |

## Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Worker bootstrap fails in test mode | Medium | High | Reuse existing `TestBed` infrastructure which is proven |
| Module loading differs from `start` | Low | High | Same module loader code path, same `EmitterFactory` |
| Merge conflicts with upstream | Medium | Medium | Keep changes minimal and isolated to new files |
| Build complexity for contributors | Low | Low | Standard `cargo build`, no new dependencies |
| Test shim doesn't cover all `Deno.test` overloads | Low | Low | Implement incrementally, start with common forms |

## Phased Delivery

### v1: Minimal Viable (Targets this proposal)
- `test` subcommand with file paths
- `--filter` flag
- Main worker context only
- Pretty reporter
- `Deno.test()` basic forms (name+fn, options object, named function)
- `t.step()` for nested tests

### v2: Enhanced (Future)
- `--worker-kind user` for user worker sandbox testing
- `--watch` mode
- JUnit/TAP output for CI
- `--timeout` per-test enforcement
- Glob patterns for file selection

### v3: Full Parity (Future)
- `--coverage` via V8 coverage
- `--parallel` execution
- `--doc` for testing code blocks in markdown
- IDE integration (LSP test discovery)

## Decision Matrix: Proposal 001 vs 002

| Criterion               | 001 (JS Runner)  | 002 (CLI Subcommand) |
|--------------------------|------------------|----------------------|
| Time to implement        | Hours            | Days-Weeks           |
| Rust knowledge required  | None             | Yes                  |
| Works with stock Docker  | Yes              | Requires custom build|
| First-class UX           | No               | Yes                  |
| User worker testing      | No               | Planned (v2)         |
| CI integration           | Workable         | Native               |
| Upstreamable to Supabase | Unlikely         | Yes                  |
| Maintenance burden       | Low              | Medium               |

**Recommendation**: Implement Proposal 001 immediately for unblocking development.
Use it to validate patterns and gather requirements. Then pursue Proposal 002 for a
proper upstream contribution to `supabase/edge-runtime`.
