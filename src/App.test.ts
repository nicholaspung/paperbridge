// The whole UI against a mocked Tauri backend.
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/svelte";
import { afterEach, beforeEach, expect, it } from "vitest";
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

let backend: { firstRunDone: boolean; status: Status; library: DocumentRecord[]; calls: string[] };

beforeEach(() => {
  backend = {
    firstRunDone: true,
    status: { connection: { state: "connected" }, syncing: false, lastSyncedMs: 1_700_000_000_000, interface: "usb0" },
    library: LIBRARY,
    calls: [],
  };
  app.status = null;
  app.settings = null;
  app.privacy = null;
  app.library = [];
  app.error = null;
  mockIPC((cmd) => {
    backend.calls.push(cmd);
    switch (cmd) {
      case "get_status": return backend.status;
      case "get_library": return backend.library;
      case "get_settings": return { firstRunDone: backend.firstRunDone, os: "linux" };
      case "get_privacy":
        return { level: "unverified", findings: [{ code: "tablet-not-inspected", severity: "info", message: "Keep Wi-Fi off." }] };
      case "complete_first_run": backend.firstRunDone = true; return null;
      case "refresh": return null;
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
  expect(screen.getByTestId("connection-badge")).toHaveTextContent("Connected");

  await fireEvent.click(screen.getByText("Projects"));
  expect(titles()).toEqual(["A novel", "Design spec"]);
  expect(screen.getByTestId("breadcrumbs")).toHaveTextContent("My tablet / Projects");

  await fireEvent.click(screen.getByText("My tablet"));
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
});

it("never claims isolation is verified", async () => {
  render(App);
  const badge = await screen.findByTestId("privacy-badge");
  expect(badge).toHaveTextContent("Isolation unverified");
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
