import assert from "node:assert/strict";
import test from "node:test";
import { providerText, resetText, translate, zhCN } from "../src/lib/i18n.ts";

test("Chinese messages preserve every interpolation parameter", () => {
  const parameters = (text) => [...text.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort();
  for (const [english, chinese] of Object.entries(zhCN)) {
    assert.ok(chinese.trim(), english);
    assert.deepEqual(parameters(chinese), parameters(english), english);
    assert.equal(translate("en", english), english);
  }
});

test("account names and diagnostic details are interpolated literally", () => {
  assert.equal(translate("zh-CN", "Show @{account} in Overview", { account: "alice@example.com" }), "在总览中显示 @alice@example.com");
  assert.equal(translate("zh-CN", "Updated {minutes}m ago", { minutes: 0 }), "0 分钟前更新");
  assert.equal(providerText("zh-CN", "Bad response: missing {field} at $.items"), "响应解析失败：missing {field} at $.items");
  assert.equal(providerText("zh-CN", "This token belongs to @alice, not @bob"), "此令牌属于 @alice，而非 @bob");
});

test("provider quotas and guidance translate without changing unfamiliar API data", () => {
  for (const [input, expected] of [
    ["5-hour limit", "5 小时额度"],
    ["30-day limit", "30 天额度"],
    ["Weekly Opus limit", "Opus 每周额度"],
    ["Run `codex login` in a terminal, then refresh.", "在终端中运行 `codex login`，然后刷新。"],
    ["Billing request failed (HTTP 403)", "账单请求失败（HTTP 403）"],
    ["HTTP 503: upstream unavailable", "HTTP 503: upstream unavailable"],
    ["Future model / quota", "Future model / quota"],
    ["constructor", "constructor"],
  ]) {
    assert.equal(providerText("zh-CN", input), expected);
    assert.equal(providerText("en", input), input);
  }
});

test("reset times use the chosen language for future, expired, and absent windows", () => {
  const now = Date.parse("2026-10-05T00:00:00Z");
  const future = "2026-10-06T02:00:00Z";
  assert.equal(resetText("en", future, now), "Resets in 1d 2h");
  assert.equal(resetText("zh-CN", future, now), "1 天 2 小时后重置");
  assert.equal(resetText("zh-CN", future, now, true), "当前周期将在 1 天 2 小时后重置");
  assert.equal(resetText("zh-CN", "2026-10-05T00:00:00Z", now), "即将重置");
  assert.equal(resetText("en", "2026-10-04T00:00:00Z", now, true), "Period resets soon");
  assert.equal(resetText("zh-CN", null, now), "");
});
