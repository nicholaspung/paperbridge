// Runs a command under strace and fails if any process it starts connects to
// anything other than loopback or a local Unix socket.
//
//   node scripts/network-audit.mjs [--timeout SECONDS] -- <command> [args...]
//
// With --timeout the command is stopped after that long and only the audit
// decides the result (for apps that never exit by themselves). Linux only.
import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";

const args = process.argv.slice(2);
const sep = args.indexOf("--");
if (sep < 0 || sep === args.length - 1) {
  console.error("usage: network-audit.mjs [--timeout SECONDS] -- <command> [args...]");
  process.exit(2);
}
const tIdx = args.indexOf("--timeout");
const timeout = tIdx >= 0 && tIdx < sep ? Number(args[tIdx + 1]) : null;
const command = args.slice(sep + 1);
const log = path.join(mkdtempSync(path.join(os.tmpdir(), "pb-audit-")), "connect.log");

/** Returns a description of the connection if it leaves the machine, else null. */
export function offending(line) {
  if (!line.includes("connect(")) return null;
  const v4 = line.match(/sa_family=AF_INET, sin_port=htons\((\d+)\), sin_addr=inet_addr\("([^"]+)"\)/);
  if (v4) return v4[2].startsWith("127.") ? null : `${v4[2]}:${v4[1]}`;
  const v6 = line.match(/sa_family=AF_INET6, sin6_port=htons\((\d+)\).*?inet_pton\(AF_INET6, "([^"]+)"/);
  if (v6) return v6[2] === "::1" || v6[2].startsWith("::ffff:127.") ? null : `[${v6[2]}]:${v6[1]}`;
  return null; // AF_UNIX, AF_NETLINK, AF_UNSPEC: local
}

const child = spawn("strace", ["-f", "-qq", "-e", "trace=connect", "-o", log, ...command], {
  stdio: "inherit",
});
let timedOut = false;
const timer =
  timeout &&
  setTimeout(() => {
    timedOut = true;
    child.kill("SIGTERM");
  }, timeout * 1000);

child.on("exit", (code, signal) => {
  if (timer) clearTimeout(timer);
  const lines = readFileSync(log, "utf8").split("\n");
  const bad = [...new Set(lines.map(offending).filter(Boolean))];
  const connects = lines.filter((l) => l.includes("connect(")).length;
  console.log(`network audit: ${connects} connect() calls traced`);
  if (connects === 0) {
    console.error("network audit: nothing was traced, so the audit proves nothing");
    process.exit(1);
  }
  if (bad.length) {
    console.error(`network audit FAILED: connections left the machine:\n  ${bad.join("\n  ")}`);
    process.exit(1);
  }
  console.log("network audit passed: loopback and local sockets only");
  if (timeout) process.exit(timedOut ? 0 : 1); // exiting early means the app crashed
  process.exit(code ?? (signal ? 1 : 0));
});
