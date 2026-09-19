/**
 * Build the parent → ordered-children index consumed by `NodeTree`. Pure
 * helper extracted so it can be unit-tested without dragging React / JSX
 * into the test runner.
 */

export interface TreeNode {
  id: string;
  position: number;
  attributes?: Record<string, string>;
}

export interface TreeIndex<T extends TreeNode = TreeNode> {
  byParent: Map<string | null, T[]>;
  roots: T[];
}

export function buildTreeIndex<T extends TreeNode>(nodes: T[]): TreeIndex<T> {
  const byParent = new Map<string | null, T[]>();
  const known = new Set(nodes.map((n) => n.id));
  for (const n of nodes) {
    const raw = n.attributes?.parent;
    const key = raw && raw !== n.id && known.has(raw) ? raw : null;
    const arr = byParent.get(key) ?? [];
    arr.push(n);
    byParent.set(key, arr);
  }
  for (const arr of byParent.values()) {
    arr.sort((a, b) => a.position - b.position);
  }
  const roots = byParent.get(null) ?? [];
  return { byParent, roots };
}