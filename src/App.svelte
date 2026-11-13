<script lang="ts">
  import { onMount } from "svelte";
  import ConnectionBadge from "./components/ConnectionBadge.svelte";
  import PrivacyBadge from "./components/PrivacyBadge.svelte";
  import { app } from "./state/app.svelte";
  import DeviceLibrary from "./views/DeviceLibrary.svelte";
  import FirstRun from "./views/FirstRun.svelte";

  onMount(() => void app.init());
</script>

<div class="shell">
  <header>
    <strong>PaperBridge</strong>
    <span class="spacer"></span>
    {#if app.status && app.settings}
      <ConnectionBadge status={app.status} os={app.settings.os} />
    {/if}
    {#if app.privacy}
      <PrivacyBadge privacy={app.privacy} />
    {/if}
  </header>

  {#if app.error}
    <p class="error" role="alert" data-testid="error">{app.error}</p>
  {/if}

  <div class="body">
    {#if app.status && app.settings}
      {#if app.settings.firstRunDone}
        <DeviceLibrary library={app.library} status={app.status} os={app.settings.os} onRefresh={() => app.refresh()} />
      {:else}
        <FirstRun status={app.status} os={app.settings.os} onDone={() => app.finishFirstRun()} />
      {/if}
    {/if}
  </div>
</div>

<style>
  .shell { height: 100%; display: flex; flex-direction: column; }
  header { display: flex; align-items: center; gap: 16px; padding: 10px 24px; border-bottom: 1px solid var(--border); background: var(--surface); }
  .spacer { flex: 1; }
  .body { flex: 1; overflow: auto; }
  .error { margin: 12px 24px 0; padding: 10px 14px; border: 1px solid var(--problem); border-radius: 6px; color: var(--problem); }
</style>
