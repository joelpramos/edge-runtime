# Proposal 001: JS-Based Test Runner for Edge Runtime

| Field         | Value                                    |
|---------------|------------------------------------------|
| **Status**    | Draft                                    |
| **Author**    | @joelpramos                              |
| **Created**   | 2026-02-16                               |
| **Requires**  | No runtime changes (works with v1.70.0+) |
| **Complexity**| Low                                      |
| **Risk**      | Low                                      |

## Problem Statement

Supabase Edge Runtime provides `start`, `bundle`, and `unbundle` CLI commands but no
`test` command. Developers writing edge functions have no way to run unit/integration
tests against the **actual runtime environment** their code will execute in.

Running tests with vanilla `deno test` is insufficient because:

- The edge runtime is a Deno 2.1.4 fork with custom API surface (`EdgeRuntime.*`,
  `Supabase.*`, patched `process.env`, `Deno.env` via `SupaEnv`, blocklisted FS APIs
  in user workers, etc.)
- API mismatches between vanilla Deno and the edge runtime cause false
  positives/negatives in tests
- Pinning Deno versions doesn't solve the problem since the Supabase customizations
  aren't present

See: https://github.com/supabase/edge-runtime/issues/661

## Proposed Solution

A **pure JavaScript test runner** that runs as a main service inside the existing
`start` command. Zero Rust changes required. Works with the current Docker image.

### Architecture

```
┌─────────────────────────────────────────────────┐
│  docker run supabase/edge-runtime:v1.70.0       │
│  start --main-service /home/deno/test-runner.ts │
│                                                 │
│  ┌───────────────────────────────────────────┐  │
│  │  test-runner.ts (Main Worker)             │  │
│  │                                           │  │
│  │  1. Shim globalThis.Deno.test             │  │
│  │  2. Discover *_test.ts / *.test.ts files  │  │
│  │  3. Dynamic import each test file         │  │
│  │  4. Execute collected test fns            │  │
│  │  5. Report results to console             │  │
│  │  6. Exit with appropriate code            │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
```

### Key Design Decisions

1. **Runs as a main worker** - Has filesystem access for test file discovery, access
   to all Deno APIs that main workers have, and can import user code.

2. **Shims `Deno.test()` into the global scope** - Since the edge runtime doesn't
   expose `Deno.test`, we define it ourselves before importing test files. The shim
   collects test definitions and executes them sequentially.

3. **Uses `Deno.exit()` for process exit** - Main workers have access to `Deno.exit`
   (or we throw to crash with non-zero exit code), giving CI-friendly exit codes.

4. **Supports `@std/assert`** - The edge runtime already supports JSR imports, so
   `assertEquals`, `assertExists`, etc. work out of the box.

### Test File Convention

Test files are discovered using the same conventions as Deno:
- `*_test.ts`, `*_test.js`
- `*.test.ts`, `*.test.js`
- Files in `__tests__/` directories

### API Surface

The shim implements a subset of `Deno.test()`:

```typescript
// Simple form
Deno.test("my test", () => {
  assertEquals(1 + 1, 2);
});

// Async
Deno.test("async test", async () => {
  const resp = await fetch("https://example.com");
  assertEquals(resp.status, 200);
});

// Options form
Deno.test({
  name: "with options",
  ignore: false,
  fn: () => { /* ... */ },
});

// Named function
Deno.test(function myTest() {
  // name inferred from function name
});

// Steps (nested tests)
Deno.test("parent", async (t) => {
  await t.step("child 1", () => { /* ... */ });
  await t.step("child 2", () => { /* ... */ });
});
```

### Usage

```bash
# Run all tests in ./functions/tests/
docker run --rm -v $(pwd):/home/deno supabase/edge-runtime:v1.70.0 \
  start --main-service /home/deno/test-runner.ts

# With environment variable to specify test directory
docker run --rm -v $(pwd):/home/deno \
  -e TEST_DIR=/home/deno/functions/tests \
  supabase/edge-runtime:v1.70.0 \
  start --main-service /home/deno/test-runner.ts

# With filter
docker run --rm -v $(pwd):/home/deno \
  -e TEST_FILTER="process.env" \
  supabase/edge-runtime:v1.70.0 \
  start --main-service /home/deno/test-runner.ts
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

## Implementation

### File: `test-runner.ts`

```typescript
const TEST_DIR = Deno.env.get("TEST_DIR") ?? "./functions/tests";
const TEST_FILTER = Deno.env.get("TEST_FILTER") ?? "";

// --- Types ---

interface TestDefinition {
  name: string;
  fn: (t: TestContext) => void | Promise<void>;
  ignore?: boolean;
  only?: boolean;
}

interface StepResult {
  name: string;
  passed: boolean;
  duration: number;
  error?: Error;
}

interface TestResult {
  name: string;
  passed: boolean;
  ignored: boolean;
  duration: number;
  error?: Error;
  steps: StepResult[];
}

class TestContext {
  #steps: StepResult[] = [];

  get steps(): StepResult[] {
    return this.#steps;
  }

  async step(
    name: string,
    fn: () => void | Promise<void>,
  ): Promise<boolean> {
    const start = performance.now();
    try {
      await fn();
      this.#steps.push({
        name,
        passed: true,
        duration: performance.now() - start,
      });
      return true;
    } catch (error) {
      this.#steps.push({
        name,
        passed: false,
        duration: performance.now() - start,
        error: error instanceof Error ? error : new Error(String(error)),
      });
      return false;
    }
  }
}

// --- Test Collection ---

const collectedTests: TestDefinition[] = [];

function denoTest(
  nameOrOptOrFn: string | TestDefinition | ((t: TestContext) => void | Promise<void>),
  maybeFn?: (t: TestContext) => void | Promise<void>,
): void {
  if (typeof nameOrOptOrFn === "function") {
    collectedTests.push({
      name: nameOrOptOrFn.name || "<anonymous>",
      fn: nameOrOptOrFn,
    });
  } else if (typeof nameOrOptOrFn === "string") {
    collectedTests.push({
      name: nameOrOptOrFn,
      fn: maybeFn!,
    });
  } else {
    collectedTests.push({
      name: nameOrOptOrFn.name,
      fn: nameOrOptOrFn.fn,
      ignore: nameOrOptOrFn.ignore,
      only: nameOrOptOrFn.only,
    });
  }
}

denoTest.ignore = (name: string, fn: (t: TestContext) => void | Promise<void>) => {
  collectedTests.push({ name, fn, ignore: true });
};

denoTest.only = (name: string, fn: (t: TestContext) => void | Promise<void>) => {
  collectedTests.push({ name, fn, only: true });
};

// Install the shim
(globalThis as any).Deno.test = denoTest;

// --- File Discovery ---

async function discoverTestFiles(dir: string): Promise<string[]> {
  const files: string[] = [];
  const testPattern = /(?:_test|\.test)\.[tj]sx?$/;

  async function walk(path: string) {
    for await (const entry of Deno.readDir(path)) {
      const fullPath = `${path}/${entry.name}`;
      if (entry.isDirectory && !entry.name.startsWith(".") && entry.name !== "node_modules") {
        await walk(fullPath);
      } else if (entry.isFile && testPattern.test(entry.name)) {
        files.push(fullPath);
      }
    }
  }

  await walk(dir);
  return files.sort();
}

// --- Reporter ---

const green = (s: string) => `\x1b[32m${s}\x1b[0m`;
const red = (s: string) => `\x1b[31m${s}\x1b[0m`;
const yellow = (s: string) => `\x1b[33m${s}\x1b[0m`;
const dim = (s: string) => `\x1b[2m${s}\x1b[0m`;

function formatDuration(ms: number): string {
  return ms < 1000 ? `${Math.round(ms)}ms` : `${(ms / 1000).toFixed(1)}s`;
}

// --- Runner ---

async function runTests(): Promise<void> {
  const testFiles = await discoverTestFiles(TEST_DIR);

  if (testFiles.length === 0) {
    console.log(yellow(`No test files found in ${TEST_DIR}`));
    console.log(dim(`(looking for *_test.ts, *.test.ts, *_test.js, *.test.js)`));
    return;
  }

  const allResults: { file: string; results: TestResult[] }[] = [];
  let totalPassed = 0;
  let totalFailed = 0;
  let totalIgnored = 0;

  for (const file of testFiles) {
    // Clear collected tests before each file
    collectedTests.length = 0;

    // Import the test file - this triggers Deno.test() calls
    await import(file);

    // Apply filter
    let testsToRun = collectedTests.filter((t) =>
      TEST_FILTER ? t.name.includes(TEST_FILTER) : true
    );

    // Handle .only
    const onlyTests = testsToRun.filter((t) => t.only);
    if (onlyTests.length > 0) {
      testsToRun = onlyTests;
    }

    if (testsToRun.length === 0) continue;

    const relativePath = file.replace(/^\.\//, "");
    console.log(`\nrunning ${testsToRun.length} tests from ${relativePath}`);

    const fileResults: TestResult[] = [];

    for (const test of testsToRun) {
      if (test.ignore) {
        console.log(`  ${test.name} ... ${yellow("ignored")}`);
        fileResults.push({
          name: test.name,
          passed: true,
          ignored: true,
          duration: 0,
          steps: [],
        });
        totalIgnored++;
        continue;
      }

      const ctx = new TestContext();
      const start = performance.now();

      try {
        await test.fn(ctx);

        // Check if any steps failed
        const failedSteps = ctx.steps.filter((s) => !s.passed);
        const duration = performance.now() - start;

        if (failedSteps.length > 0) {
          throw failedSteps[0].error ?? new Error(`Step "${failedSteps[0].name}" failed`);
        }

        console.log(`  ${test.name} ... ${green("ok")} ${dim(`(${formatDuration(duration)})`)}`);
        if (ctx.steps.length > 0) {
          for (const step of ctx.steps) {
            const status = step.passed ? green("ok") : red("FAILED");
            console.log(`    ${step.name} ... ${status} ${dim(`(${formatDuration(step.duration)})`)}`);
          }
        }

        fileResults.push({
          name: test.name,
          passed: true,
          ignored: false,
          duration,
          steps: ctx.steps,
        });
        totalPassed++;
      } catch (error) {
        const duration = performance.now() - start;
        const err = error instanceof Error ? error : new Error(String(error));

        console.log(`  ${test.name} ... ${red("FAILED")} ${dim(`(${formatDuration(duration)})`)}`);
        if (ctx.steps.length > 0) {
          for (const step of ctx.steps) {
            const status = step.passed ? green("ok") : red("FAILED");
            console.log(`    ${step.name} ... ${status} ${dim(`(${formatDuration(step.duration)})`)}`);
          }
        }

        fileResults.push({
          name: test.name,
          passed: false,
          ignored: false,
          duration,
          error: err,
          steps: ctx.steps,
        });
        totalFailed++;
      }
    }

    allResults.push({ file, results: fileResults });
  }

  // Summary
  console.log("");

  // Print failure details
  const failures = allResults.flatMap((f) =>
    f.results.filter((r) => !r.passed && !r.ignored).map((r) => ({
      file: f.file,
      ...r,
    }))
  );

  if (failures.length > 0) {
    console.log(red("FAILURES\n"));
    for (const f of failures) {
      console.log(`  ${f.file} > ${f.name}`);
      if (f.error) {
        console.log(`    ${f.error.message}`);
        if (f.error.stack) {
          const stackLines = f.error.stack.split("\n").slice(1, 4);
          for (const line of stackLines) {
            console.log(`    ${dim(line.trim())}`);
          }
        }
      }
      console.log("");
    }
  }

  const status = totalFailed > 0 ? red("FAILED") : green("ok");
  console.log(
    `test result: ${status}. ${totalPassed} passed; ${totalFailed} failed; ${totalIgnored} ignored ${dim(`(${formatDuration(performance.now())})`)}`,
  );

  if (totalFailed > 0) {
    throw new Error(`${totalFailed} test(s) failed`);
  }
}

// Run immediately on module load, then exit
await runTests();

// Minimal server so `start` command doesn't hang waiting forever.
// We still throw above on failure, which gives non-zero exit.
Deno.serve({ port: 9000 }, () => new Response("tests completed"));
```

## Limitations

| Limitation                             | Impact | Workaround                                  |
|----------------------------------------|--------|----------------------------------------------|
| No `--filter` flag (uses env var)      | Low    | `TEST_FILTER` env var                        |
| No parallel test execution             | Low    | Sequential is safer for shared state         |
| No resource/op sanitizers              | Medium | These are Deno internals, not portable       |
| Requires `start` command + port        | Low    | Minimal server at end; could be avoided      |
| No `--watch` mode                      | Low    | Use external file watcher                    |
| Test files must use dynamic import     | Low    | Standard ESM, no behavioral difference       |
| Runs as main worker (not user worker)  | Medium | Different sandbox than deployed functions    |

## Trade-offs

### Advantages
- **Zero Rust changes** - Works with any existing edge-runtime Docker image
- **Immediate availability** - Can be used today
- **Real runtime environment** - Tests execute in the actual edge runtime with its
  custom APIs, patched `process.env`, `SupaEnv`, etc.
- **Portable** - Just a TypeScript file, easy to version and share
- **Familiar API** - Uses the same `Deno.test()` signature developers already know

### Disadvantages
- **Not a first-class CLI command** - Invoked via `start` command, not `test`
- **Main worker context only** - Tests run as a main worker; deployed functions run
  as user workers with more restrictions (blocklisted FS APIs, etc.)
- **No integration with IDE test runners** - VS Code Deno extension won't discover
  these tests
- **No TAP/JUnit output** - Custom reporter only (could be extended)
