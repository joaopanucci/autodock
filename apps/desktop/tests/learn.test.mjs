import test from "node:test";
import assert from "node:assert/strict";
import { LESSONS } from "../src/learn/lessons.ts";

const texts = (lesson) => [
  lesson.title,
  lesson.summary,
  ...(lesson.tryIn ? [lesson.tryIn.label] : []),
  ...lesson.docs.map((d) => d.label),
  ...lesson.blocks.flatMap((b) => (b.kind === "code" ? (b.caption ? [b.caption] : []) : [b.text])),
];

test("lessons have unique ids and bilingual, non-empty texts", () => {
  assert.equal(new Set(LESSONS.map((l) => l.id)).size, LESSONS.length);
  for (const lesson of LESSONS) {
    for (const text of texts(lesson)) {
      assert.ok(text.en.trim().length > 0, `${lesson.id}: empty English text`);
      assert.ok(text.pt.trim().length > 0, `${lesson.id}: empty Portuguese text`);
    }
  }
});

test("lessons only link to official Docker documentation over https", () => {
  for (const lesson of LESSONS) {
    assert.ok(lesson.docs.length > 0, `${lesson.id}: no documentation link`);
    for (const doc of lesson.docs) {
      assert.match(doc.url, /^https:\/\/docs\.docker\.com\//);
    }
  }
});
