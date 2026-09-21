import { test } from "node:test";
import assert from "node:assert/strict";
import { wrapForKind, unwrapForKind } from "../src/editorSerialize.ts";

test("wrapForKind: empty content becomes empty paragraph", () => {
  assert.equal(wrapForKind("paragraph", ""), "<p></p>");
});

test("wrapForKind: passthrough for content already starting with <", () => {
  assert.equal(
    wrapForKind("paragraph", "<p>hi</p>"),
    "<p>hi</p>",
  );
});

test("wrapForKind: code kind wraps in pre+code; HTML-looking content passes through unescaped", () => {
  // Once content already looks like HTML (starts with `<`) we trust the
  // caller and skip the escape step. This matches the in-tree Tiptap
  // round-trip, which always emits valid HTML.
  assert.equal(
    wrapForKind("code", "<pre><code>x</code></pre>"),
    "<pre><code>x</code></pre>",
  );
});

test("wrapForKind: code kind escapes HTML when content is plain text", () => {
  assert.equal(
    wrapForKind("code", "if (a < b && b > c)"),
    "<pre><code>if (a &lt; b &amp;&amp; b &gt; c)</code></pre>",
  );
});

test("wrapForKind: heading becomes h2", () => {
  assert.equal(wrapForKind("heading", "Title"), "<h2>Title</h2>");
});

test("wrapForKind: multiline heading preserves following lines", () => {
  assert.equal(
    wrapForKind("heading", "Title\nBody one\nBody two"),
    "<h2>Title</h2><p>Body one</p><p>Body two</p>",
  );
});

test("unwrapForKind: block elements preserve line breaks", () => {
  assert.equal(
    unwrapForKind("paragraph", "<p>line1</p><p>line2</p>"),
    "line1\nline2",
  );
});

test("wrapForKind: link kind emits an anchor", () => {
  const out = wrapForKind("link", "https://example.com");
  assert.equal(
    out,
    '<p><a href="https://example.com">https://example.com</a></p>',
  );
});

test("wrapForKind: image kind emits an img", () => {
  const out = wrapForKind("image", "https://x/y.png");
  assert.equal(
    out,
    '<p><img src="https://x/y.png" alt="image"/></p>',
  );
});

test("wrapForKind: details wraps body in details+summary", () => {
  assert.equal(
    wrapForKind("details", "body"),
    "<details><summary>Details</summary><p>body</p></details>",
  );
});

test("wrapForKind: summary emits a summary element", () => {
  assert.equal(wrapForKind("summary", "click me"), "<summary>click me</summary>");
});

test("unwrapForKind: link reverse-extracts href", () => {
  assert.equal(
    unwrapForKind("link", '<p><a href="https://x">x</a></p>'),
    "https://x",
  );
});

test("unwrapForKind: image reverse-extracts src", () => {
  assert.equal(
    unwrapForKind("image", '<p><img src="https://y/z.png" alt="image"/></p>'),
    "https://y/z.png",
  );
});

test("unwrapForKind: details strips wrapper", () => {
  const out = unwrapForKind(
    "details",
    "<details><summary>Details</summary><p>body</p></details>",
  );
  assert.ok(out.includes("body"));
  assert.ok(!out.includes("<details"));
});

test("unwrapForKind: summary strips summary tag", () => {
  assert.equal(unwrapForKind("summary", "<summary>x</summary>"), "x");
});

test("unwrapForKind: code round-trips plain-text content", () => {
  const original = "if (a < b)";
  const html = wrapForKind("code", original);
  assert.equal(unwrapForKind("code", html), original);
});

test("wrapForKind: paragraph newlines become separate p tags", () => {
  assert.equal(
    wrapForKind("paragraph", "line1\nline2"),
    "<p>line1</p><p>line2</p>",
  );
});

test("wrapForKind: paragraph preserves intentional blank lines", () => {
  assert.equal(
    wrapForKind("paragraph", "line1\n\nline2"),
    "<p>line1</p><p><br></p><p>line2</p>",
  );
});

test("unwrapForKind: empty paragraphs remain blank lines", () => {
  assert.equal(
    unwrapForKind("paragraph", "<p>line1</p><p><br></p><p>line2</p>"),
    "line1\n\nline2",
  );
});
