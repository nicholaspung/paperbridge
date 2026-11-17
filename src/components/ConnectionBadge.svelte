<script lang="ts">
  import type { Status } from "../lib/bindings/Status";
  import { connectionMessage } from "../lib/messages";
  import Icon from "./Icon.svelte";

  let { status, os }: { status: Status; os: string } = $props();
  const msg = $derived(connectionMessage(status.connection, os));
  const label = $derived(status.syncing && status.connection.state === "connected" ? "Refreshing…" : msg.label);
</script>

<span
  class="badge {msg.tone}" role="status" data-testid="connection-badge" data-state={status.connection.state}
  aria-label={label} title={`${label}: ${msg.title}`}
>
  <Icon name="plug" size={20} />
  <span class="dot {msg.tone}"></span>
</span>

<style>
  .badge { position: relative; display: inline-flex; align-items: center; padding: 6px; color: var(--muted); }
  .badge.ok { color: var(--ok); }
  .badge.problem { color: var(--problem); }
  .dot { position: absolute; right: 2px; bottom: 3px; width: 8px; height: 8px; box-shadow: 0 0 0 2px var(--surface); }
</style>
