// Strips active content from a book's (X)HTML and SVG before it is shown.
// The app's CSP already blocks scripts; this is the second line of defence.

const DROP = "script, iframe, frame, frameset, object, embed, applet, base, form, meta[http-equiv]";
const URL_ATTRS = ["href", "src", "xlink:href", "action", "formaction", "data", "poster"];
const MARKUP = new Set(["application/xhtml+xml", "text/html", "image/svg+xml"]);

export function isMarkup(type: string): boolean {
  return MARKUP.has(type.split(";")[0].trim());
}

/** Returns sanitized markup and the type it should be served as. */
export function sanitizeMarkup(text: string, type: string): { text: string; type: string } {
  const base = type.split(";")[0].trim() as DOMParserSupportedType;
  let doc = new DOMParser().parseFromString(text, base);
  let outType: string = base;
  if (base !== "text/html" && doc.querySelector("parsererror")) {
    doc = new DOMParser().parseFromString(text, "text/html");
    outType = "text/html";
  }
  for (const el of Array.from(doc.querySelectorAll(DROP))) el.remove();
  for (const el of Array.from(doc.querySelectorAll("*"))) {
    for (const attr of Array.from(el.attributes)) {
      const name = attr.name.toLowerCase();
      const value = attr.value.replace(/[\s\u0000-\u001f]/g, "").toLowerCase();
      if (name.startsWith("on")) el.removeAttribute(attr.name);
      else if (URL_ATTRS.includes(name) && (value.startsWith("javascript:") || value.startsWith("vbscript:"))) {
        el.removeAttribute(attr.name);
      }
    }
  }
  const out =
    outType === "text/html"
      ? "<!DOCTYPE html>" + doc.documentElement.outerHTML
      : new XMLSerializer().serializeToString(doc);
  return { text: out, type: outType };
}
