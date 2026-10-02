import "../../chunks/index-server.js";
import { i as ensure_array_like, n as attr_style, o as stringify, r as derived, t as attr_class, v as escape_html } from "../../chunks/server.js";
import "@tauri-apps/api/core";
import "@tauri-apps/api/event";
//#region src/lib/ProviderCard.svelte
function ProviderCard($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { snap } = $$props;
		const fmt = new Intl.NumberFormat(void 0, { maximumFractionDigits: 0 });
		function resetIn(iso) {
			if (!iso) return "";
			const ms = new Date(iso).getTime() - Date.now();
			if (ms <= 0) return "Resets soon";
			const d = Math.floor(ms / 864e5);
			const h = Math.floor(ms % 864e5 / 36e5);
			return d > 0 ? `Resets in ${d}d ${h}h` : `Resets in ${h}h`;
		}
		function tone(r) {
			return r >= .9 ? "danger" : r >= .7 ? "warn" : "ok";
		}
		$$renderer.push(`<section class="card svelte-cyn0ng"><header class="svelte-cyn0ng"><div class="logo svelte-cyn0ng"><svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true"><path fill="currentColor" d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8"></path></svg></div> <div class="title svelte-cyn0ng"><h2 class="svelte-cyn0ng">${escape_html(snap.providerName)}</h2> `);
		if (snap.account) $$renderer.push(`<!--[0--><span class="sub svelte-cyn0ng">@${escape_html(snap.account)}${escape_html(snap.plan ? ` · ${snap.plan}` : "")}</span>`);
		else $$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--></div></header> `);
		if (snap.needsAuth) {
			$$renderer.push("<!--[0-->");
			$$renderer.push(`<!--[-1--><button class="primary svelte-cyn0ng">Sign in with GitHub</button>`);
			$$renderer.push(`<!--]--> `);
			$$renderer.push("<!--[-1-->");
			$$renderer.push(`<!--]-->`);
		} else if (snap.error) $$renderer.push(`<!--[1--><p class="error svelte-cyn0ng">${escape_html(snap.error)}</p>`);
		else {
			$$renderer.push(`<!--[-1--><!--[-->`);
			const each_array = ensure_array_like(snap.windows);
			for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
				let w = each_array[$$index];
				$$renderer.push(`<div class="quota svelte-cyn0ng"><div class="row svelte-cyn0ng"><span class="label svelte-cyn0ng">${escape_html(w.label)}</span> `);
				if (w.limit) $$renderer.push(`<!--[0--><span class="value svelte-cyn0ng">${escape_html(fmt.format(w.used))} <span class="muted svelte-cyn0ng">/ ${escape_html(fmt.format(w.limit))}</span></span>`);
				else $$renderer.push(`<!--[-1--><span class="value muted svelte-cyn0ng">Unlimited</span>`);
				$$renderer.push(`<!--]--></div> `);
				if (w.limit) {
					$$renderer.push("<!--[0-->");
					const r = Math.min(w.used / w.limit, 1);
					$$renderer.push(`<div class="bar svelte-cyn0ng"><div${attr_class(`fill ${stringify(tone(r))}`, "svelte-cyn0ng")}${attr_style("", { width: `${stringify(Math.max(r * 100, 1.5))}%` })}></div></div> <div class="row foot svelte-cyn0ng"><span class="muted svelte-cyn0ng">${escape_html((100 - r * 100).toFixed(1))}% left</span> <span class="muted svelte-cyn0ng">${escape_html(resetIn(w.resetsAt))}</span></div>`);
				} else $$renderer.push("<!--[-1-->");
				$$renderer.push(`<!--]--></div>`);
			}
			$$renderer.push(`<!--]-->`);
		}
		$$renderer.push(`<!--]--></section>`);
	});
}
//#endregion
//#region src/routes/+page.svelte
function _page($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let snaps = [];
		let refreshing = false;
		let updated = derived(() => snaps[0] ? new Date(snaps[0].fetchedAt) : null);
		$$renderer.push(`<main class="svelte-1uha8ag"><div class="top svelte-1uha8ag"><h1 class="svelte-1uha8ag">Agent Usage</h1> <button${attr_class("icon svelte-1uha8ag", void 0, { "spin": refreshing })} title="Refresh" aria-label="Refresh"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" class="svelte-1uha8ag"><path d="M21 12a9 9 0 1 1-3-6.7L21 8"></path><path d="M21 3v5h-5"></path></svg></button></div> <div class="list svelte-1uha8ag">`);
		const each_array = ensure_array_like(snaps);
		if (each_array.length !== 0) {
			$$renderer.push("<!--[-->");
			for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
				let snap = each_array[$$index];
				ProviderCard($$renderer, { snap });
			}
		} else $$renderer.push(`<!--[!--><p class="muted center svelte-1uha8ag">Loading…</p>`);
		$$renderer.push(`<!--]--></div> <footer class="svelte-1uha8ag"><span class="muted svelte-1uha8ag">${escape_html(updated() ? `Updated ${updated().toLocaleTimeString([], {
			hour: "2-digit",
			minute: "2-digit"
		})}` : "")}</span> <button class="link svelte-1uha8ag">Quit</button></footer></main>`);
	});
}
//#endregion
export { _page as default };
