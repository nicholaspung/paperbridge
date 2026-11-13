import { describe, expect, it } from "vitest";
import type { DocumentRecord } from "./bindings/DocumentRecord";
import { breadcrumbs, childrenOf, formatModified, locationOf, search } from "./library";

const rec = (id: string, parent: string | null, title: string, kind: DocumentRecord["kind"] = "pdf"): DocumentRecord => ({
  id, parent, title, kind, modified: null, bookmarked: false,
});

const lib: DocumentRecord[] = [
  rec("f1", null, "Work", "folder"),
  rec("f2", "f1", "Reports", "folder"),
  rec("d1", null, "zebra notes", "notebook"),
  rec("d2", null, "Apple 10"),
  rec("d3", null, "apple 9"),
  rec("d4", "f2", "Annual Report 2026"),
  rec("d5", "f2", "Annual Report 2026", "epub"),
];

describe("childrenOf", () => {
  it("lists one folder, folders first, natural order", () => {
    expect(childrenOf(lib, null).map((r) => r.id)).toEqual(["f1", "d3", "d2", "d1"]);
  });
  it("keeps duplicate titles apart and stable", () => {
    expect(childrenOf(lib, "f2").map((r) => r.id)).toEqual(["d4", "d5"]);
  });
});

describe("breadcrumbs", () => {
  it("walks from the root down", () => {
    expect(breadcrumbs(lib, "f2").map((r) => r.title)).toEqual(["Work", "Reports"]);
    expect(breadcrumbs(lib, null)).toEqual([]);
  });
  it("survives a missing parent and a cycle", () => {
    expect(breadcrumbs([rec("x", "gone", "X", "folder")], "x").map((r) => r.id)).toEqual(["x"]);
    const loop = [rec("a", "b", "A", "folder"), rec("b", "a", "B", "folder")];
    expect(breadcrumbs(loop, "a")).toHaveLength(2);
  });
});

describe("search", () => {
  it("matches every word, case-insensitively, across folders", () => {
    expect(search(lib, "REPORT annual", new Set()).map((r) => r.id)).toEqual(["d4", "d5"]);
    expect(search(lib, "apple", new Set()).map((r) => r.id)).toEqual(["d3", "d2"]);
  });
  it("filters by kind", () => {
    expect(search(lib, "", new Set(["epub"] as const)).map((r) => r.id)).toEqual(["d5"]);
    expect(search(lib, "report", new Set(["notebook"] as const))).toEqual([]);
  });
});

it("locationOf names the containing folders", () => {
  expect(locationOf(lib, lib[5])).toBe("Work / Reports");
  expect(locationOf(lib, lib[2])).toBe("");
});

it("formatModified tolerates the tablet's short fractions and junk", () => {
  expect(formatModified("2023-12-23T17:52:47.18Z")).toMatch(/2023/);
  expect(formatModified("not a date")).toBe("");
  expect(formatModified(null)).toBe("");
});
