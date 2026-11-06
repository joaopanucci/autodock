import test from "node:test";
import assert from "node:assert/strict";
import { logText, logsCopy } from "../src/logs-format.ts";
import { translate } from "../src/i18n/core.ts";

const t = (key, params) => translate("pt", key, params);

test("logs remain plain text, with ANSI and terminal controls removed", () => {
  assert.equal(logText("\x1b[31merror\x1b[0m\n\tline\x00"), "error\n\tline");
  assert.equal(logText("\x1b]0;title\x07text\u202e"), "text");
  assert.equal(
    logText("<script>alert(1)</script>"),
    "<script>alert(1)</script>",
  );
});
test("copy identifies streams, truncation and failure, without omitting empty stdout", () => {
  const report = {
    containerId: "abcdef012345",
    tail: 100,
    status: "timeout",
    stdout: "",
    stderr: "test failure",
    truncated: true,
    detail: "deadline",
  };
  const result = logsCopy(report, t);
  assert.match(result, /--tail 100 -- abcdef012345/);
  assert.match(result, /\[stdout\]\n\(sem saída padrão\)/);
  assert.match(result, /\[stderr\]\ntest failure/);
  assert.match(result, /timeout \(saída truncada\)/);
});
test("the copied text follows the selected language", () => {
  const report = { containerId: "abcdef012345", tail: 100, status: "timeout", stdout: "", stderr: "", truncated: true, detail: "" };
  const english = logsCopy(report, (key, params) => translate("en", key, params));
  assert.match(english, /State: timeout \(output truncated\)/);
  assert.match(english, /\(no standard output\)/);
});
