// Test file with intentional failures for testing --fail-fast and error reporting

Deno.test("this test passes", () => {
  // OK
});

Deno.test("this test fails", () => {
  throw new Error("intentional failure for testing");
});

Deno.test("this test also passes", () => {
  // OK
});
