<script lang="ts" module>
  // Stroke icons on a 24×24 grid. Each entry is a list of path `d` strings.
  const PATHS = {
    folder: ["M3 6.5A1.5 1.5 0 0 1 4.5 5h4.6l2 2.2h8.4A1.5 1.5 0 0 1 21 8.7v8.8a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z"],
    notebook: [
      "M13.4 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-7.4",
      "M2 6h4M2 10h4M2 14h4M2 18h4",
      "M21.4 5.6a2.1 2.1 0 1 0-3-3l-5 5a2 2 0 0 0-.5.9l-.8 2.8a.5.5 0 0 0 .6.6l2.8-.8a2 2 0 0 0 .9-.5z",
    ],
    pdf: ["M6 3h8l4 4v13a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z", "M14 3v4h4", "M8.5 12h7M8.5 15.5h7M8.5 8.5h3"],
    epub: [
      "M12 7v14",
      "M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4 4 4 0 0 1 4-4h5a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1h-6a3 3 0 0 0-3 3 3 3 0 0 0-3-3z",
    ],
    unknown: ["M6 3h8l4 4v13a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1z", "M14 3v4h4", "M10 12.5a2 2 0 1 1 2.8 1.8c-.5.3-.8.6-.8 1.2M12 18v.1"],
    tablet: ["M6 2h12a2 2 0 0 1 2 2v16a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2z", "M11 18h2"],
    refresh: ["M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8", "M21 3v5h-5", "M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16", "M8 16H3v5"],
    search: ["M11 3a8 8 0 1 0 0 16 8 8 0 0 0 0-16z", "M21 21l-4.3-4.3"],
    close: ["M18 6 6 18", "M6 6l12 12"],
    chevron: ["M9 18l6-6-6-6"],
    plug: ["M12 22v-5", "M9 8V2", "M15 8V2", "M18 8v5a4 4 0 0 1-4 4h-4a4 4 0 0 1-4-4V8z"],
    shield: ["M20 13c0 5-3.5 7.5-7.7 9a1 1 0 0 1-.6 0C7.5 20.5 4 18 4 13V6a1 1 0 0 1 1-1c2 0 4.5-1.2 6.2-2.7a1.2 1.2 0 0 1 1.6 0C14.5 3.8 17 5 19 5a1 1 0 0 1 1 1z"],
    alert: ["M12 8v4", "M12 16h.01"],
    alertCircle: ["M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18z", "M12 8v4", "M12 16h.01"],
    check: ["M20 6 9 17l-5-5"],
    spinner: ["M21 12a9 9 0 1 1-6.2-8.6"],
    back: ["M15 18l-6-6 6-6"],
    zoomIn: ["M11 3a8 8 0 1 0 0 16 8 8 0 0 0 0-16z", "M21 21l-4.3-4.3", "M11 8v6", "M8 11h6"],
    zoomOut: ["M11 3a8 8 0 1 0 0 16 8 8 0 0 0 0-16z", "M21 21l-4.3-4.3", "M8 11h6"],
    fitWidth: ["M3 5v14", "M21 5v14", "M7 12h10", "M10 9l-3 3 3 3", "M14 9l3 3-3 3"],
    external: ["M15 3h6v6", "M10 14 21 3", "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"],
    download: ["M12 3v12", "M7 10l5 5 5-5", "M5 21h14"],
  } as const;
  export type IconName = keyof typeof PATHS;
</script>

<script lang="ts">
  let { name, size = 20, badge }: { name: IconName; size?: number; badge?: IconName } = $props();
</script>

<svg viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" class="icon {name}">
  {#each PATHS[name] as d}<path {d} />{/each}
  {#if badge}{#each PATHS[badge] as d}<path {d} />{/each}{/if}
</svg>

<style>
  svg { fill: none; stroke: currentColor; stroke-width: 1.6; stroke-linejoin: round; stroke-linecap: round; flex: none; display: block; }
</style>
