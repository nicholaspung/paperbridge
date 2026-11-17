import { expect, it } from "vitest";
import { isMarkup, sanitizeMarkup } from "./sanitize";

const XHTML = "application/xhtml+xml";

it("removes scripts, handlers, frames and javascript: links", () => {
  const src = `<?xml version="1.0"?><html xmlns="http://www.w3.org/1999/xhtml"><head><script>alert(1)</script>
    <meta http-equiv="refresh" content="0;url=https://example.com"/></head>
    <body onload="x()"><p onclick="y()" class="keep">Text</p>
    <a href="javascript:alert(1)">bad</a><a href="  JaVa&#x0A;script:alert(1)">bad2</a><a href="c2.xhtml">good</a>
    <iframe src="https://example.com"></iframe><object data="x.swf"></object><svg xmlns="http://www.w3.org/2000/svg"><script>z()</script></svg>
    </body></html>`;
  const { text, type } = sanitizeMarkup(src, XHTML);
  expect(type).toBe(XHTML);
  expect(text).not.toMatch(/script>|onload|onclick|iframe|object|http-equiv|javascript:/i);
  expect(text).toContain('class="keep"');
  expect(text).toContain('href="c2.xhtml"');
  expect(text).toContain(">Text<");
});

it("falls back to HTML for broken XHTML and still sanitizes", () => {
  const { text, type } = sanitizeMarkup("<html><body><p>unclosed<script>bad()</script></body></html>", XHTML);
  expect(type).toBe("text/html");
  expect(text).not.toContain("script");
  expect(text).toContain("unclosed");
});

it("knows which resources are markup", () => {
  expect(isMarkup("application/xhtml+xml")).toBe(true);
  expect(isMarkup("image/svg+xml")).toBe(true);
  expect(isMarkup("text/css")).toBe(false);
  expect(isMarkup("image/png")).toBe(false);
});
