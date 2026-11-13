<script lang="ts">
  import type { DocKind } from "../lib/bindings/DocKind";
  let { kind }: { kind: DocKind } = $props();
</script>

<svg viewBox="0 0 24 24" width="22" height="22" aria-hidden="true" class={kind}>
  {#if kind === "folder"}
    <path d="M3 6.5A1.5 1.5 0 0 1 4.5 5h4.6l2 2.2h8.4A1.5 1.5 0 0 1 21 8.7v8.8a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z" />
  {:else if kind === "notebook"}
    <path d="M6 3h11a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1H6zM6 3v18M9.5 8h5M9.5 11.5h5" />
  {:else}
    <path d="M6 3h8l4 4v13a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zM14 3v4h4" />
    {#if kind === "unknown"}<path d="M10 12.5a2 2 0 1 1 2.8 1.8c-.5.3-.8.6-.8 1.2M12 18v.1" />{/if}
  {/if}
</svg>

<style>
  svg { fill: none; stroke: var(--muted); stroke-width: 1.5; stroke-linejoin: round; stroke-linecap: round; flex: none; }
  svg.folder { stroke: var(--accent); }
</style>
