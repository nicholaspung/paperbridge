// End-to-end: the real app (built with --features mock-device) driven through
// tauri-driver, talking to pb-mock-tablet on loopback. Linux only.
import { spawn } from "node:child_process";
import { mkdtempSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
export const MOCK_URL = "http://127.0.0.1:18555";

// The app is started by the WebDriver and inherits this environment.
process.env.PB_DEVICE_BASE_URL = MOCK_URL;
process.env.PB_DATA_DIR = mkdtempSync(path.join(os.tmpdir(), "pb-e2e-"));

let mock;
let driver;
const stop = (child) => child && !child.killed && child.kill();

async function waitForPort(url, what) {
  for (let i = 0; i < 100; i++) {
    try {
      await fetch(url);
      return;
    } catch {
      await new Promise((r) => setTimeout(r, 100));
    }
  }
  throw new Error(`${what} did not start`);
}

export const config = {
  hostname: "127.0.0.1",
  port: 4444,
  specs: ["./specs/**/*.js"],
  maxInstances: 1,
  capabilities: [
    { maxInstances: 1, "tauri:options": { application: path.join(root, "target/debug/paperbridge") } },
  ],
  reporters: ["spec"],
  framework: "mocha",
  mochaOpts: { ui: "bdd", timeout: 90_000 },
  logLevel: "warn",

  onPrepare: async () => {
    mock = spawn(
      path.join(root, "target/debug/pb-mock-tablet"),
      [path.join(root, "tests/fixtures/3.29.1.0"), "18555"],
      { stdio: "inherit" },
    );
    await waitForPort(`${MOCK_URL}/__mock/fault/none`, "pb-mock-tablet");
  },
  beforeSession: async () => {
    driver = spawn(path.join(os.homedir(), ".cargo/bin/tauri-driver"), [], { stdio: "inherit" });
    await waitForPort("http://127.0.0.1:4444/status", "tauri-driver");
  },
  afterSession: () => stop(driver),
  onComplete: () => stop(mock),
};

process.on("exit", () => {
  stop(driver);
  stop(mock);
});
