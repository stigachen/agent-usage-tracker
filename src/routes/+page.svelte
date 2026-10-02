<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import ProviderCard from "$lib/ProviderCard.svelte";
  import type { UsageSnapshot } from "$lib/types";

  const MAX_HEIGHT = 600;
  let snaps = $state<UsageSnapshot[]>([]);
  let mainEl: HTMLElement;
  let refreshing = $state(false);
  let updated = $derived(snaps[0] ? new Date(snaps[0].fetchedAt) : null);

  onMount(() => {
    invoke<UsageSnapshot[]>("get_snapshots").then((s) => (snaps = s));
    const un = [
      listen<UsageSnapshot[]>("usage-updated", (e) => {
        snaps = e.payload;
        refreshing = false;
      }),
      listen("panel-shown", () => (refreshing = true)),
    ];
    // Fit the window to its content so there is no empty space below the cards.
    const win = getCurrentWindow();
    const ro = new ResizeObserver(() => {
      const h = Math.min(Math.ceil(mainEl.scrollHeight), MAX_HEIGHT);
      win.setSize(new LogicalSize(360, h));
    });
    ro.observe(mainEl);
    return () => {
      ro.disconnect();
      un.forEach((p) => p.then((f) => f()));
    };
  });

  function refresh() {
    refreshing = true;
    invoke("refresh");
  }
</script>

<main bind:this={mainEl}>
  <div class="top">
    <h1>Agent Usage</h1>
    <button class="icon" class:spin={refreshing} onclick={refresh} title="Refresh" aria-label="Refresh">
      <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M21 12a9 9 0 1 1-3-6.7L21 8"/><path d="M21 3v5h-5"/></svg>
    </button>
  </div>

  <div class="list">
    {#each snaps as snap (snap.providerId)}
      <ProviderCard {snap} />
    {:else}
      <p class="muted center">Loading…</p>
    {/each}
  </div>

  <footer>
    <span class="muted">{updated ? `Updated ${updated.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}` : ""}</span>
    <button class="link" onclick={() => invoke("quit")}>Quit</button>
  </footer>
</main>

<style>
  :global(:root) {
    --fg: #1d1d1f; --muted: #86868b; --card: rgba(255,255,255,0.55);
    --border: rgba(0,0,0,0.06); --track: rgba(0,0,0,0.08); --chip: rgba(0,0,0,0.05);
    --accent: #0a84ff;
    color-scheme: light dark;
  }
  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --fg: #f5f5f7; --muted: #98989d; --card: rgba(255,255,255,0.06);
      --border: rgba(255,255,255,0.08); --track: rgba(255,255,255,0.1); --chip: rgba(255,255,255,0.08);
    }
  }
  :global(html, body) {
    margin: 0; height: 100%; background: transparent; color: var(--fg);
    font: 13px -apple-system, BlinkMacSystemFont, "Segoe UI Variable", "Segoe UI", sans-serif;
    -webkit-font-smoothing: antialiased; user-select: none; cursor: default; overflow: hidden;
  }
  main { max-height: 600px; box-sizing: border-box; padding: 14px; display: flex; flex-direction: column; gap: 12px; }
  .top { display: flex; justify-content: space-between; align-items: center; }
  h1 { margin: 0; font-size: 15px; font-weight: 650; letter-spacing: -0.01em; }
  .list { overflow-y: auto; display: flex; flex-direction: column; gap: 10px; }
  .icon {
    border: 0; background: var(--chip); color: var(--fg); width: 26px; height: 26px;
    border-radius: 7px; display: grid; place-items: center; cursor: pointer;
  }
  .icon:hover { background: var(--track); }
  .spin svg { animation: spin 0.9s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  footer { display: flex; justify-content: space-between; font-size: 11px; }
  .link { border: 0; background: none; color: var(--muted); cursor: pointer; font-size: 11px; padding: 0; }
  .link:hover { color: var(--fg); }
  .muted { color: var(--muted); }
  .center { text-align: center; }
</style>
