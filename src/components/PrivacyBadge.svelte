<script lang="ts">
  import type { PrivacyStatus } from "../lib/bindings/PrivacyStatus";
  import Icon from "./Icon.svelte";

  let { privacy }: { privacy: PrivacyStatus } = $props();
  let open = $state(false);
  const warning = $derived(privacy.level === "warning");
  const label = $derived(warning ? "Privacy warning" : "Isolation unverified");
</script>

<div class="wrap">
  <button
    class="icon-btn plain badge" class:warning data-testid="privacy-badge" data-level={privacy.level}
    aria-label={label} title={label} aria-expanded={open} onclick={() => (open = !open)}
  >
    <Icon name="shield" size={20} badge={warning ? "alert" : undefined} />
  </button>
  {#if open}
    <div class="panel" role="dialog" aria-label="Privacy details" data-testid="privacy-panel">
      <button class="icon-btn plain close" aria-label="Close" title="Close" onclick={() => (open = false)}>
        <Icon name="close" size={16} />
      </button>
      <h3>{warning ? "Something needs your attention" : "Tablet isolation is not verified"}</h3>
      <p class="muted">
        PaperBridge itself only talks to the tablet over the USB cable and makes no internet
        connections. It cannot confirm that the tablet is offline.
      </p>
      <ul>
        {#each privacy.findings as f (f.code)}
          <li class={f.severity}>{f.message}</li>
        {/each}
      </ul>
    </div>
  {/if}
</div>

<style>
  .wrap { position: relative; }
  .badge { color: var(--muted); }
  .badge.warning { color: var(--warn); }
  .close { position: absolute; top: 8px; right: 8px; width: 28px; height: 28px; }
  h3 { padding-right: 28px; }
  .panel {
    position: absolute; right: 0; top: calc(100% + 8px); z-index: 10; width: min(420px, 90vw);
    background: var(--surface); border: 1px solid var(--border); border-radius: 8px;
    padding: 16px; display: grid; gap: 10px; box-shadow: 0 8px 24px rgb(0 0 0 / 0.18);
  }
  ul { margin: 0; padding-left: 18px; display: grid; gap: 6px; }
  li.warning { color: var(--warn); font-weight: 600; }
</style>
