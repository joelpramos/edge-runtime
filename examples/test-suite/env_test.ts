// Test that edge runtime APIs are available

Deno.test("process.env is accessible", () => {
  // process.env should be available
  if (typeof process === "undefined") {
    throw new Error("process is not defined");
  }
  if (typeof process.env !== "object") {
    throw new Error("process.env is not an object");
  }
});

Deno.test("Deno.env is accessible", () => {
  if (typeof Deno.env !== "object") {
    throw new Error("Deno.env is not available");
  }
});

Deno.test("EdgeRuntime namespace exists", () => {
  if (typeof (globalThis as any).EdgeRuntime === "undefined") {
    throw new Error("EdgeRuntime is not defined");
  }
});

Deno.test("fetch is available", () => {
  if (typeof fetch !== "function") {
    throw new Error("fetch is not a function");
  }
});
