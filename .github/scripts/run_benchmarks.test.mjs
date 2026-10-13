import assert from "node:assert/strict";
import test from "node:test";

import { makeReport } from "./check_benchmarks.mjs";
import { confirmResults, measurePair } from "./run_benchmarks.mjs";

function result(estimate, lowerBound, upperBound) {
  return { name: "scanning/example/str", estimate, lowerBound, upperBound };
}

test("a slowdown must persist in both measurement orders", () => {
  const first = [result(0.12, 0.1, 0.14)];
  const confirmation = [result(0.01, -0.01, 0.03)];
  const results = confirmResults(first, confirmation);
  assert.equal(makeReport(results, 0.05).regressions.length, 0);
});

test("confirmed changes cover the uncertainty from both measurements", () => {
  const results = confirmResults(
    [result(0.12, 0.1, 0.14)],
    [result(0.08, 0.06, 0.1)],
  );
  assert.deepEqual(results, [result(0.1, 0.06, 0.14)]);
  assert.equal(makeReport(results, 0.05).regressions.length, 1);
  assert.match(makeReport(results, 0.05, true).report, /both measurement orders/);
});

test("a change that reverses direction is omitted", () => {
  const results = confirmResults(
    [result(-0.12, -0.14, -0.1)],
    [result(0.12, 0.1, 0.14)],
  );
  assert.equal(makeReport(results, 0.05).changes.length, 0);
});

test("an improvement must persist across both complete intervals", () => {
  const first = [result(-0.12, -0.14, -0.1)];
  assert.equal(
    makeReport(confirmResults(first, [result(-0.08, -0.1, -0.06)]), 0.05).changes.length,
    1,
  );
  assert.equal(
    makeReport(confirmResults(first, [result(-0.06, -0.08, -0.04)]), 0.05).changes.length,
    0,
  );
});

test("missing confirmation cannot silently pass the gate", () => {
  assert.throws(() => confirmResults([result(0.12, 0.1, 0.14)], []), /missing confirmation/);
  assert.throws(
    () => confirmResults([result(0.12, 0.1, 0.14)], [result(null, null, null)]),
    /missing confirmation/,
  );
});

test("new measurements remain unclassified", () => {
  const first = [result(null, null, null)];
  assert.deepEqual(confirmResults(first, []), first);
});

test("confirmation measures the candidate first and reuses its saved samples", () => {
  const calls = [];
  const run = (...args) => calls.push(args);
  measurePair("base", "candidate", "first", false, run);
  measurePair("base", "candidate", "second", true, run);
  assert.deepEqual(calls, [
    ["base", "first", ["--save-baseline", "main"]],
    ["candidate", "first", ["--baseline", "main"]],
    ["candidate", "second", ["--save-baseline", "candidate"]],
    ["base", "second", ["--save-baseline", "main"]],
    ["candidate", "second", ["--load-baseline", "candidate", "--baseline", "main"]],
  ]);
});
