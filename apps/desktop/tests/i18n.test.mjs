import test from "node:test";
import assert from "node:assert/strict";
import { en } from "../src/i18n/en.ts";
import { pt } from "../src/i18n/pt.ts";
import { detectLanguage, translate, translatePlural } from "../src/i18n/core.ts";

const placeholders = (text) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();

test("both languages define exactly the same keys", () => {
  assert.deepEqual(Object.keys(en).sort(), Object.keys(pt).sort());
});

test("no value is empty and placeholders match across languages", () => {
  for (const key of Object.keys(pt)) {
    assert.ok(pt[key].trim(), `empty pt: ${key}`);
    assert.ok(en[key].trim(), `empty en: ${key}`);
    assert.deepEqual(placeholders(pt[key]), placeholders(en[key]), `placeholders differ: ${key}`);
  }
});

test("plural keys come in .one/.other pairs", () => {
  const keys = Object.keys(pt);
  for (const key of keys.filter((k) => k.endsWith(".one"))) {
    assert.ok(keys.includes(key.replace(/\.one$/, ".other")), `missing .other for ${key}`);
  }
  for (const key of keys.filter((k) => k.endsWith(".other"))) {
    assert.ok(keys.includes(key.replace(/\.other$/, ".one")), `missing .one for ${key}`);
  }
});

test("the English dictionary has no leftover Portuguese diacritics (proper nouns aside)", () => {
  const allowed = new Set(["settings.language.label"]);
  for (const [key, value] of Object.entries(en)) {
    if (allowed.has(key)) continue;
    assert.ok(!/[áéíóúâêôãõç]/i.test(value), `Portuguese text in en: ${key} -> ${value}`);
  }
});

test("translate interpolates, falls back to the key name and handles plurals", () => {
  assert.equal(translate("en", "logs.lines", { n: 100 }), "100 lines");
  assert.equal(translate("pt", "logs.lines", { n: 100 }), "100 linhas");
  assert.equal(translatePlural("en", "containers.count", 1), "1 container");
  assert.equal(translatePlural("en", "containers.count", 3), "3 containers");
  assert.equal(translate("en", "logs.exitCode", {}), "Exit code: {code}");
});

test("language detection prefers the saved choice, then the system language", () => {
  const original = { window: globalThis.window, navigator: globalThis.navigator };
  const set = (stored, language) => {
    globalThis.window = { localStorage: { getItem: () => stored } };
    Object.defineProperty(globalThis, "navigator", { value: { language }, configurable: true });
  };
  try {
    set(null, "pt-BR");
    assert.equal(detectLanguage(), "pt");
    set(null, "en-US");
    assert.equal(detectLanguage(), "en");
    set(null, "de-DE");
    assert.equal(detectLanguage(), "en");
    set("pt", "en-US");
    assert.equal(detectLanguage(), "pt");
    set("xx", "en-US");
    assert.equal(detectLanguage(), "en");
    globalThis.window = { localStorage: { getItem() { throw new Error("blocked"); } } };
    Object.defineProperty(globalThis, "navigator", { value: { language: "pt-PT" }, configurable: true });
    assert.equal(detectLanguage(), "pt");
  } finally {
    globalThis.window = original.window;
    Object.defineProperty(globalThis, "navigator", { value: original.navigator, configurable: true });
  }
});
