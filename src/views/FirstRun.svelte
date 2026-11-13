<script lang="ts">
  import type { Status } from "../lib/bindings/Status";
  import { connectionMessage } from "../lib/messages";

  let { status, os, onDone }: { status: Status; os: string; onDone: () => void } = $props();
  const msg = $derived(connectionMessage(status.connection, os));
  const connected = $derived(status.connection.state === "connected");
</script>

<main data-testid="first-run">
  <h1>Welcome to PaperBridge</h1>
  <p>
    PaperBridge manages your reMarkable over the USB cable. It makes no internet connections, needs
    no account, and never changes anything on the tablet without you asking.
  </p>

  <ol>
    <li>On the tablet, turn <strong>Wi-Fi off</strong>.</li>
    <li>Plug the tablet into this computer, wake it and unlock it.</li>
    <li>On the tablet, open <strong>Menu → Settings → Storage</strong> and switch on <strong>USB web interface</strong>.</li>
    {#if os === "macos"}
      <li>
        macOS will ask whether PaperBridge may find devices on your local network. Choose
        <strong>Allow</strong>: the tablet's USB connection counts as a local network.
      </li>
    {:else}
      <li>Your computer should set up the tablet's USB network connection by itself within a few seconds.</li>
    {/if}
  </ol>

  <section class="status" data-testid="first-run-status" data-state={status.connection.state}>
    <h2><span class="dot {msg.tone}"></span> {msg.title}</h2>
    {#if msg.steps.length}
      <ul>{#each msg.steps as step}<li>{step}</li>{/each}</ul>
    {/if}
  </section>

  <div class="actions">
    <button class="btn primary" data-testid="first-run-continue" onclick={onDone}>
      {connected ? "Show my library" : "Continue without the tablet"}
    </button>
  </div>
</main>

<style>
  main { max-width: 640px; margin: 0 auto; padding: 40px 24px; display: grid; gap: 20px; }
  ol, ul { margin: 0; padding-left: 22px; display: grid; gap: 8px; }
  .status { background: var(--surface); border: 1px solid var(--border); border-radius: 8px; padding: 16px; display: grid; gap: 10px; }
  h2 { font-size: 1rem; display: flex; align-items: center; gap: 8px; }
  .actions { display: flex; justify-content: flex-end; }
</style>
