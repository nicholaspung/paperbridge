<script lang="ts">
  import type { Status } from "../lib/bindings/Status";
  import { connectionMessage } from "../lib/messages";

  let { status, os }: { status: Status; os: string } = $props();
  const msg = $derived(connectionMessage(status.connection, os));
</script>

<span class="badge" data-testid="connection-badge" data-state={status.connection.state} title={msg.title}>
  <span class="dot {msg.tone}"></span>
  {status.syncing && status.connection.state === "connected" ? "Refreshing…" : msg.label}
</span>

<style>
  .badge { display: inline-flex; align-items: center; gap: 7px; white-space: nowrap; }
</style>
