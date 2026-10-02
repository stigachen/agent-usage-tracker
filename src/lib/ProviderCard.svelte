<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { DeviceCode, UsageSnapshot } from "./types";

  let { snap }: { snap: UsageSnapshot } = $props();
  let login = $state<DeviceCode | null>(null);
  let loginError = $state<string | null>(null);

  const fmt = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });

  function resetIn(iso: string | null): string {
    if (!iso) return "";
    const ms = new Date(iso).getTime() - Date.now();
    if (ms <= 0) return "Resets soon";
    const d = Math.floor(ms / 86_400_000);
    const h = Math.floor((ms % 86_400_000) / 3_600_000);
    return d > 0 ? `Resets in ${d}d ${h}h` : `Resets in ${h}h`;
  }

  const prettyPlan = (p: string) => p.replace(/_/g, " ");

  function tone(r: number) {
    return r >= 0.9 ? "danger" : r >= 0.7 ? "warn" : "ok";
  }

  async function signIn() {
    loginError = null;
    try {
      login = await invoke<DeviceCode>("start_login", { provider: snap.providerId });
      await navigator.clipboard.writeText(login.userCode).catch(() => {});
      await invoke("open_url", { url: login.verificationUri });
      await invoke("finish_login", { provider: snap.providerId, code: login });
    } catch (e) {
      loginError = String(e);
    } finally {
      login = null;
    }
  }
</script>

<section class="card">
  <header>
    <div class="logo">
      <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
        <path fill="currentColor" d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8"/>
      </svg>
    </div>
    <div class="title">
      <h2>{snap.providerName}</h2>
      {#if snap.account}<span class="sub">@{snap.account}{snap.plan ? ` · ${prettyPlan(snap.plan)}` : ""}</span>{/if}
    </div>
  </header>

  {#if snap.needsAuth}
    {#if login}
      <div class="login">
        <p>Enter this code on GitHub (copied):</p>
        <code>{login.userCode}</code>
        <p class="muted">Waiting for authorization…</p>
      </div>
    {:else}
      <button class="primary" onclick={signIn}>Sign in with GitHub</button>
    {/if}
    {#if loginError}<p class="error">{loginError}</p>{/if}
  {:else if snap.error}
    <p class="error">{snap.error}</p>
  {:else}
    {#each snap.windows as w (w.label)}
      <div class="quota">
        <div class="row">
          <span class="label">{w.label}</span>
          {#if w.limit}
            <span class="value">{fmt.format(w.used)} <span class="muted">/ {fmt.format(w.limit)}</span></span>
          {:else}
            <span class="value muted">Unlimited</span>
          {/if}
        </div>
        {#if w.limit}
          {@const r = Math.min(w.used / w.limit, 1)}
          <div class="bar"><div class="fill {tone(r)}" style:width="{Math.max(r * 100, 1.5)}%"></div></div>
          <div class="row foot">
            <span class="muted">{(100 - r * 100).toFixed(1)}% left</span>
            <span class="muted">{resetIn(w.resetsAt)}</span>
          </div>
        {/if}
      </div>
    {/each}
  {/if}
</section>

<style>
  .card {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 12px;
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  header { display: flex; align-items: center; gap: 10px; }
  .logo {
    width: 28px; height: 28px; border-radius: 8px;
    display: grid; place-items: center;
    background: var(--chip);
  }
  h2 { margin: 0; font-size: 13px; font-weight: 600; }
  .title { display: flex; flex-direction: column; }
  .sub { font-size: 11px; color: var(--muted); text-transform: capitalize; }
  .quota { display: flex; flex-direction: column; gap: 6px; }
  .row { display: flex; justify-content: space-between; align-items: baseline; }
  .label { font-size: 12px; font-weight: 500; }
  .value { font-size: 12px; font-variant-numeric: tabular-nums; }
  .foot { font-size: 11px; }
  .muted { color: var(--muted); }
  .bar { height: 6px; border-radius: 99px; background: var(--track); overflow: hidden; }
  .fill { height: 100%; border-radius: 99px; transition: width 0.4s ease; }
  .ok { background: linear-gradient(90deg, #34c759, #30d158); }
  .warn { background: linear-gradient(90deg, #ff9f0a, #ffb340); }
  .danger { background: linear-gradient(90deg, #ff453a, #ff6961); }
  .primary {
    border: 0; border-radius: 8px; padding: 8px;
    background: var(--accent); color: white; font-weight: 500; cursor: pointer;
  }
  .login { text-align: center; font-size: 12px; }
  .login p { margin: 4px 0; }
  code { font-size: 20px; letter-spacing: 3px; font-weight: 600; user-select: text; }
  .error { color: #ff453a; font-size: 12px; margin: 0; }
</style>
