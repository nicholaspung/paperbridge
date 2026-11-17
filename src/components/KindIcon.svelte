<script lang="ts">
  import type { DocKind } from "../lib/bindings/DocKind";
  import { KIND_LABEL } from "../lib/library";
  import Icon from "./Icon.svelte";

  let { kind, size = 22 }: { kind: DocKind; size?: number } = $props();
</script>

<span class="kind-icon {kind}" title={KIND_LABEL[kind]}><Icon name={kind} {size} /></span>

<style>
  .kind-icon { display: inline-flex; color: var(--muted); flex: none; }
  .folder { color: var(--accent); }
  .notebook { color: var(--kind-notebook); }
  .pdf { color: var(--kind-pdf); }
  .epub { color: var(--kind-epub); }
</style>
