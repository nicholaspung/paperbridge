// The whole UI against a mocked Tauri backend.
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

// The real EPUB engine needs a browser too.
vi.mock("./lib/epub", () => ({
  openBook: async (_host: HTMLElement, _bytes: Uint8Array, opts: { onRelocate: (r: object) => void }) => {
    opts.onRelocate({ fraction: 0.25, chapter: "Chapter one" });
    return {
      toc: [{ label: "Chapter one", href: "c1.xhtml", depth: 0 }],
      next: async () => {}, prev: async () => {}, goTo: async () => {}, setStyles() {}, destroy() {},
    };
  },
}));

// The real PDF.js needs a browser; the viewer's logic is tested against a fake.
vi.mock("./lib/pdf", () => ({
  loadPdf: async () => {
    const page = {
      getViewport: ({ scale }: { scale: number }) => ({ width: 600 * scale, height: 800 * scale }),
      render: () => ({ promise: Promise.resolve(), cancel() {} }),
    };
    return { doc: { numPages: 2, getPage: async () => page }, destroy() {} };
  },
}));
import App from "./App.svelte";
import type { DocumentRecord } from "./lib/bindings/DocumentRecord";
import type { Status } from "./lib/bindings/Status";
import { app } from "./state/app.svelte";

const rec = (id: string, parent: string | null, title: string, kind: DocumentRecord["kind"]): DocumentRecord => ({
  id, parent, title, kind, modified: "2024-05-01T10:00:00.5Z", bookmarked: false,
});
const LIBRARY = [
  rec("f1", null, "Projects", "folder"),
  rec("d1", null, "Scratch", "notebook"),
  rec("d2", "f1", "Design spec", "pdf"),
  rec("d3", "f1", "A novel", "epub"),
];

let backend: {
  firstRunDone: boolean;
  status: Status;
  library: DocumentRecord[];
  calls: string[];
  opened: unknown[];
  refuseOpen: string | null;
};

beforeEach(() => {
  backend = {
    firstRunDone: true,
    status: { connection: { state: "connected" }, syncing: false, lastSyncedMs: 1_700_000_000_000, interface: "usb0" },
    library: LIBRARY,
    calls: [],
    opened: [],
    refuseOpen: null,
  };
  app.status = null;
  app.settings = null;
  app.privacy = null;
  app.library = [];
  app.error = null;
  app.job = null;
  app.openRefused = null;
  mockIPC((cmd, args) => {
    backend.calls.push(cmd);
    switch (cmd) {
      case "get_status": return backend.status;
      case "get_library": return backend.library;
      case "get_settings": return { firstRunDone: backend.firstRunDone, os: "linux" };
      case "get_privacy":
        return { level: "unverified", findings: [{ code: "tablet-not-inspected", severity: "info", message: "Keep Wi-Fi off." }] };
      case "complete_first_run": backend.firstRunDone = true; return null;
      case "refresh": return null;
      case "get_job": return null;
      case "get_document_file": return new Uint8Array([37, 80, 68, 70, 45]).buffer;
      case "open_external": return null;
      case "open_document":
        if (backend.refuseOpen) throw backend.refuseOpen;
        backend.opened.push((args as { id: string; format: string }).id + ":" + (args as { format: string }).format);
        return null;
      default: return null; // event plugin calls
    }
  }, { shouldMockEvents: true });
});
afterEach(() => {
  cleanup();
  clearMocks();
});

const titles = () => screen.getAllByTestId("row").map((r) => within(r).getByText(/./, { selector: ".title" }).textContent);

it("shows the root folder, opens a folder, and navigates back", async () => {
  render(App);
  await waitFor(() => expect(titles()).toEqual(["Projects", "Scratch"]));
  expect(screen.getByTestId("connection-badge")).toHaveAttribute("aria-label", "Connected");

  await fireEvent.click(screen.getByText("Projects"));
  expect(titles()).toEqual(["A novel", "Design spec"]);
  const crumbs = screen.getByTestId("breadcrumbs");
  expect(within(crumbs).getAllByRole("button").map((b) => b.getAttribute("aria-label") ?? b.textContent)).toEqual(["My tablet", "Projects"]);

  await fireEvent.click(screen.getByRole("button", { name: "My tablet" }));
  expect(titles()).toEqual(["Projects", "Scratch"]);
});

it("searches the whole library and filters by type", async () => {
  render(App);
  await waitFor(() => screen.getAllByTestId("row"));
  await fireEvent.input(screen.getByTestId("search"), { target: { value: "spec" } });
  expect(titles()).toEqual(["Design spec"]);
  expect(screen.getByTestId("result-count")).toHaveTextContent("1 result");

  await fireEvent.input(screen.getByTestId("search"), { target: { value: "" } });
  await fireEvent.click(screen.getByTestId("filter-epub"));
  expect(titles()).toEqual(["A novel"]);

  await fireEvent.input(screen.getByTestId("search"), { target: { value: "zzz" } });
  expect(screen.getByTestId("empty")).toHaveTextContent("No documents match");

  await fireEvent.click(screen.getByTestId("clear-search"));
  expect(titles()).toEqual(["Projects", "Scratch"]);
});

it("never claims isolation is verified", async () => {
  render(App);
  const badge = await screen.findByTestId("privacy-badge");
  expect(badge).toHaveAttribute("aria-label", "Isolation unverified");
  await fireEvent.click(badge);
  expect(screen.getByTestId("privacy-panel")).toHaveTextContent("cannot confirm that the tablet is offline");
});

it("keeps the cached library visible and explains a hung tablet", async () => {
  backend.status = { ...backend.status, connection: { state: "webInterfaceHung" } };
  render(App);
  const help = await screen.findByTestId("connection-help");
  expect(help).toHaveTextContent("off, then on again");
  await waitFor(() => expect(help).toHaveTextContent("Showing the library as it was"));
  expect(titles()).toEqual(["Projects", "Scratch"]);
  expect(screen.getByTestId("refresh")).toBeDisabled();
});

it("runs the first-launch guide once", async () => {
  backend.firstRunDone = false;
  render(App);
  expect(await screen.findByTestId("first-run")).toHaveTextContent("Wi-Fi off");
  await fireEvent.click(screen.getByTestId("first-run-continue"));
  await waitFor(() => expect(screen.getByTestId("library")).toBeInTheDocument());
  expect(backend.calls).toContain("complete_first_run");
});

it("asks the backend to refresh", async () => {
  render(App);
  await fireEvent.click(await screen.findByTestId("refresh"));
  expect(backend.calls).toContain("refresh");
});

const job = (phase: object, extra: object = {}) => ({
  documentId: "d1", title: "Scratch", phase, startedMs: Date.now(), fromCache: false, format: "pdf", external: false, note: null, ...extra,
});

it("opens a document and shows the export's progress", async () => {
  render(App);
  await waitFor(() => expect(titles()).toEqual(["Projects", "Scratch"]));
  await fireEvent.click(screen.getByText("Scratch"));
  expect(backend.opened).toEqual(["d1:pdf"]);

  await emit("job-changed", job({ phase: "rendering" }));
  const bar = await screen.findByTestId("job-bar");
  await waitFor(() => expect(bar).toHaveTextContent("The tablet is preparing “Scratch” as a PDF"));
  // Nothing else can use the tablet meanwhile.
  expect(screen.getByTestId("refresh")).toBeDisabled();
  await fireEvent.click(screen.getByText("Scratch"));
  expect(backend.opened).toEqual(["d1:pdf"]);

  await emit("job-changed", job({ phase: "receiving", bytes: 3 * 1024 * 1024 }));
  await waitFor(() => expect(screen.getByTestId("job-bar")).toHaveTextContent("3.0 MB"));
  await fireEvent.click(screen.getByTestId("job-cancel"));
  expect(backend.calls).toContain("cancel_job");

  await emit("job-changed", job({ phase: "failed", message: "The tablet stopped answering." }));
  await waitFor(() => expect(screen.getByTestId("job-bar")).toHaveTextContent("Could not open “Scratch”. The tablet stopped answering."));
  await fireEvent.click(screen.getByTestId("job-dismiss"));
  expect(backend.calls).toContain("dismiss_job");
});

it("explains why a document could not be opened", async () => {
  backend.refuseOpen = "Connect the tablet to open this document.";
  render(App);
  await waitFor(() => screen.getAllByTestId("row"));
  await fireEvent.click(screen.getByText("Scratch"));
  expect(await screen.findByTestId("job-bar")).toHaveTextContent("Connect the tablet to open this document.");
});

it("shows a finished document in the app's viewer", async () => {
  render(App);
  await waitFor(() => screen.getAllByTestId("row"));
  await fireEvent.click(screen.getByText("Scratch"));
  await emit("job-changed", job({ phase: "done" }, { fromCache: true }));
  const viewer = await screen.findByTestId("viewer");
  expect(viewer).toHaveTextContent("Scratch");
  await waitFor(() => expect(screen.getAllByTestId("pdf-page")).toHaveLength(2));
  expect(screen.getByTestId("viewer-pages")).toHaveTextContent("1 / 2");
  expect(screen.getByTestId("job-bar")).toHaveTextContent("Showing “Scratch” from the copy on this computer");

  await fireEvent.click(screen.getByTestId("viewer-external"));
  expect(backend.calls).toContain("open_external");

  await fireEvent.click(screen.getByTestId("viewer-close"));
  expect(screen.queryByTestId("viewer")).toBeNull();
  expect(titles()).toEqual(["Projects", "Scratch"]);
});

it("opens a book as its EPUB and can switch to the annotated PDF", async () => {
  render(App);
  await waitFor(() => screen.getAllByTestId("row"));
  await fireEvent.click(screen.getByText("Projects"));
  await fireEvent.click(screen.getByText("A novel"));
  expect(backend.opened).toEqual(["d3:epub"]);

  await emit("job-changed", job({ phase: "done" }, { documentId: "d3", title: "A novel", format: "epub" }));
  const reader = await screen.findByTestId("epub-reader");
  await waitFor(() => expect(screen.getByTestId("epub-progress")).toHaveTextContent("25% · Chapter one"));
  expect(reader).toHaveTextContent("A novel");
  expect(screen.getByTestId("show-epub")).toHaveAttribute("aria-pressed", "true");

  await fireEvent.click(screen.getByTestId("show-pdf"));
  expect(backend.opened).toEqual(["d3:epub", "d3:pdf"]);
  await emit("job-changed", job({ phase: "done" }, { documentId: "d3", title: "A novel", format: "pdf" }));
  expect(await screen.findByTestId("viewer")).toBeInTheDocument();
  expect(screen.queryByTestId("epub-reader")).toBeNull();
  expect(screen.getByTestId("show-pdf")).toHaveAttribute("aria-pressed", "true");
});

it("offers no EPUB switch for notebooks", async () => {
  render(App);
  await waitFor(() => screen.getAllByTestId("row"));
  await fireEvent.click(screen.getByText("Scratch"));
  await emit("job-changed", job({ phase: "done" }));
  await screen.findByTestId("viewer");
  expect(screen.queryByTestId("show-epub")).toBeNull();
});
