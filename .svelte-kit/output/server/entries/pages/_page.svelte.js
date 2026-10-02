import "../../chunks/index-server.js";
import { r as derived, t as attr_class, y as escape_html } from "../../chunks/server.js";
import "@tauri-apps/api/core";
import "@tauri-apps/api/event";
import "@tauri-apps/api/window";
//#endregion
//#region src/routes/+page.svelte
function _page($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let snaps = [];
		let refreshing = false;
		derived(() => Object.values(snaps.reduce((g, s) => {
			(g[s.providerId] ??= []).push(s);
			return g;
		}, {})));
		let now = Date.now();
		let updated = derived(() => snaps[0] ? new Date(snaps[0].fetchedAt).getTime() : null);
		let updatedText = derived(() => {
			if (!updated()) return "";
			const m = Math.floor((now - updated()) / 6e4);
			return m < 1 ? "Updated just now" : m < 60 ? `Updated ${m}m ago` : `Updated ${Math.floor(m / 60)}h ago`;
		});
		$$renderer.push(`<main class="svelte-1uha8ag"><div class="top svelte-1uha8ag"><h1 class="svelte-1uha8ag">Agent Usage</h1> <button${attr_class("icon svelte-1uha8ag", void 0, { "spin": refreshing })} title="Refresh" aria-label="Refresh"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" class="svelte-1uha8ag"><path d="M21 12a9 9 0 1 1-3-6.7L21 8"></path><path d="M21 3v5h-5"></path></svg></button></div> <div class="list svelte-1uha8ag">`);
		$$renderer.push(`<!--[-1--><div class="skeleton svelte-1uha8ag"><div class="sk-row svelte-1uha8ag"><div class="sk sk-logo svelte-1uha8ag"></div><div class="sk sk-line w40 svelte-1uha8ag"></div></div> <div class="sk sk-line big svelte-1uha8ag"></div> <div class="sk sk-bar svelte-1uha8ag"></div></div>`);
		$$renderer.push(`<!--]--></div> <footer class="svelte-1uha8ag"><span class="muted svelte-1uha8ag">${escape_html(updatedText())}</span> <button class="link svelte-1uha8ag">Quit</button></footer></main>`);
	});
}
//#endregion
export { _page as default };
