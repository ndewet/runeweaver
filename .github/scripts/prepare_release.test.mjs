import assert from "node:assert/strict";
import test from "node:test";

import {
  bumpVersion,
  classifyCommit,
  selectLargestChange,
  selectNextVersion,
} from "./prepare_release.mjs";

test("version bumps reset less significant parts", () => {
  assert.deepEqual(bumpVersion([1, 1, 1], "patch"), [1, 1, 2]);
  assert.deepEqual(bumpVersion([1, 1, 1], "minor"), [1, 2, 0]);
  assert.deepEqual(bumpVersion([1, 1, 1], "major"), [2, 0, 0]);
});

test("the next version can be overridden", () => {
  assert.deepEqual(selectNextVersion([1, 1, 1], "patch", "2.0.0"), [2, 0, 0]);
});

test("an override must be newer than the previous version", () => {
  assert.throws(
    () => selectNextVersion([1, 1, 1], "minor", "1.1.1"),
    /override version 1\.1\.1 must be newer than 1\.1\.1/,
  );
  assert.throws(
    () => selectNextVersion([1, 1, 1], "minor", "1.0.9"),
    /override version 1\.0\.9 must be newer than 1\.1\.1/,
  );
});

test("feat commits select a minor release", () => {
  assert.equal(classifyCommit("feat(parser): add modes", ""), "minor");
});

test("fix commits select a patch release", () => {
  assert.equal(classifyCommit("fix: reject empty rules", ""), "patch");
});

test("a bang selects a major release for any conventional type", () => {
  assert.equal(classifyCommit("refactor(core)!: new API", ""), "major");
});

test("a breaking change footer selects a major release", () => {
  const body = "Some explanation.\n\nBREAKING CHANGE: scanners now own their source";
  assert.equal(classifyCommit("feat: stream input", body), "major");
});

test("unrelated commits have no release effect", () => {
  assert.equal(classifyCommit("docs: clarify examples", ""), "none");
});

test("only the largest change across all commits is selected", () => {
  assert.equal(
    selectLargestChange([
      { subject: "fix: one", body: "" },
      { subject: "feat: two", body: "" },
      { subject: "fix: three", body: "" },
    ]),
    "minor",
  );
  assert.equal(
    selectLargestChange([
      { subject: "feat: one", body: "" },
      { subject: "fix!: break the API", body: "" },
    ]),
    "major",
  );
});
