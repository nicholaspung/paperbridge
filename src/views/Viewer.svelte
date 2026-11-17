<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import Icon from "../components/Icon.svelte";
  import * as ipc from "../lib/ipc";
  import type { PDFDocumentProxy } from "../lib/pdf";

  let { id, title, onClose, onExternal }: {
    id: string;
    title: string;
    onClose: () => void;
    onExternal: () => void;
  } = $props();

  const MIN = 0.25;
  const MAX = 4;
  const GAP = 16;

  let doc = $state.raw<PDFDocumentProxy | null>(null);
  let error = $state<string | null>(null);
  /** Page sizes in PDF points (scale 1). */
  let sizes = $state<{ w: number; h: number }[]>([]);
  let fit = $state(true);
  let manualScale = $state(1);
  let width = $state(0);
  let current = $state(1);
  let scroller: HTMLDivElement | undefined = $state();

  const widest = $derived(sizes.reduce((m, s) => Math.max(m, s.w), 1));
  // Fit the width, but do not blow a page up past twice its size.
  const fitScale = $derived(Math.min(2, Math.max(MIN, (width - 2 * GAP) / widest)));
  const scale = $derived(fit ? fitScale : manualScale);

  // Pages render lazily as they come near the viewport, and again at a new zoom.
  const pageEls: HTMLDivElement[] = [];
  const visible = new Set<number>();
  const renderedAt = new Map<number, number>();
  const tasks = new Map<number, { cancel(): void }>();
  let observer: IntersectionObserver | undefined;
  let release: (() => void) | undefined;

  onMount(async () => {
    try {
      const [{ loadPdf }, bytes] = await Promise.all([import("../lib/pdf"), ipc.getDocumentPdf(id)]);
      const { doc: loaded, destroy } = await loadPdf(bytes);
      release = destroy;
      const pages = await Promise.all(
        Array.from({ length: loaded.numPages }, async (_, i) => {
          const v = (await loaded.getPage(i + 1)).getViewport({ scale: 1 });
          return { w: v.width, h: v.height };
        }),
      );
      doc = loaded;
      sizes = pages;
    } catch (e) {
      error = `Could not show this PDF: ${e instanceof Error ? e.message : String(e)}`;
    }
  });

  onDestroy(() => {
    observer?.disconnect();
    for (const t of tasks.values()) t.cancel();
    release?.();
  });

  $effect(() => {
    if (!scroller) return;
    let frame = 0;
    const ro = new ResizeObserver(() => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => (width = scroller!.clientWidth));
    });
    ro.observe(scroller);
    width = scroller.clientWidth;
    return () => {
      cancelAnimationFrame(frame);
      ro.disconnect();
    };
  });

  async function render(n: number) {
    const el = pageEls[n - 1];
    if (!doc || !el || renderedAt.get(n) === scale) return;
    tasks.get(n)?.cancel();
    const target = scale;
    renderedAt.set(n, target);
    const page = await doc.getPage(n);
    const dpr = window.devicePixelRatio || 1;
    const viewport = page.getViewport({ scale: target * dpr });
    const canvas = document.createElement("canvas");
    canvas.width = Math.floor(viewport.width);
    canvas.height = Math.floor(viewport.height);
    const task = page.render({ canvas, viewport });
    tasks.set(n, task);
    try {
      await task.promise;
    } catch {
      return; // cancelled by a newer render
    }
    if (renderedAt.get(n) !== target) return;
    el.replaceChildren(canvas);
    el.dataset.rendered = "true";
  }

  function lazy(el: HTMLDivElement, n: number) {
    pageEls[n - 1] = el;
    observer ??= new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          const p = Number((e.target as HTMLElement).dataset.page);
          if (e.isIntersecting) {
            visible.add(p);
            void render(p);
          } else visible.delete(p);
        }
      },
      { root: scroller, rootMargin: "800px 0px" },
    );
    observer.observe(el);
    return { destroy: () => observer?.unobserve(el) };
  }

  // A new zoom level: re-render what is on screen; the rest re-renders on approach.
  $effect(() => {
    void scale;
    for (const p of visible) void render(p);
  });

  function onScroll() {
    if (!scroller) return;
    const mid = scroller.scrollTop + scroller.clientHeight / 3;
    let n = 1;
    for (let i = 0; i < pageEls.length; i++) if (pageEls[i] && pageEls[i].offsetTop <= mid) n = i + 1;
    current = n;
  }

  function zoom(factor: number) {
    manualScale = Math.min(MAX, Math.max(MIN, scale * factor));
    fit = false;
  }

  function onKey(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement) return;
    if (e.key === "Escape") onClose();
    else if (e.key === "+" || e.key === "=") zoom(1.25);
    else if (e.key === "-") zoom(1 / 1.25);
  }
</script>

<svelte:window onkeydown={onKey} />

<section class="viewer" data-testid="viewer">
  <div class="bar">
    <button class="icon-btn plain" data-testid="viewer-close" aria-label="Back to the library" title="Back to the library (Esc)" onclick={onClose}>
      <Icon name="back" size={20} />
    </button>
    <strong class="title" title={title}>{title}</strong>
    {#if sizes.length}
      <span class="muted pages" data-testid="viewer-pages">{current} / {sizes.length}</span>
    {/if}
    <span class="spacer"></span>
    {#if sizes.length}
    <button class="icon-btn plain" aria-label="Zoom out" title="Zoom out (−)" onclick={() => zoom(1 / 1.25)} disabled={scale <= MIN}>
      <Icon name="zoomOut" size={18} />
    </button>
    <span class="muted zoom">{Math.round(scale * 100)}%</span>
    <button class="icon-btn plain" aria-label="Zoom in" title="Zoom in (+)" onclick={() => zoom(1.25)} disabled={scale >= MAX}>
      <Icon name="zoomIn" size={18} />
    </button>
    <button class="icon-btn plain" aria-label="Fit to width" title="Fit to width" aria-pressed={fit} onclick={() => (fit = true)}>
      <Icon name="fitWidth" size={18} />
    </button>
    {/if}
    <button class="icon-btn plain" data-testid="viewer-external" aria-label="Open in the default PDF app" title="Open in the default PDF app" onclick={onExternal}>
      <Icon name="external" size={18} />
    </button>
  </div>

  <div class="scroll" bind:this={scroller} onscroll={onScroll}>
    {#if error}
      <p class="msg" role="alert">{error}</p>
    {:else if !sizes.length}
      <p class="msg muted"><span class="spinning"><Icon name="spinner" size={20} /></span></p>
    {:else}
      {#each sizes as s, i (i)}
        <div
          class="page" data-testid="pdf-page" data-page={i + 1}
          style:width="{Math.floor(s.w * scale)}px" style:height="{Math.floor(s.h * scale)}px"
          use:lazy={i + 1}
        ></div>
      {/each}
    {/if}
  </div>
</section>

<style>
  .viewer { height: 100%; display: flex; flex-direction: column; min-height: 0; }
  .bar { display: flex; align-items: center; gap: 6px; padding: 6px 12px; border-bottom: 1px solid var(--border); background: var(--surface); }
  .title { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; margin-left: 4px; }
  .pages { white-space: nowrap; margin-left: 8px; font-variant-numeric: tabular-nums; }
  .zoom { width: 46px; text-align: center; font-variant-numeric: tabular-nums; font-size: 0.9rem; }
  .spacer { flex: 1; }
  .icon-btn[aria-pressed="true"] { color: var(--accent); }
  .scroll { flex: 1; overflow: auto; padding: 16px; display: flex; flex-direction: column; align-items: flex-start; gap: 16px; background: var(--bg); }
  .page { flex: none; margin: 0 auto; background: #fff; box-shadow: 0 1px 4px rgb(0 0 0 / 0.18); }
  .page :global(canvas) { display: block; width: 100%; height: 100%; }
  .msg { margin: auto; }
</style>
