import test from "node:test";
import assert from "node:assert/strict";
import { logText, logsCopy } from "../src/logs-format.ts";

test("logs remain plain text, with ANSI and terminal controls removed", () => {
  assert.equal(logText("\x1b[31merror\x1b[0m\n\tline\x00"), "error\n\tline");
  assert.equal(logText("\x1b]0;title\x07text\u202e"), "text");
  assert.equal(
    logText("<script>alert(1)</script>"),
    "<script>alert(1)</script>",
  );
});
test("copy identifies streams, truncation and failure, without omitting empty stdout", () => {
  const result = logsCopy({
    containerId: "abcdef012345",
    tail: 100,
    status: "timeout",
    stdout: "",
    stderr: "test failure",
    truncated: true,
    detail: "deadline",
  });
  assert.match(result, /--tail 100 -- abcdef012345/);
  assert.match(result, /\[stdout\]\n\(sem saída padrão\)/);
  assert.match(result, /\[stderr\]\ntest failure/);
  assert.match(result, /timeout \(saída truncada\)/);
});
