import { test } from "node:test";
import assert from "node:assert/strict";
import {
  compatibility,
  displayPath,
  groupModules,
  rememberProject,
  recommendation,
  checkSummary,
} from "../src/model.ts";

const scan = (findings = []) => ({
  schemaVersion: 1,
  project: "C:\\work\\app",
  components: [],
  warnings: [],
  findings,
});
const runtime = (value) => ({
  module: ".",
  kind: "runtime",
  name: "node",
  value,
  source: "package.json → engines.node",
  status: "declared",
});
const check = (status = "available", stdout = "v22.0.0") => ({
  name: "node",
  status,
  stdout,
  stderr: "",
  detail: "",
});
const doctor = (checks) => ({
  schemaVersion: 1,
  os: "windows",
  checks,
  notes: [],
});

test("groups technologies by module without conflating Maven and Spring as two modules", () => {
  const result = groupModules({
    ...scan(),
    components: [
      { path: "api", technology: "maven", evidence: ["api/pom.xml"] },
      { path: "api", technology: "spring-boot", evidence: ["api/pom.xml"] },
    ],
  });
  assert.equal(result.length, 1);
  assert.deepEqual(result[0].technologies, ["maven", "spring-boot"]);
  assert.equal(result[0].evidence.length, 1);
});
test("supports older reports without findings and Docker-only modules", () => {
  assert.deepEqual(
    groupModules({ project: "x", components: [], warnings: [] }),
    [],
  );
  assert.equal(
    groupModules(scan([{ module: "infra", kind: "container_config" }]))[0].path,
    "infra",
  );
});
test("normalizes Windows extended paths and preserves UNC", () => {
  assert.equal(displayPath("\\\\?\\C:\\work\\app"), "C:\\work\\app");
  assert.equal(displayPath("\\\\?\\UNC\\server\\share"), "\\\\server\\share");
});
test("rescanning replaces a Windows project, but macOS case differences remain distinct", () => {
  const first = { scan: scan(), analyzedAt: "old" };
  const second = {
    scan: { ...scan(), project: "\\\\?\\c:\\WORK\\app" },
    analyzedAt: "new",
  };
  assert.equal(rememberProject([first], second).length, 1);
  assert.equal(rememberProject([first], second)[0].analyzedAt, "new");
  assert.equal(
    rememberProject(
      [{ ...first, scan: { ...scan(), project: "/work/App" } }],
      { ...first, scan: { ...scan(), project: "/work/app" } },
      "macos",
    ).length,
    2,
  );
});
test("confirms only exact Node mismatch backed by an available check", () => {
  assert.equal(
    compatibility(scan([runtime("20.0.0")]), doctor([check()]))[0].status,
    "incompatible",
  );
  assert.equal(
    compatibility(scan([runtime("22.0.0")]), doctor([check()]))[0].status,
    "matches",
  );
});
test("ranges, prereleases, references and malformed versions remain unverified", () => {
  for (const value of [
    ">=20",
    "^22.0.0",
    "22",
    "${node.version}",
    "22.0.0-beta",
    "022.0.0",
    "*",
  ]) {
    assert.equal(
      compatibility(scan([runtime(value)]), doctor([check()]))[0].status,
      "unverified",
    );
  }
});
test("missing, timeout, execution failure, malformed output and no diagnostic do not prove incompatibility", () => {
  for (const status of ["missing", "timeout", "execution_error"])
    assert.equal(
      compatibility(scan([runtime("20.0.0")]), doctor([check(status)]))[0]
        .status,
      "unverified",
    );
  assert.equal(
    compatibility(
      scan([runtime("20.0.0")]),
      doctor([check("available", "unknown")]),
    )[0].status,
    "unverified",
  );
  assert.equal(
    compatibility(scan([runtime("20.0.0")]), null)[0].status,
    "unverified",
  );
});
test("Java target declarations are not compared against unrelated installed tools", () => {
  assert.equal(
    compatibility(
      scan([{ ...runtime("17"), name: "maven.compiler.target" }]),
      doctor([check()]),
    )[0].status,
    "unverified",
  );
});
test("recommendations distinguish uncertainty and never declare Docker stopped from an error", () => {
  assert.match(recommendation(check("timeout")), /não confirma ausência/);
  assert.match(
    recommendation({ ...check("execution_error"), name: "docker-engine" }),
    /causa não foi confirmada/,
  );
  assert.match(recommendation(check("missing")), /não confirma/);
});
test("a missing version is shown explicitly without inventing a version", () => {
  assert.equal(checkSummary(check("available", "")), "Sem versão informada");
});
