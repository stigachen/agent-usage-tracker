import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { createServer } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

let server;
let Overview;
let ProviderCard;
let language;
let render;
before(async () => {
  // Compile the real components for SSR; no browser, Tauri process, sockets or API calls.
  server = await createServer({
    configFile: false,
    cacheDir: "node_modules/.cache/usage-ui-tests",
    plugins: [svelte({ configFile: false })],
    server: { middlewareMode: true, hmr: false, ws: false, watch: null },
    appType: "custom",
  });
  Overview = (await server.ssrLoadModule("/src/lib/Overview.svelte")).default;
  ProviderCard = (await server.ssrLoadModule("/src/lib/ProviderCard.svelte")).default;
  language = (await server.ssrLoadModule("/src/lib/language.ts")).language;
  render = (await server.ssrLoadModule("svelte/server")).render;
});
after(async () => { await server?.close(); });

const now = Date.parse("2026-10-05T00:10:00Z");
const snapshot = (fields = {}) => ({
  providerId: "grok", providerName: "Grok", accountId: "a@example.com", account: "a@example.com",
  plan: "SuperGrok", windows: [{ label: "Weekly limit", used: 23, limit: 100, resetsAt: "2026-10-08T00:00:00Z" }],
  error: null, needsAuth: false, managed: true, loginHint: null, note: null, periodEndsAt: null,
  billing: null, billingConfigured: false, hidden: false, fetchedAt: "2026-10-05T00:10:00Z", ...fields,
});

function markup(component, snap, locale = "en") {
  language.set({ preference: locale, locale, ready: true, saving: false });
  return render(component, { props: { snaps: [snap], now, onselect() {} } }).body;
}

function text(html) {
  return html.replace(/<[^>]*>/g, "").replace(/\s+/g, " ").trim();
}

test("both cards clearly identify historical usage and keep the original success time", () => {
  const expired = snapshot({
    needsAuth: true, credentialIssue: "expired", stale: true, lastSuccessAt: "2026-10-05T00:00:00Z",
    loginHint: "Run `grok` to renew your session. Usage updates automatically when the sign-in changes.",
  });
  for (const component of [Overview, ProviderCard]) {
    for (const [locale, status, previous, time] of [
      ["en", "Session expired", "Previous", "Last success 10m ago"],
      ["zh-CN", "登录已过期", "上次", "上次成功更新：10 分钟前"],
    ]) {
      const html = markup(component, expired, locale);
      const visible = text(html);
      assert.ok(visible.includes(status), visible);
      assert.ok(visible.includes(previous), visible);
      assert.ok(visible.includes(time), visible);
      assert.match(visible, /77\s*%/);
      assert.doesNotMatch(visible, /Resets|后重置|即将重置/);
      assert.doesNotMatch(html, /class="bar\b/);
    }
  }
});

test("network failure remains visible beside historical usage without a login prompt", () => {
  const failed = snapshot({
    error: "Network details", fetchIssue: "network", stale: true, lastSuccessAt: "2026-10-05T00:00:00Z",
  });
  for (const component of [Overview, ProviderCard]) {
    const visible = text(markup(component, failed));
    assert.match(visible, /Network request failed/);
    assert.match(visible, /Previous/);
    assert.doesNotMatch(visible, /Session expired|grok login|Not connected/);
  }
});

test("a successful recovery restores the current quota and reset display", () => {
  for (const component of [Overview, ProviderCard]) {
    const html = markup(component, snapshot());
    const visible = text(html);
    assert.match(visible, /77\s*%/);
    assert.match(visible, /Resets in/);
    assert.match(html, /class="bar\b/);
    assert.doesNotMatch(visible, /Previous|Last success|expired/);
  }
});

test("unreadable Grok login has an explicit state in both no-account cards", () => {
  const unreadable = snapshot({
    accountId: null, account: null, plan: null, windows: [], credentialIssue: "unreadable",
    error: "Couldn't read Grok CLI credentials. Check the file permissions and GROK_HOME.",
    loginHint: "Couldn't read Grok CLI credentials. Check the file permissions and GROK_HOME.",
  });
  for (const component of [Overview, ProviderCard]) {
    const visible = text(markup(component, unreadable, "zh-CN"));
    assert.match(visible, /无法读取登录信息/);
    assert.doesNotMatch(visible, /未连接|登录已过期|77/);
  }
});

test("other providers preserve quota rendering and existing session-expired guidance", () => {
  const codex = snapshot({ providerId: "codex", providerName: "Codex", plan: null });
  assert.match(text(markup(ProviderCard, codex)), /77\s*%/);
  const expired = { ...codex, needsAuth: true, loginHint: "Run `codex login` in a terminal, then refresh." };
  const visible = text(markup(ProviderCard, expired));
  assert.match(visible, /Session expired/);
  assert.match(visible, /codex login/);
  assert.doesNotMatch(visible, /Previous|77\s*%/);
});
