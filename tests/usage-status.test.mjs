import assert from "node:assert/strict";
import test from "node:test";
import { grokStatus, hasGrokProblem } from "../src/lib/usage-status.ts";
import { lastSuccessText, translate } from "../src/lib/i18n.ts";

const snapshot = (fields = {}) => ({
  providerId: "grok", providerName: "Grok", accountId: "a@example.com", account: "a@example.com",
  plan: null, windows: [], error: null, needsAuth: false, managed: true, loginHint: null,
  note: null, periodEndsAt: null, billing: null, billingConfigured: false, hidden: false,
  fetchedAt: "2026-10-05T00:10:00Z", ...fields,
});

test("Grok credential states remain distinct with English and Chinese labels", () => {
  for (const [credentialIssue, en, zh] of [
    ["missing", "Not signed in", "未登录"],
    ["expired", "Session expired", "登录已过期"],
    ["rejected", "Sign-in rejected", "登录凭据被拒绝"],
    ["unreadable", "Can't read sign-in", "无法读取登录信息"],
    ["invalid", "Invalid sign-in data", "登录信息无效"],
    ["unsupported", "Unsupported sign-in", "不支持的登录方式"],
    ["ambiguous", "Multiple sign-ins found", "无法确定登录账号"],
    ["changed", "Sign-in changed", "登录信息已变化"],
  ]) {
    const status = grokStatus(snapshot({ credentialIssue }));
    assert.equal(status, en);
    assert.equal(translate("zh-CN", status), zh);
  }
});

test("network and service failures are not presented as an expired login", () => {
  for (const [fetchIssue, label] of [
    ["network", "Network request failed"], ["service", "Service unavailable"], ["response", "Invalid usage response"],
  ]) {
    assert.equal(grokStatus(snapshot({ fetchIssue, error: "details", stale: true })), label);
  }
  assert.equal(grokStatus(snapshot({ error: "unknown" })), "Couldn't load usage");
  assert.equal(grokStatus(snapshot()), null);
});

test("read failures stay visible without an account ID while absent logins stay unconnected", () => {
  assert.equal(hasGrokProblem(snapshot({ accountId: null, credentialIssue: "missing", needsAuth: true })), false);
  for (const credentialIssue of ["unreadable", "invalid", "unsupported", "ambiguous"]) {
    assert.equal(hasGrokProblem(snapshot({ accountId: null, credentialIssue })), true);
  }
});

test("other providers keep their existing status and grouping behavior", () => {
  for (const providerId of ["codex", "copilot", "claude"]) {
    const other = snapshot({ providerId, needsAuth: true, error: "failed" });
    assert.equal(grokStatus(other), null);
    assert.equal(hasGrokProblem(other), false);
  }
});

test("historical usage ages from the last success, not the latest failed attempt", () => {
  const now = Date.parse("2026-10-05T00:10:00Z");
  const old = snapshot({ stale: true, lastSuccessAt: "2026-10-05T00:00:00Z" });
  assert.equal(lastSuccessText("en", old.lastSuccessAt, now), "Last success 10m ago");
  assert.equal(lastSuccessText("zh-CN", old.lastSuccessAt, now), "上次成功更新：10 分钟前");
  assert.equal(lastSuccessText("en", old.lastSuccessAt, now + 3600_000), "Last success 1h ago");
  assert.equal(lastSuccessText("zh-CN", old.lastSuccessAt, now + 86400_000), "上次成功更新：1 天前");
  assert.equal(lastSuccessText("en", "invalid", now), "Previous usage");
  assert.equal(lastSuccessText("en", "2099-01-01", now), "Last success just now");
});
