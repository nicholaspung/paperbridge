<script lang="ts">
  import { onDestroy, onMount, type Snippet } from "svelte";
  import Icon from "../components/Icon.svelte";
  import * as ipc from "../lib/ipc";
  import type { Book, TocItem } from "../lib/epub";

  let { id, title, onClose, onExternal, switcher }: {
    id: string;
    title: string;
    onClose: () => void;
    onExternal: () => void;
    switcher?: Snippet;
  } = $props();

  const posKey = () => `pb-epub-pos:${id}`;
  const SIZE_KEY = "pb-epub-font";

  let host: HTMLDivElement | undefined = $state();
  let book: Book | null = null;
  let toc = $state<TocItem[]>([]);
  let error = $state<string | null>(null);
  let ready = $state(false);
  let fraction = $state(0);
  let chapter = $state<string | undefined>();
  let fontSize = $state(load(SIZE_KEY, 100));
  let loadedSections = $state(0);

  function load(key: string, fallback: number): number {
    try {
      const v = Number(localStorage.getItem(key));
      return v > 0 ? v : fallback;
    } catch {
      return fallback;
    }
  }
  function save(key: string, value: string) {
    try {
      localStorage.setItem(key, value);
    } catch {
      /* private mode or blocked storage: positions are a convenience */
    }
  }

  // The book's pages live in iframes and cannot see the app's CSS variables.
  function css(): string {
    const v = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return `
      html { color-scheme: light dark; color: ${v("--text")}; background: transparent !important; font-size: ${fontSize}% !important; }
      body { background: transparent !important; }
      a:link, a:visited { color: ${v("--accent")}; }
      p, li, blockquote, dd { line-height: 1.5; hyphens: auto; -webkit-hyphens: auto; widows: 2; orphans: 2; }
      pre { white-space: pre-wrap !important; }
      img, svg { max-width: 100%; height: auto; }
    `;
  }

  onMount(async () => {
    try {
      const [{ openBook }, bytes] = await Promise.all([import("../lib/epub"), ipc.getDocumentFile(id, "epub")]);
      let last: string | null = null;
      try {
        last = localStorage.getItem(posKey());
      } catch {
        /* ignore */
      }
      book = await openBook(host!, bytes, {
        lastLocation: last,
        onRelocate: (r) => {
          fraction = r.fraction;
          chapter = r.chapter;
          if (r.cfi) save(posKey(), r.cfi);
        },
        onLoad: (doc) => {
          loadedSections += 1;
          doc.addEventListener("keydown", onKey);
        },
      });
      book.setStyles(css());
      toc = book.toc;
      ready = true;
    } catch (e) {
      error = `Could not show this book: ${e instanceof Error ? e.message : String(e)}`;
    }
  });

  onDestroy(() => book?.destroy());

  $effect(() => {
    void fontSize;
    if (ready) book?.setStyles(css());
  });

  function font(step: number) {
    fontSize = Math.min(200, Math.max(70, fontSize + step));
    save(SIZE_KEY, String(fontSize));
  }

  function onKey(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return;
    if (e.key === "Escape") onClose();
    else if (e.key === "ArrowRight" || e.key === "PageDown" || e.key === " ") {
      e.preventDefault();
      void book?.next();
    } else if (e.key === "ArrowLeft" || e.key === "PageUp") {
      e.preventDefault();
      void book?.prev();
    }
  }

  function jump(e: Event) {
    const href = (e.target as HTMLSelectElement).value;
    if (href) void book?.goTo(href);
  }
</script>

<svelte:window onkeydown={onKey} />

<section class="reader" data-testid="epub-reader" data-ready={ready} data-sections={loadedSections}>
  <div class="bar">
    <button class="icon-btn plain" data-testid="viewer-close" aria-label="Back to the library" title="Back to the library (Esc)" onclick={onClose}>
      <Icon name="back" size={20} />
    </button>
    <strong class="title" {title}>{title}</strong>
    {#if ready}
      <span class="muted where" data-testid="epub-progress">{Math.round(fraction * 100)}%{chapter ? ` · ${chapter}` : ""}</span>
    {/if}
    <span class="spacer"></span>
    {#if toc.length}
      <select class="toc" aria-label="Contents" title="Contents" onchange={jump}>
        <option value="">Contents</option>
        {#each toc as item (item.href + item.depth)}
          <option value={item.href}>{" ".repeat(item.depth)}{item.label}</option>
        {/each}
      </select>
    {/if}
    {#if ready}
      <button class="icon-btn plain" aria-label="Smaller text" title="Smaller text" onclick={() => font(-10)} disabled={fontSize <= 70}>
        <span class="aa small">A</span>
      </button>
      <button class="icon-btn plain" aria-label="Larger text" title="Larger text" onclick={() => font(10)} disabled={fontSize >= 200}>
        <span class="aa">A</span>
      </button>
    {/if}
    {@render switcher?.()}
    <button class="icon-btn plain" data-testid="viewer-external" aria-label="Open in the default book app" title="Open in the default book app" onclick={onExternal}>
      <Icon name="external" size={18} />
    </button>
  </div>

  <div class="stage">
    <button class="turn" aria-label="Previous page" title="Previous page (←)" onclick={() => book?.prev()} disabled={!ready}>
      <Icon name="back" size={22} />
    </button>
    <div class="host" bind:this={host}>
      {#if error}
        <p class="msg" role="alert">{error}</p>
      {:else if !ready}
        <p class="msg muted"><span class="spinning"><Icon name="spinner" size={20} /></span></p>
      {/if}
    </div>
    <button class="turn" data-testid="epub-next" aria-label="Next page" title="Next page (→)" onclick={() => book?.next()} disabled={!ready}>
      <Icon name="chevron" size={22} />
    </button>
  </div>
</section>

<style>
  .reader { height: 100%; display: flex; flex-direction: column; min-height: 0; }
  .bar { display: flex; align-items: center; gap: 6px; padding: 6px 12px; border-bottom: 1px solid var(--border); background: var(--surface); }
  .title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-left: 4px; }
  .where { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0; margin-left: 8px; font-size: 0.9rem; }
  .spacer { flex: 1; }
  .toc { max-width: 180px; font: inherit; font-size: 0.9rem; color: inherit; background: var(--surface); border: 1px solid var(--border); border-radius: 6px; padding: 4px 6px; }
  .aa { font-weight: 600; font-size: 1.05rem; line-height: 1; }
  .aa.small { font-size: 0.8rem; }
  .stage { flex: 1; min-height: 0; display: flex; align-items: stretch; background: var(--bg); }
  .host { flex: 1; min-width: 0; position: relative; display: flex; }
  .turn { flex: none; width: 44px; border: 0; background: none; color: var(--muted); display: flex; align-items: center; justify-content: center; }
  .turn:hover:not(:disabled) { background: var(--hover); color: var(--text); }
  .turn:disabled { opacity: 0.3; cursor: default; }
  .msg { margin: auto; }
</style>
