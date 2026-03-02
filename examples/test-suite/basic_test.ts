// Basic test suite exercising all Deno.test() signatures

// Signature 1: Deno.test(name, fn)
Deno.test("basic assertion passes", () => {
  const x = 1 + 1;
  if (x !== 2) throw new Error(`Expected 2, got ${x}`);
});

// Signature 2: Deno.test(fn) — uses fn.name
Deno.test(function additionWorks() {
  const result = 2 + 3;
  if (result !== 5) throw new Error(`Expected 5, got ${result}`);
});

// Signature 3: Deno.test(options)
Deno.test({
  name: "string includes check",
  fn() {
    const str = "hello world";
    if (!str.includes("world")) {
      throw new Error("Expected string to include 'world'");
    }
  },
});

// Signature 4: Deno.test(name, options, fn)
Deno.test("array length check", {}, (t) => {
  const arr = [1, 2, 3];
  if (arr.length !== 3) throw new Error(`Expected length 3, got ${arr.length}`);
});

// Signature 5: Deno.test(options, fn)
Deno.test({ name: "object keys" }, () => {
  const obj = { a: 1, b: 2 };
  const keys = Object.keys(obj);
  if (keys.length !== 2) throw new Error(`Expected 2 keys, got ${keys.length}`);
});

// Ignored test
Deno.test({
  name: "this test is ignored",
  ignore: true,
  fn() {
    throw new Error("should not run");
  },
});
