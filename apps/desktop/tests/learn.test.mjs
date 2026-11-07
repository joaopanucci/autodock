import test from "node:test";
import assert from "node:assert/strict";
import { LESSONS, TRACKS } from "../src/learn/lessons.ts";
import { GLOSSARY, REFERENCE, REF_GROUPS } from "../src/learn/reference.ts";

const bilingual = (text, where) => {
  assert.ok(text && text.en.trim().length > 0, `${where}: empty English text`);
  assert.ok(text && text.pt.trim().length > 0, `${where}: empty Portuguese text`);
};

const blockTexts = (block) => {
  switch (block.kind) {
    case "code":
      return block.caption ? [block.caption] : [];
    case "shell":
      return block.caption ? [block.caption] : [];
    case "steps":
    case "list":
      return block.items;
    default:
      return [block.text];
  }
};

test("lessons have unique ids and bilingual, non-empty texts", () => {
  assert.equal(new Set(LESSONS.map((l) => l.id)).size, LESSONS.length);
  for (const lesson of LESSONS) {
    bilingual(lesson.title, `${lesson.id}.title`);
    bilingual(lesson.summary, `${lesson.id}.summary`);
    if (lesson.tryIn) bilingual(lesson.tryIn.label, `${lesson.id}.tryIn`);
    lesson.docs.forEach((d, i) => bilingual(d.label, `${lesson.id}.docs[${i}]`));
    lesson.blocks.flatMap(blockTexts).forEach((t, i) => bilingual(t, `${lesson.id}.blocks[${i}]`));
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

test("every lesson belongs to a known track and tracks are in order", () => {
  const ids = new Set(TRACKS.map((t) => t.id));
  for (const track of TRACKS) {
    bilingual(track.title, `track ${track.id}.title`);
    bilingual(track.description, `track ${track.id}.description`);
    assert.ok(LESSONS.some((l) => l.track === track.id), `track ${track.id} has no lessons`);
  }
  for (const lesson of LESSONS) assert.ok(ids.has(lesson.track), `${lesson.id}: unknown track`);
  // Previous/next navigation follows the array, so tracks must be contiguous.
  const order = LESSONS.map((l) => l.track);
  const seen = [];
  for (const track of order) if (seen[seen.length - 1] !== track) seen.push(track);
  assert.equal(new Set(seen).size, seen.length, "lessons of one track are split apart");
});

test("every lesson has a valid self-check quiz", () => {
  for (const lesson of LESSONS) {
    assert.ok(lesson.quiz.length >= 2, `${lesson.id}: needs at least two questions`);
    lesson.quiz.forEach((q, i) => {
      const where = `${lesson.id}.quiz[${i}]`;
      bilingual(q.question, `${where}.question`);
      bilingual(q.why, `${where}.why`);
      assert.ok(q.options.length >= 2, `${where}: needs at least two options`);
      q.options.forEach((o, j) => bilingual(o, `${where}.options[${j}]`));
      assert.ok(Number.isInteger(q.answer) && q.answer >= 0 && q.answer < q.options.length, `${where}: answer out of range`);
    });
  }
});

test("shell blocks always provide bash and a real variant when they have tabs", () => {
  for (const lesson of LESSONS) {
    for (const block of lesson.blocks.filter((b) => b.kind === "shell")) {
      assert.ok(block.bash.trim(), `${lesson.id}: shell block without bash text`);
      const differs = [block.powershell, block.cmd].some((v) => v !== undefined && v !== block.bash);
      assert.ok(differs, `${lesson.id}: shell block whose variants are all identical (use a plain code block)`);
    }
  }
});

test("quick reference commands are unique, grouped and bilingual", () => {
  const groups = new Set(REF_GROUPS.map((g) => g.id));
  REF_GROUPS.forEach((g) => bilingual(g.title, `group ${g.id}`));
  assert.equal(new Set(REFERENCE.map((c) => c.id)).size, REFERENCE.length);
  for (const command of REFERENCE) {
    assert.ok(groups.has(command.group), `${command.id}: unknown group`);
    assert.match(command.bash, /^docker /, `${command.id}: not a docker command`);
    bilingual(command.description, `reference ${command.id}`);
  }
  for (const group of groups) assert.ok(REFERENCE.some((c) => c.group === group), `empty group ${group}`);
});

test("glossary terms are unique and bilingual", () => {
  assert.equal(new Set(GLOSSARY.map((g) => g.id)).size, GLOSSARY.length);
  for (const entry of GLOSSARY) {
    bilingual(entry.term, `glossary ${entry.id}.term`);
    bilingual(entry.definition, `glossary ${entry.id}.definition`);
  }
});

test("Windows is covered: the lesson exists and bind mounts have PowerShell and CMD forms", () => {
  assert.ok(LESSONS.some((l) => l.id === "docker-windows"));
  const bind = REFERENCE.find((c) => c.id === "bind");
  assert.ok(bind.powershell && bind.cmd, "bind mount reference needs PowerShell and CMD spellings");
  assert.match(bind.powershell, /\$\{PWD\}/);
  assert.match(bind.cmd, /%cd%/);
});
