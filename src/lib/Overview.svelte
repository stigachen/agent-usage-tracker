<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { flip } from "svelte/animate";
  import { fly } from "svelte/transition";
  import ProviderIcon from "./ProviderIcon.svelte";
  import { Reorder } from "./reorder.svelte";
  import type { UsageSnapshot } from "./types";

  let {
    snaps,
    more = [],
    now,
    onselect,
  }: { snaps: UsageSnapshot[]; more?: UsageSnapshot[]; now: number; onselect: (id: string) => void } = $props();

  // One row per account; providers without accounts get a placeholder row.
  let rows = $derived(
    snaps.map((s) => {
      const limited = s.windows.filter((w) => w.limit);
      const worst = limited.reduce<(typeof limited)[number] | null>(
        (a, w) => (!a || w.used / w.limit! > a.used / a.limit! ? w : a),
        null,
      );
      const left = worst ? Math.max(0, Math.floor(100 - (worst.used / worst.limit!) * 100)) : null;
      return { s, worst, left };
    }),
  );

  // Rows grouped by provider; providers are what gets reordered here.
  let groups = $derived(
    rows.reduce<{ id: string; rows: typeof rows }[]>((g, r) => {
      const last = g[g.length - 1];
      if (last?.id === r.s.providerId) last.rows.push(r);
      else g.push({ id: r.s.providerId, rows: [r] });
      return g;
    }, []),
  );
  const reorder = new Reorder((order) => invoke("set_provider_order", { order }));
  let shown = $derived(reorder.order(groups, (g) => g.id));

  const tone = (left: number) => (left <= 10 ? "danger" : left <= 30 ? "warn" : "ok");

  function resetIn(iso: string | null) {
    if (!iso) return "";
    const ms = new Date(iso).getTime() - now;
    if (ms <= 0) return "soon";
    const d = Math.floor(ms / 86_400_000);
    const h = Math.floor((ms % 86_400_000) / 3_600_000);
    return d > 0 ? `${d}d ${h}h` : `${h}h`;
  }
</script>

<div class="list" class:sortable={groups.length > 1} role="list" data-reorder-list>
  {#each shown as g, gi (g.id)}
  <div
    class="group"
    role="listitem"
    class:dragging={reorder.dragging === g.id}
    data-reorder-key={g.id}
    animate:flip={{ duration: 160 }}
  >
  {#if groups.length > 1}
    <span
      class="handle"
      role="button"
      tabindex="-1"
      aria-label="Drag to reorder"
      title="Drag to reorder"
      onpointerdown={(e) => reorder.start(e, g.id, groups.map((x) => x.id))}
    ><svg viewBox="0 0 10 16" width="8" height="13" fill="currentColor" aria-hidden="true"><circle cx="3" cy="3" r="1.4"/><circle cx="7" cy="3" r="1.4"/><circle cx="3" cy="8" r="1.4"/><circle cx="7" cy="8" r="1.4"/><circle cx="3" cy="13" r="1.4"/><circle cx="7" cy="13" r="1.4"/></svg></span>
  {/if}
  {#each g.rows as { s, worst, left }, j (s.providerId + (s.accountId ?? ""))}
    {@const i = gi + j}
    <button class="row" onclick={() => onselect(s.providerId)} in:fly={{ y: 6, duration: 200, delay: i * 30 }}>
      <div class="logo"><ProviderIcon id={s.providerId} /></div>
      <div class="mid">
        <div class="name">
          {s.providerName}
          {#if s.account}<span class="muted acc">{s.account}</span>{/if}
        </div>
        {#if s.needsAuth || !s.accountId}
          <span class="muted small">Not connected</span>
        {:else if s.error}
          <span class="err small">Couldn't load</span>
        {:else if worst && left !== null}
          <div class="bar"><div class="fill {tone(left)}" style:width="{Math.max(100 - left, 1.5)}%"></div></div>
          <span class="muted small">{worst.label}{worst.resetsAt ? ` · resets in ${resetIn(worst.resetsAt)}` : ""}</span>
        {:else if s.note}
          <span class="muted small">No usage data</span>
        {:else}
          <span class="muted small">Unlimited</span>
        {/if}
      </div>
      {#if left !== null && !s.needsAuth && !s.error}
        <div class="pct {tone(left)}-text">{left}<span>%</span></div>
      {/if}
    </button>
  {/each}
  </div>
  {/each}
</div>

{#if more.length}
  <div class="more">
    <span class="muted small">Connect</span>
    {#each more as s (s.providerId)}
      <button class="chip" onclick={() => onselect(s.providerId)}>
        <ProviderIcon id={s.providerId} size={12} />{s.providerName}
      </button>
    {/each}
  </div>
{/if}

<style>
  .more { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; padding: 0 2px; }
  .chip {
    all: unset; cursor: pointer; display: inline-flex; align-items: center; gap: 5px;
    font-size: 11px; color: var(--muted); padding: 3px 8px; border-radius: 99px; background: var(--chip);
    transition: color 0.15s;
  }
  .chip:hover { color: var(--fg); }
  .list {
    background: var(--card); border: 1px solid var(--border); border-radius: 14px; overflow: hidden;
  }
  .row {
    all: unset; box-sizing: border-box; width: 100%; cursor: pointer;
    display: flex; align-items: center; gap: 10px; padding: 11px 12px;
    transition: background 0.15s;
  }
  .row:hover { background: var(--chip); }
  .row + .row, .group + .group { border-top: 1px solid var(--border); }
  .group { position: relative; }
  /* The list's card background is translucent, so only the lifted item gets its own. */
  .group.dragging { z-index: 1; background: var(--tab-active); box-shadow: 0 4px 14px rgba(0, 0, 0, 0.12); }
  /* Right-edge grip, shown on hover; sortable lists reserve its slot so it never covers the %. */
  .list.sortable .row { padding-right: 24px; }
  .handle {
    position: absolute; right: 4px; top: 0; bottom: 0; width: 16px; z-index: 1;
    display: grid; place-items: center; color: var(--muted);
    cursor: grab; opacity: 0; transition: opacity 0.15s, color 0.15s; touch-action: none;
  }
  .group:hover .handle, .group.dragging .handle { opacity: 1; }
  .handle:hover { color: var(--fg); }
  .group.dragging .handle { cursor: grabbing; }
  .logo {
    width: 30px; height: 30px; border-radius: 9px; flex: none;
    display: grid; place-items: center; background: var(--chip);
  }
  .mid { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 5px; }
  .name {
    font-size: 12.5px; font-weight: 600; display: flex; gap: 6px; align-items: baseline;
    white-space: nowrap; overflow: hidden;
  }
  .acc { font-weight: 400; font-size: 11px; overflow: hidden; text-overflow: ellipsis; }
  .small { font-size: 11px; }
  .muted { color: var(--muted); }
  .err { color: #ff453a; }
  .bar { height: 5px; border-radius: 99px; background: var(--track); overflow: hidden; }
  .fill { height: 100%; border-radius: 99px; transition: width 0.6s cubic-bezier(0.22, 1, 0.36, 1); }
  .ok { background: linear-gradient(90deg, #30d158, #34c759); }
  .warn { background: linear-gradient(90deg, #ff9f0a, #ffb340); }
  .danger { background: linear-gradient(90deg, #ff453a, #ff6961); }
  .pct {
    font-size: 20px; font-weight: 650; letter-spacing: -0.02em; font-variant-numeric: tabular-nums;
    min-width: 48px; text-align: right;
  }
  .pct span { font-size: 12px; opacity: 0.7; margin-left: 1px; }
  .ok-text { color: var(--fg); }
  .warn-text { color: #ff9f0a; }
  .danger-text { color: #ff453a; }
</style>
