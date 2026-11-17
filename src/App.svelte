<script lang="ts">
  import { onMount } from "svelte";
  import ConnectionBadge from "./components/ConnectionBadge.svelte";
  import JobBar from "./components/JobBar.svelte";
  import PrivacyBadge from "./components/PrivacyBadge.svelte";
  import { app } from "./state/app.svelte";
  import DeviceLibrary from "./views/DeviceLibrary.svelte";
  import FirstRun from "./views/FirstRun.svelte";
  import Viewer from "./views/Viewer.svelte";
  import EpubReader from "./views/EpubReader.svelte";
  import Icon from "./components/Icon.svelte";

  const viewedKind = $derived(app.library.find((r) => r.id === app.viewing?.id)?.kind);

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
        {#snippet switcher()}
          {#if viewedKind === "epub"}
            <div class="seg" role="group" aria-label="Show as">
              <button
                class="icon-btn plain" data-testid="show-epub" aria-pressed={app.viewing?.format === "epub"}
                aria-label="Read the book" title="Read the book (EPUB, without annotations)"
                disabled={app.jobActive} onclick={() => app.switchFormat("epub")}
              ><Icon name="epub" size={18} /></button>
              <button
                class="icon-btn plain" data-testid="show-pdf" aria-pressed={app.viewing?.format === "pdf"}
                aria-label="With annotations" title="With annotations (the tablet's PDF)"
                disabled={app.jobActive} onclick={() => app.switchFormat("pdf")}
              ><Icon name="notebook" size={18} /></button>
            </div>
          {/if}
        {/snippet}
        {#if app.viewing}
          {#key `${app.viewing.id}:${app.viewing.format}`}
            {#if app.viewing.format === "epub"}
              <EpubReader
                id={app.viewing.id} title={app.viewing.title} {switcher}
                onClose={() => app.closeViewer()} onExternal={() => app.openExternal()}
              />
            {:else}
              <Viewer
                id={app.viewing.id} title={app.viewing.title} {switcher}
                onClose={() => app.closeViewer()} onExternal={() => app.openExternal()}
              />
            {/if}
          {/key}
        {/if}
        <div class="lib" hidden={!!app.viewing}>
        <DeviceLibrary
          library={app.library} status={app.status} os={app.settings.os}
          onRefresh={() => app.refresh()} onOpen={(rec) => app.open(rec)}
          busy={app.jobActive} openingId={app.jobActive ? (app.job?.documentId ?? null) : null}
        />
        </div>
      {:else}
        <FirstRun status={app.status} os={app.settings.os} onDone={() => app.finishFirstRun()} />
      {/if}
    {/if}
  </div>

  <JobBar job={app.job} refused={app.openRefused} onCancel={() => app.cancelJob()} onDismiss={() => app.dismissJob()} />
</div>

<style>
  .shell { height: 100%; display: flex; flex-direction: column; }
  header { display: flex; align-items: center; gap: 16px; padding: 10px 24px; border-bottom: 1px solid var(--border); background: var(--surface); }
  .spacer { flex: 1; }
  .body { flex: 1; overflow: auto; min-height: 0; }
  .body:has(:global([data-testid="viewer"], [data-testid="epub-reader"])) { overflow: hidden; }
  .seg { display: inline-flex; border: 1px solid var(--border); border-radius: 7px; padding: 1px; gap: 1px; margin: 0 4px; }
  .seg .icon-btn { width: 30px; height: 30px; }
  .seg .icon-btn[aria-pressed="true"] { background: var(--hover); color: var(--accent); }
  .error { margin: 12px 24px 0; padding: 10px 14px; border: 1px solid var(--problem); border-radius: 6px; color: var(--problem); }
</style>
