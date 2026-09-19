import { test } from "node:test";
import assert from "node:assert/strict";
import { mergeRecent, parseStored, MAX_ENTRIES } from "../src/hooks/useRecentFiles.ts";

test("mergeRecent: new path moves to head, dedupes old entry", () => {
  const start = ["/a", "/b", "/c"];
  assert.deepEqual(mergeRecent(start, "/b"), ["/b", "/a", "/c"]);
});

test("mergeRecent: brand-new path lands at index 0", () => {
  assert.deepEqual(mergeRecent(["/a", "/b"], "/c"), ["/c", "/a", "/b"]);
});

test("mergeRecent: enforces MAX_ENTRIES cap (default 8)", () => {
  const full = Array.from({ length: 8 }, (_, i) => `/p${i}`);
  const out = mergeRecent(full, "/new");
  assert.equal(out.length, 8);
  assert.equal(out[0], "/new");
  // Order preserved from the original list; oldest /p7 dropped.
  assert.deepEqual(out, [
    "/new", "/p0", "/p1", "/p2", "/p3", "/p4", "/p5", "/p6",
  ]);
});

test("mergeRecent: respects custom max", () => {
  const out = mergeRecent(["/a", "/b", "/c"], "/d", 2);
  assert.deepEqual(out, ["/d", "/a"]);
});

test("MAX_ENTRIES is exported as 8", () => {
  assert.equal(MAX_ENTRIES, 8);
});

test("parseStored: returns [] on null", () => {
  assert.deepEqual(parseStored(null), []);
});

test("parseStored: returns [] on malformed JSON", () => {
  assert.deepEqual(parseStored("not json"), []);
});

test("parseStored: returns [] on non-array JSON", () => {
  assert.deepEqual(parseStored('{"x":1}'), []);
});

test("parseStored: filters out non-string entries", () => {
  assert.deepEqual(parseStored('["/a", 1, null, "/b"]'), ["/a", "/b"]);
});

test("parseStored: returns [] on empty string", () => {
  assert.deepEqual(parseStored(""), []);
});