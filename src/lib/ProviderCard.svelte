<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { fly } from "svelte/transition";
  import ProviderIcon from "./ProviderIcon.svelte";
  import type { DeviceCode, UsageSnapshot } from "./types";

  let { snaps, now }: { snaps: UsageSnapshot[]; now: number } = $props();
  let first = $derived(snaps[0]);
  // Placeholder snapshot (no account) means the provider has no accounts yet.
  let accounts = $derived(snaps.filter((s) => s.accountId));
  let login = $state<DeviceCode | null>(null);
  let loginError = $state<string | null>(null);
  let copied = $state(false);
  let confirming = $state<string | null>(null);

  const fmt = new Intl.NumberFormat(undefined, { maximumFractionDigits: 0 });
  // Providers that only report percentages use limit = 100.
  const isPct = (w: { limit: number | null }) => w.limit === 100;
  const prettyPlan = (p: string) => p.replace(/_/g, " ");

  function resetIn(iso: string | null): string {
    if (!iso) return "";
    const ms = new Date(iso).getTime() - now;
    if (ms <= 0) return "Resets soon";
    const d = Math.floor(ms / 86_400_000);
    const h = Math.floor((ms % 86_400_000) / 3_600_000);
    return d > 0 ? `Resets in ${d}d ${h}h` : `Resets in ${h}h`;
  }

  const resetDate = (iso: string | null) =>
    iso ? new Date(iso).toLocaleDateString([], { month: "short", day: "numeric" }) : "";

  function quotas(snap: UsageSnapshot) {
    const limited = snap.windows.filter((w) => w.limit);
    // The headline quota is the most consumed one.
    const hero = limited.reduce<(typeof limited)[number] | null>(
      (a, w) => (!a || w.used / w.limit! > a.used / a.limit! ? w : a),
      null,
    );
    return {
      hero,
      others: limited.filter((w) => w !== hero),
      unlimited: snap.windows.filter((w) => !w.limit),
    };
  }

  const tone = (r: number) => (r >= 0.9 ? "danger" : r >= 0.7 ? "warn" : "ok");

  async function signIn() {
    loginError = null;
    try {
      login = await invoke<DeviceCode>("start_login", { provider: first.providerId });
      await copyCode();
      await invoke("open_url", { url: login.verificationUri });
      await invoke("finish_login", { provider: first.providerId, code: login });
    } catch (e) {
      loginError = String(e);
    } finally {
      login = null;
    }
  }

  async function copyCode() {
    if (!login) return;
    await navigator.clipboard.writeText(login.userCode).catch(() => {});
    copied = true;
    setTimeout(() => (copied = false), 1500);
  }

  // First click arms the button, second click within 3s signs out.
  async function signOut(snap: UsageSnapshot) {
    const id = snap.accountId!;
    if (confirming !== id) {
      confirming = id;
      setTimeout(() => confirming === id && (confirming = null), 3000);
      return;
    }
    confirming = null;
    loginError = null;
    try {
      await invoke("logout", { provider: snap.providerId, account: id });
    } catch (e) {
      loginError = String(e);
    }
  }
</script>

<section class="card" in:fly={{ y: 6, duration: 220 }}>
  <header>
    <div class="logo"><ProviderIcon id={first.providerId} /></div>
    <div class="title">
      <h2>{first.providerName}</h2>
      {#if accounts.length > 1}<span class="sub">{accounts.length} accounts</span>{/if}
    </div>
    {#if accounts.length && !login && !first.managed}
      <button class="ghost show" onclick={signIn} title="Add account" aria-label="Add account">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
      </button>
    {/if}
  </header>

  {#if login}
    <div class="login">
      <span class="muted small">Enter this code on GitHub</span>
      <button class="code" onclick={copyCode} title="Copy">{login.userCode}</button>
      <span class="muted small">{copied ? "Copied to clipboard" : "Waiting for authorization…"}</span>
    </div>
  {:else if !accounts.length && first.loginHint}
    <div class="hint">{first.loginHint}</div>
  {:else if !accounts.length}
    <button class="primary" onclick={signIn}>Sign in with GitHub</button>
  {/if}
  {#if loginError}<p class="error">{loginError}</p>{/if}

  {#each accounts as snap (snap.accountId)}
    {@const q = quotas(snap)}
    <div class="account">
      <div class="acc-head">
        <span class="acc-name">@{snap.account}</span>
        {#if snap.plan}<span class="plan">{prettyPlan(snap.plan)}</span>{/if}
        {#if snap.managed}
          <span></span>
        {:else if confirming === snap.accountId}
          <button class="armed" onclick={() => signOut(snap)}>Sign out?</button>
        {:else}
        <button class="ghost" onclick={() => signOut(snap)} title="Sign out" aria-label="Sign out @{snap.account}">
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><path d="m16 17 5-5-5-5"/><path d="M21 12H9"/></svg>
        </button>
        {/if}
      </div>

      {#if snap.needsAuth}
        <div class="error-box"><span>Session expired</span><span class="muted small">{snap.loginHint ?? "Sign out and add the account again."}</span></div>
      {:else if snap.error}
        <div class="error-box">
          <span>Couldn't load usage</span>
          <span class="muted small">{snap.error}</span>
        </div>
      {:else}
        {#if snap.note}
          <div class="hint">
            {snap.note}
            {#if snap.periodEndsAt}<br /><span class="small">Period {resetIn(snap.periodEndsAt).toLowerCase()}</span>{/if}
          </div>
        {/if}
        {#if q.hero}
          {@const r = Math.min(q.hero.used / q.hero.limit!, 1)}
          <div class="hero">
            <div class="hero-top">
              <div>
                <div class="big {tone(r)}-text">{Math.floor(100 - r * 100)}<span class="pct">%</span></div>
                <div class="muted small">{q.hero.label} left</div>
              </div>
              <div class="right">
                {#if isPct(q.hero)}
                  <div class="value">{fmt.format(q.hero.used)}%<span class="muted"> used</span></div>
                {:else}
                  <div class="value">{fmt.format(q.hero.used)}<span class="muted"> / {fmt.format(q.hero.limit!)}</span></div>
                {/if}
                <div class="muted small" title={resetDate(q.hero.resetsAt)}>{resetIn(q.hero.resetsAt)}</div>
              </div>
            </div>
            <div class="bar"><div class="fill {tone(r)}" style:width="{Math.max(r * 100, 1.5)}%"></div></div>
          </div>
        {/if}

        {#each q.others as w (w.label)}
          {@const r = Math.min(w.used / w.limit!, 1)}
          <div class="quota">
            <div class="row">
              <span class="label">{w.label}</span>
              {#if isPct(w)}
                <span class="value">{Math.floor(100 - r * 100)}%<span class="muted"> left</span></span>
              {:else}
                <span class="value">{fmt.format(w.used)}<span class="muted"> / {fmt.format(w.limit!)}</span></span>
              {/if}
            </div>
            <div class="bar thin"><div class="fill {tone(r)}" style:width="{Math.max(r * 100, 1.5)}%"></div></div>
            {#if w.resetsAt}<span class="muted small">{resetIn(w.resetsAt)}</span>{/if}
          </div>
        {/each}

        {#if q.unlimited.length}
          <div class="chips">
            {#each q.unlimited as w (w.label)}
              <span class="chip"><span class="dot"></span>{w.label}<span class="muted">∞</span></span>
            {/each}
          </div>
        {/if}
      {/if}
    </div>
  {/each}
</section>

<style>
  .card {
    background: var(--card);
    border: 1px solid var(--border);
    border-radius: 14px;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
  }
  header { display: flex; align-items: center; gap: 10px; }
  .logo {
    width: 30px; height: 30px; border-radius: 9px; flex: none;
    display: grid; place-items: center;
    background: var(--chip);
  }
  .title { display: flex; flex-direction: column; gap: 1px; flex: 1; min-width: 0; }
  h2 { margin: 0; font-size: 13px; font-weight: 600; }
  .sub { font-size: 11px; color: var(--muted); }
  .plan {
    text-transform: capitalize; font-size: 10px; font-weight: 500;
    padding: 1px 6px; border-radius: 99px; background: var(--chip); color: var(--fg);
  }
  .ghost {
    border: 0; background: none; color: var(--muted); cursor: pointer;
    width: 24px; height: 24px; border-radius: 6px; display: grid; place-items: center;
    opacity: 0; transition: opacity 0.15s, background 0.15s;
  }
  .account:hover .ghost, .ghost.show { opacity: 1; }
  .ghost:hover { background: var(--chip); color: var(--fg); }

  .account { display: flex; flex-direction: column; gap: 10px; }
  .account + .account { border-top: 1px solid var(--border); padding-top: 12px; }
  .acc-head { display: flex; align-items: center; gap: 6px; font-size: 11px; color: var(--muted); }
  .acc-head .ghost { margin-left: auto; width: 20px; height: 20px; }
  .armed {
    margin-left: auto; border: 0; border-radius: 6px; padding: 2px 8px; font-size: 11px;
    font-weight: 500; background: #ff453a; color: white; cursor: pointer;
  }
  .acc-name { font-weight: 500; }
  .hero { display: flex; flex-direction: column; gap: 10px; }
  .hero-top { display: flex; justify-content: space-between; align-items: flex-end; }
  .big {
    font-size: 30px; font-weight: 650; letter-spacing: -0.03em; line-height: 1;
    font-variant-numeric: tabular-nums;
  }
  .pct { font-size: 16px; font-weight: 600; margin-left: 1px; opacity: 0.7; }
  .right { text-align: right; display: flex; flex-direction: column; gap: 3px; }
  .ok-text { color: var(--fg); }
  .warn-text { color: #ff9f0a; }
  .danger-text { color: #ff453a; }

  .quota { display: flex; flex-direction: column; gap: 6px; }
  .row { display: flex; justify-content: space-between; align-items: baseline; }
  .label { font-size: 12px; font-weight: 500; }
  .value { font-size: 12px; font-weight: 500; font-variant-numeric: tabular-nums; }
  .small { font-size: 11px; }
  .muted { color: var(--muted); }

  .bar { height: 8px; border-radius: 99px; background: var(--track); overflow: hidden; }
  .bar.thin { height: 5px; }
  .fill { height: 100%; border-radius: 99px; transition: width 0.6s cubic-bezier(0.22, 1, 0.36, 1); }
  .ok { background: linear-gradient(90deg, #30d158, #34c759); }
  .warn { background: linear-gradient(90deg, #ff9f0a, #ffb340); }
  .danger { background: linear-gradient(90deg, #ff453a, #ff6961); }

  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .chip {
    display: inline-flex; align-items: center; gap: 5px;
    font-size: 11px; padding: 3px 8px; border-radius: 99px; background: var(--chip);
  }
  .dot { width: 5px; height: 5px; border-radius: 50%; background: #30d158; }

  .primary {
    border: 0; border-radius: 9px; padding: 8px; font-size: 12px;
    background: var(--accent); color: white; font-weight: 500; cursor: pointer;
    transition: filter 0.15s;
  }
  .primary:hover { filter: brightness(1.08); }
  .login { display: flex; flex-direction: column; align-items: center; gap: 6px; }
  .code {
    border: 1px dashed var(--border-strong); background: var(--chip); color: var(--fg);
    border-radius: 9px; padding: 6px 14px; cursor: pointer;
    font: 600 20px ui-monospace, SFMono-Regular, Menlo, monospace; letter-spacing: 3px;
  }
  .hint {
    font-size: 12px; color: var(--muted); padding: 8px 10px; border-radius: 9px; background: var(--chip);
  }
  .error { color: #ff453a; font-size: 12px; margin: 0; }
  .error-box {
    display: flex; flex-direction: column; gap: 2px; font-size: 12px;
    padding: 8px 10px; border-radius: 9px; background: rgba(255, 69, 58, 0.1); color: #ff453a;
  }
</style>
