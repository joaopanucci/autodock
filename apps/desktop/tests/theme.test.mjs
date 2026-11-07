import test from "node:test";
import assert from "node:assert/strict";
import { THEME_MODES, normalizeMode, resolveTheme } from "../src/theme.ts";

test("only light and dark are explicit choices; anything else follows the system", () => {
  assert.equal(normalizeMode("light"), "light");
  assert.equal(normalizeMode("dark"), "dark");
  for (const value of ["system", "", null, undefined, "blue", 1, {}]) {
    assert.equal(normalizeMode(value), "system");
  }
});

test("the system mode follows the operating system and explicit modes ignore it", () => {
  assert.equal(resolveTheme("system", true), "dark");
  assert.equal(resolveTheme("system", false), "light");
  assert.equal(resolveTheme("light", true), "light");
  assert.equal(resolveTheme("dark", false), "dark");
});

test("the switch offers exactly light, dark and system", () => {
  assert.deepEqual([...THEME_MODES].sort(), ["dark", "light", "system"]);
});
