<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { invoke } from "@tauri-apps/api/core";
  import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import type { TrayDisplay, UsageSnapshot } from "./types";

  let {
    snaps,
    initialPage = "general",
    onclose,
  }: { snaps: UsageSnapshot[]; initialPage?: "general" | "accounts"; onclose: () => void } = $props();

  const pages = [
    ["general", "General"],
    ["accounts", "Accounts"],
  ] as const;
  // svelte-ignore state_referenced_locally -- only the page to open with
  let page = $state<(typeof pages)[number][0]>(initialPage);
  let tray = $state("lowest");
  let refreshSecs = $state(600);
  let autostart = $state(false);
  let confirming = $state<string | null>(null);
  let error = $state<string | null>(null);
  let version = $state("");
  let platform = $state("");
  const REPO = "https://github.com/stigachen/agent-usage-tracker";

  let accounts = $derived(snaps.filter((s) => s.accountId));
  let copilot = $derived(accounts.filter((s) => s.providerId === "copilot"));
  // Account id whose billing token field is open, and its draft value.
  let editing = $state<string | null>(null);
  let draft = $state("");
  // Account id with a billing token request in flight; its buttons are disabled meanwhile.
  let saving = $state<string | null>(null);
  let patError = $state<{ account: string; msg: string } | null>(null);
  const PAT_URL =
    "https://github.com/settings/tokens/new?scopes=user&description=Agent%20Usage%20billing";
  const key = (s: UsageSnapshot) => `${s.providerId}:${s.accountId}`;

  const intervals = [
    [60, "1 min"],
    [300, "5 min"],
    [600, "10 min"],
    [1800, "30 min"],
    [3600, "1 hour"],
  ] as const;

  onMount(async () => {
    getVersion().then((v) => (version = v));
    invoke<string>("get_platform").then((p) => (platform = p));
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

  function edit(account: string | null) {
    editing = account;
    draft = "";
    patError = null;
  }

  async function saveToken(account: string, token: string) {
    saving = account;
    patError = null;
    try {
      await invoke("set_billing_token", { account, token });
      editing = null;
      draft = "";
    } catch (e) {
      patError = { account, msg: String(e) };
    } finally {
      saving = null;
    }
  }

  function setHidden(s: UsageSnapshot, hidden: boolean) {
    run(() => invoke("set_account_hidden", { provider: s.providerId, account: s.accountId, hidden }));
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

<div class="settings" class:windows={platform === "windows"} in:fly={{ x: 12, duration: 180 }}>
  <div class="top">
    <button class="back" onclick={onclose} aria-label="Back">
      <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="m15 18-6-6 6-6"/></svg>
    </button>
    <h1>Settings</h1>
  </div>

  <div class="tabs" role="tablist">
    {#each pages as [id, name] (id)}
      <button role="tab" class="tab" class:active={page === id} aria-selected={page === id} onclick={() => (page = id)}>
        {name}{#if id === "accounts" && accounts.length}<span class="count">{accounts.length}</span>{/if}
      </button>
    {/each}
  </div>

  {#if page === "general"}
  <section>
    <h3>{platform === "windows" ? "System tray" : "Menu bar"}</h3>
    {#if platform === "windows"}
      <p class="hint">Hover over the tray icon to see the selected remaining quota.</p>
    {/if}
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
    <h3>About</h3>
    <div class="group">
      <div class="about">
        <img src="/app-icon.png" alt="" width="44" height="44" />
        <div class="about-text">
          <span class="name">Agent Usage</span>
          <span class="muted">Version {version}</span>
          <span class="muted">Usage and quota for your coding agents</span>
        </div>
      </div>
      <div class="row">
        <span class="muted">© 2026 Guang Chen</span>
        <button class="link" onclick={() => invoke("open_url", { url: REPO })}>GitHub ↗</button>
      </div>
    </div>
  </section>
  {:else}
  <section>
    <h3>Accounts</h3>
    <p class="hint">Switch off to hide an account from Overview. It stays in its own tab.</p>
    <div class="group">
      {#each accounts as s (key(s))}
        <div class="row">
          <span class="acc">
            <span>@{s.account}</span>
            <span class="muted">{s.providerName}</span>
          </span>
          <span class="actions">
            <button
              class="switch small"
              class:on={!s.hidden}
              onclick={() => setHidden(s, !s.hidden)}
              role="switch"
              aria-checked={!s.hidden}
              aria-label="Show @{s.account} in Overview"
              title={s.hidden ? "Hidden from Overview" : "Shown in Overview"}
            >
              <span class="knob"></span>
            </button>
            {#if !s.managed}
              <button class="danger" class:armed={confirming === key(s)} onclick={() => signOut(s)}>
                {confirming === key(s) ? "Sign out?" : "Sign out"}
              </button>
            {/if}
          </span>
        </div>
      {:else}
        <div class="row muted">No accounts yet</div>
      {/each}
    </div>
  </section>

  {#if copilot.length}
    <section>
      <h3>Copilot model usage</h3>
      <div class="group">
        {#each copilot as s (s.accountId)}
          <div class="row">
            <span class="acc">
              <span>@{s.account}</span>
              <span class="muted">{s.billingConfigured ? (s.billing?.error ?? "Token saved") : "Not set up"}</span>
            </span>
            {#if s.billingConfigured}
              <span class="actions">
                <button class="link" disabled={saving === s.accountId} onclick={() => edit(s.accountId)}>Replace</button>
                <button class="danger" disabled={saving === s.accountId} onclick={() => saveToken(s.accountId!, "")}>Remove</button>
              </span>
            {:else if editing !== s.accountId}
              <button class="link" onclick={() => edit(s.accountId)}>Set up</button>
            {/if}
          </div>
          {#if editing === s.accountId}
            <div class="pat">
              <span class="muted">
                Create a classic token with the <b>user</b> scope while signed in to GitHub as
                <b>@{s.account}</b>. It is stored in the system credential store and only used to read billing.
              </span>
              <button class="link" onclick={() => invoke("open_url", { url: PAT_URL })}>Create token on GitHub ↗</button>
              <div class="pat-row">
                <input type="password" placeholder="ghp_…" bind:value={draft} spellcheck="false" autocomplete="off" />
                <button class="primary" disabled={!draft.trim() || saving === s.accountId} onclick={() => saveToken(s.accountId!, draft)}>
                  {saving === s.accountId ? "Checking…" : "Save"}
                </button>
                <button class="link" disabled={saving === s.accountId} onclick={() => edit(null)}>Cancel</button>
              </div>
            </div>
          {/if}
          {#if patError?.account === s.accountId}<p class="error pat-error">{patError.msg}</p>{/if}
        {/each}
      </div>
    </section>
  {/if}
  {/if}

  {#if error}<p class="error">{error}</p>{/if}
</div>

<style>
  /* min-height: 0 lets the page shrink inside the 600px panel and scroll instead of clipping. */
  .settings { display: flex; flex-direction: column; gap: 14px; min-height: 0; overflow-y: auto; }
  .top { display: flex; align-items: center; gap: 8px; }
  h1 { margin: 0; font-size: 15px; font-weight: 650; letter-spacing: -0.01em; }
  .back {
    border: 0; background: var(--chip); color: var(--fg); width: 26px; height: 26px;
    border-radius: 7px; display: grid; place-items: center; cursor: pointer;
  }
  .tabs { display: flex; gap: 2px; padding: 3px; border-radius: 10px; background: var(--chip); }
  .tab {
    all: unset; flex: 1; display: flex; align-items: center; justify-content: center; gap: 5px;
    font-size: 11.5px; font-weight: 500; color: var(--muted); padding: 5px 6px; border-radius: 7px;
    cursor: pointer; transition: background 0.18s, color 0.18s, box-shadow 0.18s;
  }
  .tab:hover { color: var(--fg); }
  .tab.active { background: var(--tab-active); color: var(--fg); box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12); }
  .count { font-size: 10px; padding: 0 5px; border-radius: 99px; background: var(--chip); }
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
  /* WebView2 popups need an opaque surface; --chip is translucent. */
  .windows { --select-bg: #f2f2f7; }
  @media (prefers-color-scheme: dark) {
    .windows { --select-bg: #2c2c2e; }
  }
  .windows select, .windows option {
    color: var(--fg); background-color: var(--select-bg);
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
  .switch.small { width: 26px; height: 16px; }
  .switch.small .knob { width: 12px; height: 12px; }
  .switch.small.on .knob { transform: translateX(10px); }
  .hint { margin: 0 4px; font-size: 11px; color: var(--muted); }
  .danger {
    border: 0; border-radius: 6px; padding: 4px 9px; font-size: 11px; font-weight: 500;
    background: var(--chip); color: #ff453a; cursor: pointer; transition: all 0.15s;
  }
  .danger.armed { background: #ff453a; color: white; }
  .about { display: flex; align-items: center; gap: 12px; padding: 12px; }
  .about img { border-radius: 10px; }
  .about-text { display: flex; flex-direction: column; gap: 1px; font-size: 11px; }
  .name { font-size: 13px; font-weight: 600; }
  .link {
    border: 0; background: none; color: var(--accent); font-size: 12px; cursor: pointer; padding: 0;
  }
  .actions { display: flex; gap: 8px; align-items: center; }
  .pat {
    display: flex; flex-direction: column; gap: 8px; padding: 0 12px 10px; font-size: 11px;
    align-items: flex-start;
  }
  .pat-error { padding: 0 12px 10px; }
  .pat-row { display: flex; gap: 6px; align-items: center; width: 100%; }
  .pat input {
    flex: 1; min-width: 0; font: inherit; font-size: 12px; color: var(--fg); background: var(--chip);
    border: 1px solid var(--border); border-radius: 6px; padding: 4px 8px;
  }
  .primary {
    border: 0; border-radius: 6px; padding: 4px 10px; font-size: 11px; font-weight: 500;
    background: var(--accent); color: white; cursor: pointer;
  }
  .primary:disabled, .link:disabled, .danger:disabled { opacity: 0.5; cursor: default; }
  .error { color: #ff453a; font-size: 12px; margin: 0 4px; }
</style>
