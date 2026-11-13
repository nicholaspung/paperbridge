// The release binary must ignore PB_DEVICE_BASE_URL (plan §4.1). Point the
// variable at a loopback listener, run the app, and fail if it ever connects.
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import net from "node:net";

const binary = process.argv[2] ?? "target/release/paperbridge";
const RUN_MS = 12_000; // four of the app's 3-second detection cycles

if (!existsSync(binary)) {
  console.error(`release binary not found: ${binary}`);
  process.exit(2);
}
let hits = 0;
const server = net.createServer((socket) => {
  hits++;
  socket.destroy();
});
server.listen(0, "127.0.0.1", () => {
  const url = `http://127.0.0.1:${server.address().port}`;
  const app = spawn(binary, [], { env: { ...process.env, PB_DEVICE_BASE_URL: url }, stdio: "inherit" });
  let exitedEarly = false;
  app.on("exit", () => (exitedEarly = true));
  setTimeout(() => {
    const early = exitedEarly;
    app.kill("SIGTERM");
    server.close();
    if (early) {
      console.error("the app exited before the check finished, so the check proves nothing");
      process.exit(1);
    }
    if (hits > 0) {
      console.error(`FAILED: the release binary connected to ${url} ${hits} time(s)`);
      process.exit(1);
    }
    console.log("release binary ignored PB_DEVICE_BASE_URL");
    process.exit(0);
  }, RUN_MS);
});
