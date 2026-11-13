// Pure functions over the cached library. No Tauri, no DOM.
import type { DocKind } from "./bindings/DocKind";
import type { DocumentRecord } from "./bindings/DocumentRecord";

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

/** Folders first, then by title; ID breaks ties so duplicate titles are stable. */
export function sortRecords(records: DocumentRecord[]): DocumentRecord[] {
  return [...records].sort(
    (a, b) =>
      Number(b.kind === "folder") - Number(a.kind === "folder") ||
      collator.compare(a.title, b.title) ||
      a.id.localeCompare(b.id),
  );
}

export function childrenOf(records: DocumentRecord[], folder: string | null): DocumentRecord[] {
  return sortRecords(records.filter((r) => r.parent === folder));
}

/** Folders from the root down to `folder`. Stops on a missing parent or a cycle. */
export function breadcrumbs(records: DocumentRecord[], folder: string | null): DocumentRecord[] {
  const byId = new Map(records.map((r) => [r.id, r]));
  const trail: DocumentRecord[] = [];
  const seen = new Set<string>();
  let id = folder;
  while (id !== null && !seen.has(id)) {
    seen.add(id);
    const rec = byId.get(id);
    if (!rec) break;
    trail.unshift(rec);
    id = rec.parent;
  }
  return trail;
}

/** "Folder / Subfolder" for a record, or "" in the root. */
export function locationOf(records: DocumentRecord[], rec: DocumentRecord): string {
  return breadcrumbs(records, rec.parent)
    .map((f) => f.title)
    .join(" / ");
}

/** Title search over the whole library; every word must appear. */
export function search(
  records: DocumentRecord[],
  query: string,
  kinds: ReadonlySet<DocKind>,
): DocumentRecord[] {
  const words = query.toLocaleLowerCase().split(/\s+/).filter(Boolean);
  return sortRecords(
    records.filter((r) => {
      if (kinds.size > 0 && !kinds.has(r.kind)) return false;
      const title = r.title.toLocaleLowerCase();
      return words.every((w) => title.includes(w));
    }),
  );
}

export function formatModified(iso: string | null): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

export const KIND_LABEL: Record<DocKind, string> = {
  folder: "Folder",
  pdf: "PDF",
  epub: "EPUB",
  notebook: "Notebook",
  unknown: "Unknown type",
};
