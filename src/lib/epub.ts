// The EPUB engine (foliate-js), loaded lazily by the reader. Every (X)HTML
// and SVG resource is sanitized before it becomes a page, links to the
// internet are never followed, and nothing is fetched from the network.
import "foliate-js/view.js";
import { isMarkup, sanitizeMarkup } from "./sanitize";

export interface TocItem {
  label: string;
  href: string;
  depth: number;
}

export interface Relocation {
  fraction: number;
  cfi?: string;
  chapter?: string;
}

/* eslint-disable @typescript-eslint/no-explicit-any */
export interface Book {
  view: any;
  toc: TocItem[];
  next(): Promise<void>;
  prev(): Promise<void>;
  goTo(target: string): Promise<void>;
  setStyles(css: string): void;
  destroy(): void;
}

function flatten(items: any[] | undefined, depth = 0, out: TocItem[] = []): TocItem[] {
  for (const it of items ?? []) {
    if (it?.href) out.push({ label: String(it.label ?? "").trim() || "Untitled", href: it.href, depth });
    flatten(it?.subitems, depth + 1, out);
  }
  return out;
}

export async function openBook(
  host: HTMLElement,
  bytes: Uint8Array,
  opts: { lastLocation?: string | null; onRelocate: (r: Relocation) => void; onLoad?: (doc: Document) => void },
): Promise<Book> {
  const view: any = document.createElement("foliate-view");
  view.style.display = "block";
  view.style.width = "100%";
  view.style.height = "100%";
  host.append(view);

  // Never navigate away or open windows for links out of the book.
  view.addEventListener("external-link", (e: Event) => e.preventDefault());
  view.addEventListener("relocate", (e: CustomEvent) => {
    const d = e.detail ?? {};
    opts.onRelocate({ fraction: d.fraction ?? 0, cfi: d.cfi, chapter: d.tocItem?.label });
  });
  view.addEventListener("load", (e: CustomEvent) => opts.onLoad?.(e.detail.doc));

  const file = new File([bytes as BlobPart], "book.epub", { type: "application/epub+zip" });
  await view.open(file);
  view.book.transformTarget?.addEventListener("data", (e: CustomEvent) => {
    const detail = e.detail;
    detail.data = Promise.all([detail.data, detail.type]).then(([data, type]) => {
      if (typeof data !== "string" || !isMarkup(String(type))) return data;
      const clean = sanitizeMarkup(data, String(type));
      detail.type = clean.type;
      return clean.text;
    });
  });

  const r = view.renderer;
  r.setAttribute("flow", "paginated");
  r.setAttribute("max-inline-size", "680px");
  r.setAttribute("max-column-count", "2");
  r.setAttribute("margin", "40px");
  r.setAttribute("gap", "6%");

  await view.init({ lastLocation: opts.lastLocation ?? undefined, showTextStart: true });

  return {
    view,
    toc: flatten(view.book.toc),
    next: () => view.next(),
    prev: () => view.prev(),
    goTo: (t) => view.goTo(t),
    setStyles: (css) => r.setStyles?.(css),
    destroy: () => {
      try {
        view.close?.();
      } finally {
        view.remove();
      }
    },
  };
}
