<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import KindIcon from "../components/KindIcon.svelte";
  import type { DocKind } from "../lib/bindings/DocKind";
  import type { DocumentRecord } from "../lib/bindings/DocumentRecord";
  import type { Status } from "../lib/bindings/Status";
  import { breadcrumbs, childrenOf, formatModified, KIND_LABEL, locationOf, search } from "../lib/library";
  import { connectionMessage } from "../lib/messages";

  let { library, status, os, onRefresh, onOpen, busy = false, openingId = null }: {
    library: DocumentRecord[];
    status: Status;
    os: string;
    onRefresh: () => void;
    onOpen: (rec: DocumentRecord) => void;
    /** A document is being prepared; others cannot be opened meanwhile. */
    busy?: boolean;
    openingId?: string | null;
  } = $props();

  let folder = $state<string | null>(null);
  let query = $state("");
  let kinds = $state<DocKind[]>([]);

  const FILTERS: DocKind[] = ["notebook", "pdf", "epub"];
  const msg = $derived(connectionMessage(status.connection, os));
  const connected = $derived(status.connection.state === "connected");
  const searching = $derived(query.trim() !== "" || kinds.length > 0);
  // If the open folder vanished from the tablet, fall back to the root.
  const current = $derived(folder !== null && library.some((r) => r.id === folder) ? folder : null);
  const trail = $derived(breadcrumbs(library, current));
  const rows = $derived(searching ? search(library, query, new Set(kinds)) : childrenOf(library, current));
  const refreshLabel = $derived(
    status.syncing ? "Refreshing…" : busy ? "Available when the current document is ready" : "Refresh from the tablet",
  );
  const synced = $derived(
    status.lastSyncedMs === null ? null : new Date(status.lastSyncedMs).toLocaleString(),
  );

  function toggle(kind: DocKind) {
    kinds = kinds.includes(kind) ? kinds.filter((k) => k !== kind) : [...kinds, kind];
  }
  function clearSearch() {
    query = "";
    kinds = [];
  }
  function open(rec: DocumentRecord) {
    if (rec.kind !== "folder") {
      if (!busy) onOpen(rec);
      return;
    }
    folder = rec.id;
    clearSearch();
  }
</script>

<main data-testid="library">
  {#if !connected}
    <section class="notice" data-testid="connection-help" data-state={status.connection.state}>
      <h2><span class="dot {msg.tone}"></span> {msg.title}</h2>
      {#if msg.steps.length}
        <ul>{#each msg.steps as step}<li>{step}</li>{/each}</ul>
      {/if}
      {#if library.length}
        <p class="muted">Showing the library as it was {synced ? `on ${synced}` : "at the last sync"}.</p>
      {/if}
    </section>
  {/if}

  <div class="toolbar">
    <div class="search">
      <span class="search-icon muted"><Icon name="search" size={18} /></span>
      <input
        type="search" placeholder="Search titles" aria-label="Search titles"
        data-testid="search" bind:value={query}
      />
      {#if searching}
        <button class="icon-btn plain clear" data-testid="clear-search" aria-label="Clear search" title="Clear search" onclick={clearSearch}>
          <Icon name="close" size={16} />
        </button>
      {/if}
    </div>
    <div class="filters" role="group" aria-label="Filter by type">
      {#each FILTERS as kind}
        {@const label = `Show only ${KIND_LABEL[kind]}s`}
        <button
          class="icon-btn chip" aria-pressed={kinds.includes(kind)} data-testid="filter-{kind}"
          aria-label={label} title={label} onclick={() => toggle(kind)}
        >
          <KindIcon {kind} size={20} />
        </button>
      {/each}
    </div>
    <button
      class="icon-btn" data-testid="refresh" aria-label={refreshLabel} title={refreshLabel}
      disabled={!connected || status.syncing || busy} onclick={onRefresh}
    >
      <span class:spinning={status.syncing} class="spin-wrap"><Icon name="refresh" size={18} /></span>
    </button>
  </div>

  {#if searching}
    <p class="muted" data-testid="result-count">
      {rows.length} {rows.length === 1 ? "result" : "results"} in the whole library
    </p>
  {:else}
    <nav aria-label="Folder path" data-testid="breadcrumbs">
      <button class="crumb home" aria-label="My tablet" title="My tablet" onclick={() => (folder = null)}>
        <Icon name="tablet" size={18} />
      </button>
      {#each trail as f (f.id)}
        <span class="muted sep"><Icon name="chevron" size={14} /></span>
        <button class="crumb" onclick={() => (folder = f.id)}>{f.title}</button>
      {/each}
    </nav>
  {/if}

  {#if rows.length === 0}
    <p class="empty muted" data-testid="empty">
      {#if library.length === 0}
        {connected ? "The tablet has no documents." : "Nothing to show yet. Connect the tablet to load its library."}
      {:else if searching}
        No documents match.
      {:else}
        This folder is empty.
      {/if}
    </p>
  {:else}
    <ul class="rows" data-testid="rows">
      {#each rows as rec (rec.id)}
        <li>
          <button
            class="row" data-testid="row" data-kind={rec.kind}
            aria-disabled={rec.kind !== "folder" && busy}
            title={rec.kind === "folder" ? undefined : busy ? "Another document is being prepared" : "Open as PDF"}
            onclick={() => open(rec)}
          >
            <KindIcon kind={rec.kind} />
            <span class="title">{rec.title}</span>
            {#if searching}<span class="where muted">{locationOf(library, rec)}</span>{/if}
            <span class="date muted">{formatModified(rec.modified)}</span>
            <span class="go muted">
              {#if rec.kind === "folder"}<Icon name="chevron" size={16} />
              {:else if rec.id === openingId}<span class="spinning"><Icon name="spinner" size={16} /></span>{/if}
            </span>
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if connected && synced}
    <p class="foot muted">{library.length} items · last refreshed {synced}</p>
  {/if}
</main>

<style>
  main { padding: 16px 24px 24px; display: grid; gap: 12px; align-content: start; }
  .notice { background: var(--surface); border: 1px solid var(--border); border-radius: 8px; padding: 14px 16px; display: grid; gap: 8px; }
  .notice h2 { font-size: 1rem; display: flex; align-items: center; gap: 8px; }
  .notice ul { margin: 0; padding-left: 22px; }
  .toolbar { display: flex; gap: 10px; flex-wrap: wrap; align-items: center; }
  .search { position: relative; flex: 1 1 220px; display: flex; align-items: center; }
  .search-icon { position: absolute; left: 9px; display: inline-flex; pointer-events: none; }
  .search .clear { position: absolute; right: 3px; width: 28px; height: 28px; }
  input::-webkit-search-cancel-button { display: none; }
  input { flex: 1;  font: inherit; color: inherit; padding: 7px 34px; border: 1px solid var(--border); border-radius: 6px; background: var(--surface); }
  .filters { display: flex; gap: 6px; }
  .chip[aria-pressed="true"] { border-color: var(--accent); box-shadow: inset 0 0 0 1px var(--accent); background: var(--hover); }
  .spin-wrap { display: inline-flex; }
  nav { display: flex; gap: 6px; flex-wrap: wrap; align-items: center; }
  .crumb { border: 0; background: none; padding: 2px 4px; border-radius: 4px; color: var(--accent); }
  .crumb:hover { background: var(--hover); }
  .crumb.home { display: inline-flex; padding: 3px; }
  .sep { display: inline-flex; }
  .rows { list-style: none; margin: 0; padding: 0; background: var(--surface); border: 1px solid var(--border); border-radius: 8px; overflow: hidden; }
  .rows li + li { border-top: 1px solid var(--border); }
  .row { width: 100%; display: flex; align-items: center; gap: 12px; padding: 9px 14px; border: 0; background: none; text-align: left; }
  .row:hover { background: var(--hover); }
  .row[aria-disabled="true"] { cursor: default; }
  .row[aria-disabled="true"]:hover { background: none; }
  .title { flex: 1 1 auto; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .where { flex: 0 1 30%; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .date { flex: 0 0 110px; text-align: right; }
  .go { flex: 0 0 16px; display: inline-flex; }
  .empty { padding: 32px; text-align: center; }
  .foot { font-size: 0.85rem; }
</style>
