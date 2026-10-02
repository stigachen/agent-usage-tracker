<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import type { TrayDisplay, UsageSnapshot } from "./types";

  let { snaps, onclose }: { snaps: UsageSnapshot[]; onclose: () => void } = $props();

  let tray = $state("lowest");
  let refreshSecs = $state(600);
  let autostart = $state(false);
  let confirming = $state<string | null>(null);
  let error = $state<string | null>(null);

  let accounts = $derived(snaps.filter((s) => s.accountId));
  const key = (s: UsageSnapshot) => `${s.providerId}:${s.accountId}`;

  const intervals = [
    [60, "1 min"],
    [300, "5 min"],
    [600, "10 min"],
    [1800, "30 min"],
    [3600, "1 hour"],
  ] as const;

  onMount(async () => {
    const [d, secs, auto] = await Promise.all([
      invoke<TrayDisplay>("get_tray_display"),
      invoke<number>("get_refresh_secs"),
      isEnabled().catch(() => false),
    ]);
    tray = d.mode === "pinned" ? `${d.provider}:${d.account}` : d.mode;
    refreshSecs = secs;
    autostart = auto;
  });

  async function run(f: () => Promise<unknown>) {
    error = null;
    try {
      await f();
    } catch (e) {
      error = String(e);
    }
  }

  function setTray(v: string) {
    tray = v;
    let display: TrayDisplay;
    if (v === "lowest" || v === "iconOnly") display = { mode: v };
    else {
      const i = v.indexOf(":");
      display = { mode: "pinned", provider: v.slice(0, i), account: v.slice(i + 1) };
    }
    run(() => invoke("set_tray_display", { display }));
  }

  function setRefresh(v: number) {
    refreshSecs = v;
    run(() => invoke("set_refresh_secs", { secs: v }));
  }

  function toggleAutostart() {
    const next = !autostart;
    run(async () => {
      await (next ? enable() : disable());
      autostart = next;
    });
  }

  function signOut(s: UsageSnapshot) {
    const k = key(s);
    if (confirming !== k) {
      confirming = k;
      setTimeout(() => confirming === k && (confirming = null), 3000);
      return;
    }
    confirming = null;
    run(() => invoke("logout", { provider: s.providerId, account: s.accountId }));
  }
</script>

<div class="settings" in:fly={{ x: 12, duration: 180 }}>
  <div class="top">
    <button class="back" onclick={onclose} aria-label="Back">
      <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="m15 18-6-6 6-6"/></svg>
    </button>
    <h1>Settings</h1>
  </div>

  <section>
    <h3>Menu bar</h3>
    <div class="group">
      <label class="row">
        <span>Show</span>
        <select value={tray} onchange={(e) => setTray(e.currentTarget.value)}>
          <option value="lowest">Lowest remaining</option>
          {#each accounts as s (key(s))}
            <option value={key(s)}>{s.providerName} · @{s.account}</option>
          {/each}
          <option value="iconOnly">Icon only</option>
        </select>
      </label>
    </div>
  </section>

  <section>
    <h3>General</h3>
    <div class="group">
      <label class="row">
        <span>Refresh every</span>
        <select value={refreshSecs} onchange={(e) => setRefresh(+e.currentTarget.value)}>
          {#each intervals as [v, l] (v)}
            <option value={v}>{l}</option>
          {/each}
        </select>
      </label>
      <div class="row">
        <span>Launch at login</span>
        <button class="switch" class:on={autostart} onclick={toggleAutostart} role="switch" aria-checked={autostart} aria-label="Launch at login">
          <span class="knob"></span>
        </button>
      </div>
    </div>
  </section>

  <section>
    <h3>Accounts</h3>
    <div class="group">
      {#each accounts as s (key(s))}
        <div class="row">
          <span class="acc">
            <span>@{s.account}</span>
            <span class="muted">{s.providerName}</span>
          </span>
          <button class="danger" class:armed={confirming === key(s)} onclick={() => signOut(s)}>
            {confirming === key(s) ? "Sign out?" : "Sign out"}
          </button>
        </div>
      {:else}
        <div class="row muted">No accounts yet</div>
      {/each}
    </div>
  </section>

  {#if error}<p class="error">{error}</p>{/if}
</div>

<style>
  .settings { display: flex; flex-direction: column; gap: 14px; }
  .top { display: flex; align-items: center; gap: 8px; }
  h1 { margin: 0; font-size: 15px; font-weight: 650; letter-spacing: -0.01em; }
  .back {
    border: 0; background: var(--chip); color: var(--fg); width: 26px; height: 26px;
    border-radius: 7px; display: grid; place-items: center; cursor: pointer;
  }
  .back:hover { background: var(--track); }
  section { display: flex; flex-direction: column; gap: 6px; }
  h3 {
    margin: 0 4px; font-size: 11px; font-weight: 600; color: var(--muted);
    text-transform: uppercase; letter-spacing: 0.04em;
  }
  .group {
    background: var(--card); border: 1px solid var(--border); border-radius: 12px; overflow: hidden;
  }
  .row {
    display: flex; justify-content: space-between; align-items: center; gap: 10px;
    padding: 9px 12px; font-size: 12px; min-height: 22px;
  }
  .row + .row { border-top: 1px solid var(--border); }
  .acc { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .acc .muted { font-size: 11px; }
  .muted { color: var(--muted); }
  select {
    font: inherit; font-size: 12px; color: var(--fg); background: var(--chip);
    border: 0; border-radius: 6px; padding: 3px 6px; max-width: 190px; cursor: pointer;
  }
  .switch {
    position: relative; width: 32px; height: 19px; border: 0; border-radius: 99px; flex: none;
    background: var(--track); cursor: pointer; transition: background 0.2s; padding: 0;
  }
  .switch.on { background: #30d158; }
  .knob {
    position: absolute; top: 2px; left: 2px; width: 15px; height: 15px; border-radius: 50%;
    background: white; box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25); transition: transform 0.2s;
  }
  .switch.on .knob { transform: translateX(13px); }
  .danger {
    border: 0; border-radius: 6px; padding: 4px 9px; font-size: 11px; font-weight: 500;
    background: var(--chip); color: #ff453a; cursor: pointer; transition: all 0.15s;
  }
  .danger.armed { background: #ff453a; color: white; }
  .error { color: #ff453a; font-size: 12px; margin: 0 4px; }
</style>
