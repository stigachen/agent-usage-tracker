<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import ProviderCard from "$lib/ProviderCard.svelte";
  import Settings from "$lib/Settings.svelte";
  import type { UsageSnapshot } from "$lib/types";

  const MAX_HEIGHT = 600;
  let snaps = $state<UsageSnapshot[]>([]);
  let mainEl: HTMLElement;
  let refreshing = $state(false);
  let loaded = $state(false);
  let showSettings = $state(false);
  let groups = $derived(
    Object.values(
      snaps.reduce<Record<string, UsageSnapshot[]>>((g, s) => {
        (g[s.providerId] ??= []).push(s);
        return g;
      }, {}),
    ),
  );
  // Ticks once a minute so relative times stay fresh while the panel is open.
  let now = $state(Date.now());
  let updated = $derived(snaps[0] ? new Date(snaps[0].fetchedAt).getTime() : null);
  let updatedText = $derived.by(() => {
    if (!updated) return "";
    const m = Math.floor((now - updated) / 60_000);
    return m < 1 ? "Updated just now" : m < 60 ? `Updated ${m}m ago` : `Updated ${Math.floor(m / 60)}h ago`;
  });

  onMount(() => {
    invoke<UsageSnapshot[]>("get_snapshots").then((s) => {
      snaps = s;
      loaded = s.length > 0;
    });
    let timer: ReturnType<typeof setInterval> | undefined;
    const tick = () => (now = Date.now());
    const onVis = () => {
      clearInterval(timer);
      if (!document.hidden) {
        tick();
        timer = setInterval(tick, 60_000);
      }
    };
    onVis();
    document.addEventListener("visibilitychange", onVis);
    const un = [
      listen<UsageSnapshot[]>("usage-updated", (e) => {
        snaps = e.payload;
        loaded = true;
        refreshing = false;
        now = Date.now();
      }),
      listen("panel-shown", () => {
        refreshing = true;
        showSettings = false;
      }),
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
      clearInterval(timer);
      document.removeEventListener("visibilitychange", onVis);
      un.forEach((p) => p.then((f) => f()));
    };
  });

  function refresh() {
    refreshing = true;
    invoke("refresh");
  }
</script>

<main bind:this={mainEl}>
  {#if showSettings}
    <Settings {snaps} onclose={() => (showSettings = false)} />
  {:else}
  <div class="top">
    <h1>Agent Usage</h1>
    <div class="actions">
    <button class="icon" onclick={() => (showSettings = true)} title="Settings" aria-label="Settings">
      <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/><circle cx="12" cy="12" r="3"/></svg>
    </button>
    <button class="icon" class:spin={refreshing} onclick={refresh} title="Refresh" aria-label="Refresh">
      <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M21 12a9 9 0 1 1-3-6.7L21 8"/><path d="M21 3v5h-5"/></svg>
    </button>
    </div>
  </div>

  <div class="list">
    {#if loaded}
      {#each groups as g (g[0].providerId)}
        <ProviderCard snaps={g} {now} />
      {/each}
    {:else}
      <div class="skeleton">
        <div class="sk-row"><div class="sk sk-logo"></div><div class="sk sk-line w40"></div></div>
        <div class="sk sk-line big"></div>
        <div class="sk sk-bar"></div>
      </div>
    {/if}
  </div>

  <footer>
    <span class="muted">{updatedText}</span>
    <button class="link" onclick={() => invoke("quit")}>Quit</button>
  </footer>
  {/if}
</main>

<style>
  :global(:root) {
    --fg: #1d1d1f; --muted: #86868b; --card: rgba(255,255,255,0.55);
    --border: rgba(0,0,0,0.06); --track: rgba(0,0,0,0.08); --chip: rgba(0,0,0,0.05);
    --accent: #0a84ff; --border-strong: rgba(0,0,0,0.18);
    color-scheme: light dark;
  }
  @media (prefers-color-scheme: dark) {
    :global(:root) {
      --fg: #f5f5f7; --muted: #98989d; --card: rgba(255,255,255,0.06);
      --border: rgba(255,255,255,0.08); --track: rgba(255,255,255,0.1); --chip: rgba(255,255,255,0.08);
      --border-strong: rgba(255,255,255,0.22);
    }
  }
  :global(html, body) {
    margin: 0; height: 100%; background: transparent; color: var(--fg);
    font: 13px -apple-system, BlinkMacSystemFont, "Segoe UI Variable", "Segoe UI", sans-serif;
    -webkit-font-smoothing: antialiased; user-select: none; cursor: default; overflow: hidden;
  }
  main { max-height: 600px; box-sizing: border-box; padding: 14px; display: flex; flex-direction: column; gap: 12px; }
  .actions { display: flex; gap: 6px; }
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
  footer { padding: 0 2px; }
  .skeleton {
    background: var(--card); border: 1px solid var(--border); border-radius: 14px;
    padding: 14px; display: flex; flex-direction: column; gap: 14px;
  }
  .sk-row { display: flex; align-items: center; gap: 10px; }
  .sk {
    border-radius: 6px;
    background: linear-gradient(90deg, var(--track) 0%, var(--chip) 50%, var(--track) 100%);
    background-size: 200% 100%; animation: shimmer 1.2s ease-in-out infinite;
  }
  .sk-logo { width: 30px; height: 30px; border-radius: 9px; }
  .sk-line { height: 10px; }
  .sk-line.big { height: 28px; width: 35%; }
  .w40 { width: 40%; }
  .sk-bar { height: 8px; border-radius: 99px; }
  @keyframes shimmer { from { background-position: 200% 0; } to { background-position: -200% 0; } }
</style>
