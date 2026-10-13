#!/usr/bin/env node
// Choose and apply the next workspace version from conventional commits.

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const MANIFEST = resolve(ROOT, "Cargo.toml");
const DEPENDENCY_MANIFESTS = [
  [resolve(ROOT, "runeweaver/Cargo.toml"), "runeweaver-derive"],
  [resolve(ROOT, "runeweaver-derive/Cargo.toml"), "runeweaver-codegen"],
];
const VERSION_RE = /^version = "(\d+)\.(\d+)\.(\d+)"$/m;
const TAG_RE = /^v(\d+)\.(\d+)\.(\d+)$/;
const HEADER_RE = /^(?<kind>[A-Za-z][A-Za-z0-9_-]*)(?:\([^\r\n)]+\))?(?<breaking>!)?:\s+/;
const BREAKING_FOOTER_RE = /^BREAKING(?:[ -]CHANGE):\s*\S/im;
const RANK = { none: 0, patch: 1, minor: 2, major: 3 };

export function parseVersion(value) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(value);
  if (match === null) {
    throw new Error(`invalid release version: ${value}`);
  }
  return match.slice(1).map(Number);
}

export function formatVersion(version) {
  return version.join(".");
}

export function bumpVersion(version, change) {
  const [major, minor, patch] = version;
  if (change === "major") return [major + 1, 0, 0];
  if (change === "minor") return [major, minor + 1, 0];
  if (change === "patch") return [major, minor, patch + 1];
  throw new Error(`cannot bump version for change type: ${change}`);
}

function compareVersions(left, right) {
  for (let index = 0; index < 3; index += 1) {
    if (left[index] !== right[index]) return left[index] - right[index];
  }
  return 0;
}

function versionsEqual(left, right) {
  return compareVersions(left, right) === 0;
}

export function selectNextVersion(previousVersion, change, override = "") {
  if (override === "") return bumpVersion(previousVersion, change);

  const version = parseVersion(override);
  if (compareVersions(version, previousVersion) <= 0) {
    throw new Error(
      `override version ${override} must be newer than ${formatVersion(previousVersion)}`,
    );
  }
  return version;
}

function git(...arguments_) {
  return execFileSync("git", arguments_, { cwd: ROOT, encoding: "utf8" }).trim();
}

function currentVersion() {
  const match = VERSION_RE.exec(readFileSync(MANIFEST, "utf8"));
  if (match === null) {
    throw new Error("workspace.package must contain a plain semantic version");
  }
  return match.slice(1).map(Number);
}

function latestReleaseTag() {
  const releases = git("tag", "--merged", "HEAD", "--list", "v*")
    .split("\n")
    .filter(Boolean)
    .flatMap((tag) => {
      const match = TAG_RE.exec(tag);
      return match === null ? [] : [{ tag, version: match.slice(1).map(Number) }];
    });
  releases.sort((left, right) => compareVersions(left.version, right.version));
  return releases.at(-1) ?? null;
}

export function classifyCommit(subject, body) {
  const header = HEADER_RE.exec(subject);
  if (header?.groups.breaking || BREAKING_FOOTER_RE.test(body)) return "major";
  const kind = header?.groups.kind.toLowerCase();
  if (kind === "feat") return "minor";
  if (kind === "fix") return "patch";
  return "none";
}

export function selectLargestChange(commits) {
  let change = "none";
  for (const { subject, body } of commits) {
    const candidate = classifyCommit(subject, body);
    if (RANK[candidate] > RANK[change]) change = candidate;
  }
  return change;
}

function largestChange(revisionRange) {
  const hashes = git("rev-list", "--reverse", revisionRange).split("\n").filter(Boolean);
  const commits = hashes.map((commit) => ({
    subject: git("show", "-s", "--format=%s", commit),
    body: git("show", "-s", "--format=%b", commit),
  }));
  return selectLargestChange(commits);
}

function replaceOnce(path, pattern, replacement) {
  const original = readFileSync(path, "utf8");
  const matches = original.match(pattern);
  if (matches === null) {
    throw new Error(`expected exactly one version entry in ${path}`);
  }
  const updated = original.replace(pattern, replacement);
  if (updated === original) {
    throw new Error(`version entry did not change in ${path}`);
  }
  writeFileSync(path, updated);
}

function applyVersion(version) {
  const formatted = formatVersion(version);
  const current = formatVersion(currentVersion());
  if (formatted !== current) {
    replaceOnce(MANIFEST, VERSION_RE, `version = "${formatted}"`);
  }
  for (const [manifest, packageName] of DEPENDENCY_MANIFESTS) {
    const dependencyPattern = new RegExp(
      `^${packageName} = \\{ version = "=[^"]+", path = "([^"]+)"(, (?:optional = true|default-features = false))? \\}$`,
      "m",
    );
    const contents = readFileSync(manifest, "utf8");
    const expected = `${packageName} = { version = "=${formatted}",`;
    if (!contents.includes(expected)) {
      replaceOnce(
        manifest,
        dependencyPattern,
        `${packageName} = { version = "=${formatted}", path = "$1"$2 }`,
      );
    }
  }
}

function determineRelease(override = "") {
  const manifestVersion = currentVersion();
  const previous = latestReleaseTag();
  if (previous === null) {
    return {
      version: override === "" ? manifestVersion : parseVersion(override),
      change: "initial",
      previousTag: "",
      mode: "new",
    };
  }

  const taggedCommit = git("rev-list", "-n", "1", previous.tag);
  const headCommit = git("rev-parse", "HEAD");
  if (taggedCommit === headCommit) {
    if (override !== "" && !versionsEqual(parseVersion(override), previous.version)) {
      throw new Error(`cannot override the version when resuming release ${previous.tag}`);
    }
    if (!versionsEqual(manifestVersion, previous.version)) {
      throw new Error(
        `${previous.tag} points at HEAD but the manifest is ${formatVersion(manifestVersion)}`,
      );
    }
    return {
      version: manifestVersion,
      change: "resume",
      previousTag: previous.tag,
      mode: "resume",
    };
  }

  if (!versionsEqual(manifestVersion, previous.version)) {
    throw new Error(
      `manifest version ${formatVersion(manifestVersion)} does not match latest tag ${previous.tag}`,
    );
  }
  const change = largestChange(`${previous.tag}..HEAD`);
  if (change === "none") {
    throw new Error(
      `no feat, fix, or breaking conventional commit exists after ${previous.tag}`,
    );
  }
  return {
    version: selectNextVersion(previous.version, change, override),
    change,
    previousTag: previous.tag,
    mode: "new",
  };
}

function main() {
  const checkOnly = process.argv.slice(2).includes("--check");
  const release = determineRelease(process.env.NEXT_VERSION ?? "");
  if (!checkOnly && release.mode === "new") applyVersion(release.version);

  const version = formatVersion(release.version);
  console.log(`version=${version}`);
  console.log(`tag=v${version}`);
  console.log(`change=${release.change}`);
  console.log(`previous_tag=${release.previousTag}`);
  console.log(`mode=${release.mode}`);
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    main();
  } catch (error) {
    console.error(`release preparation failed: ${error.message}`);
    process.exitCode = 1;
  }
}
