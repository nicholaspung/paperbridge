// Plain-language explanations for each connection state.
import type { ConnectionState } from "./bindings/ConnectionState";

export type Tone = "ok" | "idle" | "problem";

export interface ConnectionMessage {
  label: string;
  tone: Tone;
  title: string;
  steps: string[];
}

export function connectionMessage(c: ConnectionState, os: string): ConnectionMessage {
  switch (c.state) {
    case "connected":
      return { label: "Connected", tone: "ok", title: "Tablet connected over USB", steps: [] };
    case "usbInterfaceUp":
      return { label: "Connecting…", tone: "idle", title: "Tablet found, connecting…", steps: [] };
    case "noUsbInterface":
      return {
        label: "Not connected",
        tone: "idle",
        title: "No tablet found",
        steps: [
          "Plug the tablet into this computer with a USB cable.",
          "Wake the tablet and unlock it.",
          os === "linux"
            ? "If nothing happens, try another cable or port. Some cables only charge."
            : "If nothing happens, try another cable or port, and check that the tablet appears under System Settings → Network.",
        ],
      };
    case "webInterfaceOff":
      return {
        label: "Web interface off",
        tone: "problem",
        title: "The tablet is connected, but its USB web interface is switched off",
        steps: [
          "On the tablet, open Menu → Settings → Storage.",
          "Switch on “USB web interface”. The cable must stay plugged in.",
        ],
      };
    case "webInterfaceHung":
      return {
        label: "Tablet not responding",
        tone: "problem",
        title: "The tablet accepted the connection but is not answering",
        steps: [
          "On the tablet, open Menu → Settings → Storage.",
          "Switch “USB web interface” off, then on again.",
          "If that does not help, restart the tablet and switch the setting back on.",
        ],
      };
    case "localNetworkPermissionDenied":
      return {
        label: "Blocked by macOS",
        tone: "problem",
        title: "macOS is blocking PaperBridge from reaching the tablet",
        steps: [
          "Open System Settings → Privacy & Security → Local Network.",
          "Switch PaperBridge on.",
          "PaperBridge only talks to the tablet at 10.11.99.1 over the cable.",
        ],
      };
    case "unsupportedResponse":
      return {
        label: "Unexpected reply",
        tone: "problem",
        title: "The tablet replied in a way PaperBridge does not understand",
        steps: [
          "This can happen after a tablet software update.",
          `Details: ${c.detail}`,
        ],
      };
    case "transportError":
      return {
        label: "Connection problem",
        tone: "problem",
        title: "Something went wrong talking to the tablet",
        steps: ["Check the cable and try again.", `Details: ${c.detail}`],
      };
  }
}
