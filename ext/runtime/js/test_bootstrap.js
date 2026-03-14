// Test bootstrap for edge-runtime `test` subcommand.
// Loaded via execute_script() when test_mode is enabled.
// Provides Deno.test() shim, TestContext, and __test_runner.run().

((globalThis) => {
  "use strict";

  const tests = [];

  // --- Deno.test() shim ---
  // Supports all 5 call signatures from Deno:
  //   1. Deno.test(name, fn)
  //   2. Deno.test(fn)  (fn.name used as test name)
  //   3. Deno.test(options)
  //   4. Deno.test(name, options, fn)
  //   5. Deno.test(options, fn)

  const SANITIZER_KEYS = [
    "sanitizeOps",
    "sanitizeResources",
    "sanitizeExit",
    "permissions",
  ];

  function warnIgnoredOptions(opts) {
    for (const key of SANITIZER_KEYS) {
      if (opts[key] !== undefined) {
        console.warn(
          `[test] Warning: "${key}" option is not supported and will be ignored.`,
        );
      }
    }
  }

  function registerTest(name, fn, opts = {}) {
    warnIgnoredOptions(opts);
    tests.push({
      name,
      fn,
      ignore: opts.ignore ?? false,
      only: opts.only ?? false,
    });
  }

  function parseTestArgs(args, overrides = {}) {
    if (args.length === 0) {
      throw new TypeError("Deno.test requires at least one argument");
    }

    // Signature 1: Deno.test(name, fn)
    if (typeof args[0] === "string" && typeof args[1] === "function") {
      registerTest(args[0], args[1], { ...(args[2] || {}), ...overrides });
      return;
    }

    // Signature 2: Deno.test(fn)
    if (typeof args[0] === "function") {
      registerTest(args[0].name || "<anonymous>", args[0], overrides);
      return;
    }

    // Signature 3: Deno.test(options) — fn inside options
    if (typeof args[0] === "object" && args.length === 1) {
      const opts = args[0];
      if (typeof opts.fn !== "function") {
        throw new TypeError("Deno.test options must include a `fn` property");
      }
      registerTest(
        opts.name || opts.fn.name || "<anonymous>",
        opts.fn,
        { ...opts, ...overrides },
      );
      return;
    }

    // Signature 4: Deno.test(name, options, fn)
    if (
      typeof args[0] === "string" &&
      typeof args[1] === "object" &&
      typeof args[2] === "function"
    ) {
      registerTest(args[0], args[2], { ...args[1], ...overrides });
      return;
    }

    // Signature 5: Deno.test(options, fn)
    if (typeof args[0] === "object" && typeof args[1] === "function") {
      const opts = args[0];
      registerTest(
        opts.name || args[1].name || "<anonymous>",
        args[1],
        { ...opts, ...overrides },
      );
      return;
    }

    throw new TypeError("Invalid arguments to Deno.test()");
  }

  function denoTest(...args) {
    parseTestArgs(args);
  }

  // Deno.test.ignore() — shorthand for { ignore: true }
  denoTest.ignore = function (...args) {
    parseTestArgs(args, { ignore: true });
  };

  // Deno.test.only() — shorthand for { only: true }
  denoTest.only = function (...args) {
    parseTestArgs(args, { only: true });
  };

  // --- TestContext ---

  class TestContext {
    #name;
    #origin;
    #parent;
    #steps;

    constructor(name, origin, parent = null) {
      this.#name = name;
      this.#origin = origin;
      this.#parent = parent;
      this.#steps = [];
    }

    get name() {
      return this.#name;
    }
    get origin() {
      return this.#origin;
    }
    get parent() {
      return this.#parent;
    }

    async step(...args) {
      let name, fn;

      // step(name, fn)
      if (typeof args[0] === "string" && typeof args[1] === "function") {
        name = args[0];
        fn = args[1];
      }
      // step(fn)
      else if (typeof args[0] === "function") {
        name = args[0].name || "<anonymous step>";
        fn = args[0];
      }
      // step(options)
      else if (typeof args[0] === "object") {
        name = args[0].name || "<anonymous step>";
        fn = args[0].fn;
        if (typeof fn !== "function") {
          throw new TypeError("step options must include a `fn` property");
        }
      } else {
        throw new TypeError("Invalid arguments to TestContext.step()");
      }

      const childCtx = new TestContext(name, this.#origin, this);
      const stepResult = {
        name,
        status: "passed",
        duration: 0,
        error: null,
        steps: [],
      };

      const start = Date.now();
      try {
        await fn(childCtx);
        stepResult.steps = childCtx.#steps;
      } catch (err) {
        stepResult.status = "failed";
        stepResult.error = formatError(err);
        stepResult.steps = childCtx.#steps;
      }
      stepResult.duration = Date.now() - start;

      this.#steps.push(stepResult);

      if (stepResult.status === "failed") {
        throw new Error(
          `Step "${name}" failed: ${stepResult.error}`,
        );
      }

      return true;
    }

    _getSteps() {
      return this.#steps;
    }
  }

  // --- Error formatting ---

  function formatError(err) {
    if (err instanceof Error) {
      return err.stack || err.message || String(err);
    }
    return String(err);
  }

  // --- Filter matching ---

  function matchesFilter(testName, filterConfig) {
    if (!filterConfig) return true;

    if (filterConfig.regex) {
      try {
        const re = new RegExp(filterConfig.regex);
        return re.test(testName);
      } catch {
        // Invalid regex, fall through to substring
        return testName.includes(filterConfig.regex);
      }
    }

    if (filterConfig.substring) {
      return testName.includes(filterConfig.substring);
    }

    // Legacy: plain string filter (backward compat)
    if (typeof filterConfig === "string") {
      return testName.includes(filterConfig);
    }

    return true;
  }

  // --- Step result counting ---

  function countStepResults(steps) {
    let passed = 0, failed = 0, ignored = 0;
    for (const step of steps) {
      if (step.status === "passed") passed++;
      else if (step.status === "failed") failed++;
      else if (step.status === "ignored") ignored++;
      const sub = countStepResults(step.steps || []);
      passed += sub.passed;
      failed += sub.failed;
      ignored += sub.ignored;
    }
    return { passed, failed, ignored };
  }

  // --- Test runner ---

  async function runTests(filterConfig, failFast) {
    const results = [];
    const failures = [];
    let passed = 0;
    let failed = 0;
    let ignored = 0;
    let filteredOut = 0;
    let passedSteps = 0;
    let failedSteps = 0;
    let ignoredSteps = 0;

    // Check for `only` tests
    const hasOnly = tests.some((t) => t.only);

    for (const test of tests) {
      // Filter
      if (!matchesFilter(test.name, filterConfig)) {
        filteredOut++;
        continue;
      }

      // If any test has `only`, skip tests without it
      if (hasOnly && !test.only) {
        ignored++;
        results.push({
          name: test.name,
          status: "ignored",
          duration: 0,
          error: null,
          steps: [],
        });
        continue;
      }

      // Ignore
      if (test.ignore) {
        ignored++;
        results.push({
          name: test.name,
          status: "ignored",
          duration: 0,
          error: null,
          steps: [],
        });
        continue;
      }

      const ctx = new TestContext(test.name, "edge-runtime-test");
      const result = {
        name: test.name,
        status: "passed",
        duration: 0,
        error: null,
        steps: [],
      };

      const start = Date.now();
      try {
        await test.fn(ctx);
        result.steps = ctx._getSteps();
        passed++;
      } catch (err) {
        result.status = "failed";
        result.error = formatError(err);
        result.steps = ctx._getSteps();
        failed++;
        failures.push({ name: test.name, error: result.error });
      }
      result.duration = Date.now() - start;

      // Count step results
      const stepCounts = countStepResults(result.steps);
      passedSteps += stepCounts.passed;
      failedSteps += stepCounts.failed;
      ignoredSteps += stepCounts.ignored;

      results.push(result);

      if (failFast && result.status === "failed") {
        break;
      }
    }

    return JSON.stringify({
      total: results.length,
      passed,
      failed,
      ignored,
      filtered_out: filteredOut,
      passed_steps: passedSteps,
      failed_steps: failedSteps,
      ignored_steps: ignoredSteps,
      failures,
      results,
    });
  }

  // Install Deno.test
  if (typeof globalThis.Deno === "undefined") {
    globalThis.Deno = {};
  }
  globalThis.Deno.test = denoTest;

  // Install __test_runner
  globalThis.__test_runner = {
    run: runTests,
  };
})(globalThis);
