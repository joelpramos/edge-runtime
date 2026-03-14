// Test suite exercising TestContext and step() API

Deno.test("nested steps", async (t) => {
  await t.step("step 1: setup", () => {
    const ready = true;
    if (!ready) throw new Error("not ready");
  });

  await t.step("step 2: verify", async (t) => {
    await t.step("sub-step 2a", () => {
      if (1 + 1 !== 2) throw new Error("math broken");
    });

    await t.step("sub-step 2b", () => {
      if ("hello".length !== 5) throw new Error("string broken");
    });
  });
});

Deno.test("step with function name", async (t) => {
  await t.step(function namedStep() {
    // This step gets its name from the function
    const x = 42;
    if (x !== 42) throw new Error("wrong value");
  });
});
