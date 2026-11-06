import { test } from "node:test";
import assert from "node:assert/strict";
import { crc16, pixPayload } from "../src/pix.ts";

test("CRC16-CCITT matches the standard check value", () => {
  assert.equal(crc16("123456789"), "29B1");
});

test("payload is a valid static BR Code with a correct checksum", () => {
  const payload = pixPayload({
    key: "123e4567-e89b-12d3-a456-426614174000",
    name: "João Paulo Panucci Bernardes",
    city: "São Paulo",
  });
  assert.ok(payload.startsWith("000201010211"));
  assert.ok(payload.includes("br.gov.bcb.pix0136123e4567-e89b-12d3-a456-426614174000"));
  assert.ok(payload.includes("5303986") && payload.includes("5802BR"));
  assert.ok(payload.includes("5925JOAO PAULO PANUCCI BERNAR"));
  assert.ok(payload.includes("6009SAO PAULO"));
  assert.ok(payload.includes("62070503***"));
  assert.equal(payload.slice(-4), crc16(payload.slice(0, -4)));
  assert.ok(!/[^\x20-\x7e]/.test(payload));
});

test("amount is optional, formatted with two decimals and validated", () => {
  const base = { key: "a@b.co", name: "Fulano", city: "Recife" };
  assert.ok(!pixPayload(base).includes("54"));
  assert.ok(pixPayload({ ...base, amount: 25 }).includes("540525.00"));
  for (const amount of [0, -1, NaN, 2_000_000]) {
    assert.throws(() => pixPayload({ ...base, amount }));
  }
});

test("rejects empty or unsafe data", () => {
  assert.throws(() => pixPayload({ key: "", name: "A", city: "B" }));
  assert.throws(() => pixPayload({ key: "a b", name: "A", city: "B" }));
  assert.throws(() => pixPayload({ key: "k", name: "", city: "B" }));
  assert.throws(() => pixPayload({ key: "k", name: "A", city: "" }));
});
