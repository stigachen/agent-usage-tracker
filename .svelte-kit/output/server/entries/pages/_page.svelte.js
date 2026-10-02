import "../../chunks/index-server.js";
import { i as ensure_array_like, r as derived, t as attr_class, v as attr, y as escape_html } from "../../chunks/server.js";
import { invoke } from "@tauri-apps/api/core";
import "@tauri-apps/api/event";
import "@tauri-apps/api/window";
import "@tauri-apps/api/app";
import "@tauri-apps/plugin-autostart";
//#endregion
//#region src/lib/Settings.svelte
function Settings($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let { snaps, onclose } = $$props;
		let tray = "lowest";
		let refreshSecs = 600;
		let autostart = false;
		let confirming = null;
		let error = null;
		let version = "";
		let accounts = derived(() => snaps.filter((s) => s.accountId));
		const key = (s) => `${s.providerId}:${s.accountId}`;
		const intervals = [
			[60, "1 min"],
			[300, "5 min"],
			[600, "10 min"],
			[1800, "30 min"],
			[3600, "1 hour"]
		];
		async function run(f) {
			error = null;
			try {
				await f();
			} catch (e) {
				error = String(e);
			}
		}
		function setTray(v) {
			tray = v;
			let display;
			if (v === "lowest" || v === "iconOnly") display = { mode: v };
			else {
				const i = v.indexOf(":");
				display = {
					mode: "pinned",
					provider: v.slice(0, i),
					account: v.slice(i + 1)
				};
			}
			run(() => invoke("set_tray_display", { display }));
		}
		function setRefresh(v) {
			refreshSecs = v;
			run(() => invoke("set_refresh_secs", { secs: v }));
		}
		$$renderer.push(`<div class="settings svelte-lqmuci"><div class="top svelte-lqmuci"><button class="back svelte-lqmuci" aria-label="Back"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="m15 18-6-6 6-6"></path></svg></button> <h1 class="svelte-lqmuci">Settings</h1></div> <section class="svelte-lqmuci"><h3 class="svelte-lqmuci">Menu bar</h3> <div class="group svelte-lqmuci"><label class="row svelte-lqmuci"><span>Show</span> `);
		$$renderer.select({
			value: tray,
			onchange: (e) => setTray(e.currentTarget.value),
			class: ""
		}, ($$renderer) => {
			$$renderer.option({ value: "lowest" }, ($$renderer) => {
				$$renderer.push(`Lowest remaining`);
			});
			$$renderer.push(`<!--[-->`);
			const each_array = ensure_array_like(accounts());
			for (let $$index = 0, $$length = each_array.length; $$index < $$length; $$index++) {
				let s = each_array[$$index];
				$$renderer.option({ value: key(s) }, ($$renderer) => {
					$$renderer.push(`${escape_html(s.providerName)} · @${escape_html(s.account)}`);
				});
			}
			$$renderer.push(`<!--]-->`);
			$$renderer.option({ value: "iconOnly" }, ($$renderer) => {
				$$renderer.push(`Icon only`);
			});
		}, "svelte-lqmuci");
		$$renderer.push(`</label></div></section> <section class="svelte-lqmuci"><h3 class="svelte-lqmuci">General</h3> <div class="group svelte-lqmuci"><label class="row svelte-lqmuci"><span>Refresh every</span> `);
		$$renderer.select({
			value: refreshSecs,
			onchange: (e) => setRefresh(+e.currentTarget.value),
			class: ""
		}, ($$renderer) => {
			$$renderer.push(`<!--[-->`);
			const each_array_1 = ensure_array_like(intervals);
			for (let $$index_1 = 0, $$length = each_array_1.length; $$index_1 < $$length; $$index_1++) {
				let [v, l] = each_array_1[$$index_1];
				$$renderer.option({ value: v }, ($$renderer) => {
					$$renderer.push(`${escape_html(l)}`);
				});
			}
			$$renderer.push(`<!--]-->`);
		}, "svelte-lqmuci");
		$$renderer.push(`</label> <div class="row svelte-lqmuci"><span>Launch at login</span> <button${attr_class("switch svelte-lqmuci", void 0, { "on": autostart })} role="switch"${attr("aria-checked", autostart)} aria-label="Launch at login"><span class="knob svelte-lqmuci"></span></button></div></div></section> <section class="svelte-lqmuci"><h3 class="svelte-lqmuci">Accounts</h3> <div class="group svelte-lqmuci">`);
		const each_array_2 = ensure_array_like(accounts());
		if (each_array_2.length !== 0) {
			$$renderer.push("<!--[-->");
			for (let $$index_2 = 0, $$length = each_array_2.length; $$index_2 < $$length; $$index_2++) {
				let s = each_array_2[$$index_2];
				$$renderer.push(`<div class="row svelte-lqmuci"><span class="acc svelte-lqmuci"><span>@${escape_html(s.account)}</span> <span class="muted svelte-lqmuci">${escape_html(s.providerName)}</span></span> <button${attr_class("danger svelte-lqmuci", void 0, { "armed": confirming === key(s) })}>${escape_html(confirming === key(s) ? "Sign out?" : "Sign out")}</button></div>`);
			}
		} else $$renderer.push(`<!--[!--><div class="row muted svelte-lqmuci">No accounts yet</div>`);
		$$renderer.push(`<!--]--></div></section> <section class="svelte-lqmuci"><h3 class="svelte-lqmuci">About</h3> <div class="group svelte-lqmuci"><div class="about svelte-lqmuci"><img src="/app-icon.png" alt="" width="44" height="44" class="svelte-lqmuci"/> <div class="about-text svelte-lqmuci"><span class="name svelte-lqmuci">Agent Usage</span> <span class="muted svelte-lqmuci">Version ${escape_html(version)}</span> <span class="muted svelte-lqmuci">Usage and quota for your coding agents</span></div></div> <div class="row svelte-lqmuci"><span class="muted svelte-lqmuci">© 2026 Guang Chen</span> <button class="link svelte-lqmuci">GitHub ↗</button></div></div></section> `);
		if (error) $$renderer.push(`<!--[0--><p class="error svelte-lqmuci">${escape_html(error)}</p>`);
		else $$renderer.push("<!--[-1-->");
		$$renderer.push(`<!--]--></div>`);
	});
}
//#endregion
//#region src/routes/+page.svelte
function _page($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		let snaps = [];
		let refreshing = false;
		let showSettings = false;
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
		$$renderer.push(`<main class="svelte-1uha8ag">`);
		if (showSettings) {
			$$renderer.push("<!--[0-->");
			Settings($$renderer, {
				snaps,
				onclose: () => showSettings = false
			});
		} else {
			$$renderer.push(`<!--[-1--><div class="top svelte-1uha8ag"><h1 class="svelte-1uha8ag">Agent Usage</h1> <div class="actions svelte-1uha8ag"><button class="icon svelte-1uha8ag" title="Settings" aria-label="Settings"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"></path><circle cx="12" cy="12" r="3"></circle></svg></button> <button${attr_class("icon svelte-1uha8ag", void 0, { "spin": refreshing })} title="Refresh" aria-label="Refresh"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" class="svelte-1uha8ag"><path d="M21 12a9 9 0 1 1-3-6.7L21 8"></path><path d="M21 3v5h-5"></path></svg></button></div></div> <div class="list svelte-1uha8ag">`);
			$$renderer.push(`<!--[-1--><div class="skeleton svelte-1uha8ag"><div class="sk-row svelte-1uha8ag"><div class="sk sk-logo svelte-1uha8ag"></div><div class="sk sk-line w40 svelte-1uha8ag"></div></div> <div class="sk sk-line big svelte-1uha8ag"></div> <div class="sk sk-bar svelte-1uha8ag"></div></div>`);
			$$renderer.push(`<!--]--></div> <footer class="svelte-1uha8ag"><span class="muted svelte-1uha8ag">${escape_html(updatedText())}</span> <button class="link svelte-1uha8ag">Quit</button></footer>`);
		}
		$$renderer.push(`<!--]--></main>`);
	});
}
//#endregion
export { _page as default };
