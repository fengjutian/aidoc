import { test } from "node:test";
import assert from "node:assert/strict";
import { buildTreeIndex } from "../src/nodeTreeIndex.ts";

test("buildTreeIndex: flat list lands at null", () => {
  const { byParent, roots } = buildTreeIndex([
    { id: "a", position: 0 },
    { id: "b", position: 1 },
    { id: "c", position: 2 },
  ]);
  assert.equal(roots.length, 3);
  assert.deepEqual(roots.map((n) => n.id), ["a", "b", "c"]);
  assert.equal(byParent.get("a")?.length ?? 0, 0);
});

test("buildTreeIndex: nested under known parent", () => {
  const { byParent, roots } = buildTreeIndex([
    { id: "root", position: 0 },
    { id: "child-1", position: 0, attributes: { parent: "root" } },
    { id: "child-2", position: 1, attributes: { parent: "root" } },
    { id: "grandchild", position: 0, attributes: { parent: "child-1" } },
  ]);
  assert.deepEqual(roots.map((n) => n.id), ["root"]);
  assert.deepEqual(byParent.get("root")!.map((n) => n.id), ["child-1", "child-2"]);
  assert.deepEqual(byParent.get("child-1")!.map((n) => n.id), ["grandchild"]);
  assert.equal(byParent.get("child-2")?.length ?? 0, 0);
});

test("buildTreeIndex: parent pointing at unknown id falls back to root", () => {
  const { byParent, roots } = buildTreeIndex([
    { id: "a", position: 0, attributes: { parent: "ghost" } },
    { id: "b", position: 1 },
  ]);
  assert.equal(roots.length, 2);
  assert.equal(byParent.get("ghost")?.length ?? 0, 0);
});

test("buildTreeIndex: node pointing at itself falls back to root (cycle prevention)", () => {
  const { byParent, roots } = buildTreeIndex([
    { id: "loop", position: 0, attributes: { parent: "loop" } },
  ]);
  assert.equal(roots.length, 1);
  assert.equal(byParent.get("loop")?.length ?? 0, 0);
});

test("buildTreeIndex: children sorted by position", () => {
  const { byParent } = buildTreeIndex([
    { id: "root", position: 0 },
    { id: "c2", position: 2, attributes: { parent: "root" } },
    { id: "c0", position: 0, attributes: { parent: "root" } },
    { id: "c1", position: 1, attributes: { parent: "root" } },
  ]);
  assert.deepEqual(
    byParent.get("root")!.map((n) => n.id),
    ["c0", "c1", "c2"],
  );
});

test("buildTreeIndex: empty input yields empty index", () => {
  const { byParent, roots } = buildTreeIndex([]);
  assert.equal(roots.length, 0);
  assert.equal(byParent.size, 0);
});