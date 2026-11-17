<script lang="ts">
  import type { Job } from "../lib/bindings/Job";
  import Icon from "./Icon.svelte";

  let { job, refused, onCancel, onDismiss }: {
    job: Job | null;
    refused: string | null;
    onCancel: () => void;
    onDismiss: () => void;
  } = $props();

  const phase = $derived(job?.phase.phase);
  const active = $derived(phase === "waiting" || phase === "rendering" || phase === "receiving" || phase === "opening");
  let now = $state(Date.now());

  $effect(() => {
    if (!active) return;
    const t = setInterval(() => (now = Date.now()), 1000);
    return () => clearInterval(t);
  });

  // Success and cancellation clear themselves; failures stay until dismissed.
  $effect(() => {
    if (phase !== "done" && phase !== "cancelled") return;
    const t = setTimeout(onDismiss, 5000);
    return () => clearTimeout(t);
  });

  function elapsed(): string {
    const s = Math.max(0, Math.floor((now - (job?.startedMs ?? now)) / 1000));
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
  }
  function size(bytes: number): string {
    return bytes < 1 << 20 ? `${Math.round(bytes / 1024)} KB` : `${(bytes / (1 << 20)).toFixed(1)} MB`;
  }
</script>

{#if refused || job}
  <div class="bar {phase ?? 'failed'}" role="status" data-testid="job-bar" data-phase={refused ? "refused" : phase}>
    <span class="lead">
      {#if refused || phase === "failed"}
        <Icon name="alertCircle" size={20} />
      {:else if phase === "done"}
        <Icon name="check" size={20} />
      {:else if phase === "receiving"}
        <Icon name="download" size={20} />
      {:else if phase === "cancelled"}
        <Icon name="close" size={20} />
      {:else}
        <span class="spinning"><Icon name="spinner" size={20} /></span>
      {/if}
    </span>

    <div class="text">
      {#if refused}
        <span>{refused}</span>
      {:else if job}
        {@const t = `“${job.title}”`}
        {#if job.phase.phase === "waiting"}
          <span>Waiting for the library refresh to finish before preparing {t}…</span>
        {:else if job.phase.phase === "rendering"}
          <span>The tablet is preparing {t} as a PDF <span class="muted">· {elapsed()}</span></span>
          <span class="muted small">It renders the whole document before sending anything, so large notebooks can take a few minutes.</span>
        {:else if job.phase.phase === "receiving"}
          <span>Receiving {t} <span class="muted">· {size(job.phase.bytes)}</span></span>
        {:else if job.phase.phase === "opening"}
          <span>Opening {t}…</span>
        {:else if job.phase.phase === "done"}
          <span>Opened {t}{#if job.fromCache}<span class="muted"> from the copy on this computer</span>{/if}</span>
        {:else if job.phase.phase === "failed"}
          <span>Could not open {t}. {job.phase.message}</span>
        {:else}
          <span>Cancelled opening {t}.</span>
        {/if}
      {/if}
    </div>

    {#if active}
      {#if phase !== "opening"}
        <button class="icon-btn plain" data-testid="job-cancel" aria-label="Cancel" title="Cancel" onclick={onCancel}>
          <Icon name="close" size={18} />
        </button>
      {/if}
    {:else}
      <button class="icon-btn plain" data-testid="job-dismiss" aria-label="Dismiss" title="Dismiss" onclick={onDismiss}>
        <Icon name="close" size={18} />
      </button>
    {/if}
  </div>
{/if}

<style>
  .bar {
    display: flex; align-items: center; gap: 12px; padding: 10px 12px 10px 16px;
    border-top: 1px solid var(--border); background: var(--surface);
  }
  .lead { display: inline-flex; color: var(--accent); }
  .done .lead { color: var(--ok); }
  .failed .lead { color: var(--problem); }
  .cancelled .lead { color: var(--muted); }
  .text { flex: 1; min-width: 0; display: grid; gap: 2px; }
  .small { font-size: 0.85rem; }
</style>
