<script lang="ts">
  import type { PrivacyStatus } from "../lib/bindings/PrivacyStatus";

  let { privacy }: { privacy: PrivacyStatus } = $props();
  let open = $state(false);
  const warning = $derived(privacy.level === "warning");
</script>

<div class="wrap">
  <button class="btn" data-testid="privacy-badge" data-level={privacy.level} aria-expanded={open} onclick={() => (open = !open)}>
    <span class="dot {warning ? 'warning' : ''}"></span>
    {warning ? "Privacy warning" : "Isolation unverified"}
  </button>
  {#if open}
    <div class="panel" role="dialog" aria-label="Privacy details" data-testid="privacy-panel">
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
      <button class="btn" onclick={() => (open = false)}>Close</button>
    </div>
  {/if}
</div>

<style>
  .wrap { position: relative; }
  .btn { display: inline-flex; align-items: center; gap: 7px; white-space: nowrap; }
  .panel {
    position: absolute; right: 0; top: calc(100% + 8px); z-index: 10; width: min(420px, 90vw);
    background: var(--surface); border: 1px solid var(--border); border-radius: 8px;
    padding: 16px; display: grid; gap: 10px; box-shadow: 0 8px 24px rgb(0 0 0 / 0.18);
  }
  ul { margin: 0; padding-left: 18px; display: grid; gap: 6px; }
  li.warning { color: var(--warn); font-weight: 600; }
  .panel .btn { justify-self: end; }
</style>
