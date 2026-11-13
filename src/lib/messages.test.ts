import { expect, it } from "vitest";
import type { ConnectionState } from "./bindings/ConnectionState";
import { connectionMessage } from "./messages";

const states: ConnectionState[] = [
  { state: "noUsbInterface" },
  { state: "usbInterfaceUp" },
  { state: "connected" },
  { state: "localNetworkPermissionDenied" },
  { state: "webInterfaceOff" },
  { state: "webInterfaceHung" },
  { state: "unsupportedResponse", detail: "listing returned HTTP 500" },
  { state: "transportError", detail: "boom" },
];

it("every state has a label, and every problem says what to do", () => {
  for (const os of ["linux", "macos"]) {
    for (const s of states) {
      const m = connectionMessage(s, os);
      expect(m.label).not.toBe("");
      expect(m.title).not.toBe("");
      if (m.tone === "problem") expect(m.steps.length).toBeGreaterThan(0);
    }
  }
});

it("only a real connection is shown as ok", () => {
  expect(states.filter((s) => connectionMessage(s, "linux").tone === "ok")).toEqual([{ state: "connected" }]);
});

it("includes the technical detail where there is one", () => {
  expect(connectionMessage(states[6], "linux").steps.join(" ")).toContain("HTTP 500");
});
