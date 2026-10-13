#!/usr/bin/env node

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";

import { loadResults, makeReport } from "./check_benchmarks.mjs";

export function confirmResults(first, second) {
  const confirmations = new Map(second.map((result) => [result.name, result]));
  return first.map((result) => {
    const confirmation = confirmations.get(result.name);
    if (result.estimate === null) {
      return result;
    }
    if (!confirmation || confirmation.estimate === null) {
      throw new Error(`missing confirmation for ${result.name}`);
    }
    return {
      name: result.name,
      estimate: (result.estimate + confirmation.estimate) / 2,
      lowerBound: Math.min(result.lowerBound, confirmation.lowerBound),
      upperBound: Math.max(result.upperBound, confirmation.upperBound),
    };
  });
}

function executables(path) {
  return new Map(
    readFileSync(path, "utf8")
      .trim()
      .split(/\r?\n/)
      .filter(Boolean)
      .map((line) => line.split("\t")),
  );
}

export function measurePair(base, candidate, directory, reverse, run) {
  if (reverse) {
    run(candidate, directory, ["--save-baseline", "candidate"]);
    run(base, directory, ["--save-baseline", "main"]);
    run(candidate, directory, ["--load-baseline", "candidate", "--baseline", "main"]);
  } else {
    run(base, directory, ["--save-baseline", "main"]);
    run(candidate, directory, ["--baseline", "main"]);
  }
}

function run(executable, directory, args) {
  mkdirSync(directory, { recursive: true });
  const result = spawnSync(executable, ["--bench", ...args], {
    env: { ...process.env, CARGO_TARGET_DIR: directory },
    stdio: "inherit",
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(`${executable} failed with status ${result.status}`);
  }
}

function main() {
  const [basePath, candidatePath, affectedPath, outputPath] = process.argv.slice(2);
  const base = executables(basePath);
  const candidate = executables(candidatePath);
  const affected = readFileSync(affectedPath, "utf8").trim().split(/\r?\n/).filter(Boolean);
  const output = resolve(outputPath);
  mkdirSync(output, { recursive: true });
  const results = [];
  for (const name of affected) {
    if (!base.has(name) || !candidate.has(name)) {
      throw new Error(`missing benchmark executable for ${name}`);
    }
    const firstDirectory = resolve(output, name, "first");
    measurePair(base.get(name), candidate.get(name), firstDirectory, false, run);
    const first = loadResults(resolve(firstDirectory, "criterion"));
    if (!first.some((result) => result.estimate !== null)) {
      throw new Error(`no benchmark comparison for ${name}`);
    }
    if (makeReport(first, 0.05).changes.length > 0) {
      console.log(`Confirm ${name} with the opposite measurement order.`);
      const secondDirectory = resolve(output, name, "confirmation");
      measurePair(base.get(name), candidate.get(name), secondDirectory, true, run);
      const second = loadResults(resolve(secondDirectory, "criterion"));
      results.push(...confirmResults(first, second));
    } else {
      results.push(...first);
    }
  }
  writeFileSync(resolve(output, "results.json"), JSON.stringify(results, null, 2), "utf8");
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main();
}
