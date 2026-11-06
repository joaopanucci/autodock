import assert from "node:assert/strict";
import test from "node:test";
import { parseConsoleContainers } from "../src/console-format.ts";

test("rejeita identidades vazias, duplicadas e campos com tipos inválidos", () => {
  for (const value of [
    { ID: "", Names: "x" },
    { ID: "x", Names: " " },
    { ID: "x", Names: "a", Ports: 42 },
  ])
    assert.equal(parseConsoleContainers(JSON.stringify(value)), null);
  const row = JSON.stringify({ ID: "x", Names: "a" });
  assert.equal(parseConsoleContainers(row + "\n" + row), null);
});

test("interpreta containers JSON por linha", () => {
  const output = [
    JSON.stringify({
      ID: "abc123",
      Names: "app-web-1",
      Image: "nginx:latest",
      State: "running",
      Status: "Up 2 hours (healthy)",
      Ports: "127.0.0.1:8080->80/tcp",
    }),
  ].join("\n");

  const rows = parseConsoleContainers(output);

  assert.equal(rows?.length, 1);
  assert.equal(rows?.[0].name, "app-web-1");
  assert.equal(rows?.[0].state, "running");
  assert.equal(rows?.[0].ports, "127.0.0.1:8080->80/tcp");
});

test("aceita lista vazia", () => {
  assert.deepEqual(parseConsoleContainers(""), []);
});

test("rejeita JSON invalido", () => {
  assert.equal(parseConsoleContainers("{invalid"), null);
});

test("rejeita registros sem identidade", () => {
  assert.equal(parseConsoleContainers('{"Image":"nginx"}'), null);
});

test("rejeita JSON que nao representa objeto", () => {
  assert.equal(parseConsoleContainers('"texto"'), null);
});

test("rejeita saida parcialmente truncada", () => {
  const valid = JSON.stringify({
    ID: "abc",
    Names: "web",
  });

  assert.equal(parseConsoleContainers(`${valid}\n{"ID":`), null);
});
