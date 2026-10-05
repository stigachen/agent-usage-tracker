export type Locale = "en" | "zh-CN";
export type LanguagePreference = "system" | Locale;
export interface LanguageSettings {
  preference: LanguagePreference;
  locale: Locale;
}

// English is also the message key, so untranslated external/provider text can
// remain readable without changing the provider protocol or cached snapshots.
export const zhCN = {
  "Overview": "总览",
  "Settings": "设置",
  "Refresh": "刷新",
  "Quit": "退出",
  "Back": "返回",
  "General": "通用",
  "Accounts": "账号",
  "Updated just now": "刚刚更新",
  "Updated {minutes}m ago": "{minutes} 分钟前更新",
  "Updated {hours}h ago": "{hours} 小时前更新",
  "Checked just now": "刚刚检查",
  "Checked {minutes}m ago": "{minutes} 分钟前检查",
  "Checked {hours}h ago": "{hours} 小时前检查",
  "Not signed in": "未登录",
  "Sign-in rejected": "登录凭据被拒绝",
  "Can't read sign-in": "无法读取登录信息",
  "Invalid sign-in data": "登录信息无效",
  "Unsupported sign-in": "不支持的登录方式",
  "Multiple sign-ins found": "无法确定登录账号",
  "Sign-in changed": "登录信息已变化",
  "Network request failed": "网络请求失败",
  "Service unavailable": "服务暂不可用",
  "Invalid usage response": "用量响应无效",
  "Previous": "上次",
  "Previous usage": "历史用量",
  "Last success just now": "上次成功更新：刚刚",
  "Last success {minutes}m ago": "上次成功更新：{minutes} 分钟前",
  "Last success {hours}h ago": "上次成功更新：{hours} 小时前",
  "Last success {days}d ago": "上次成功更新：{days} 天前",
  "Usage will retry automatically.": "用量查询会自动重试。",
  "Sign in with `grok login`. Usage updates automatically.": "请运行 `grok login` 登录，用量会自动更新。",
  "Run `grok` to renew your session. Usage updates automatically when the sign-in changes.": "请运行 `grok` 更新登录凭据，凭据变化后用量会自动更新。",
  "Run `grok login` to sign in again. Usage updates automatically.": "请运行 `grok login` 重新登录，用量会自动更新。",
  "Couldn't load Grok's usage percentage.": "无法获取 Grok 的用量百分比。",
  "Grok sign-in changed. Usage will update automatically.": "Grok 登录信息已变化，用量会自动更新。",
  "System tray": "系统托盘",
  "Menu bar": "菜单栏",
  "Hover over the tray icon to see the selected remaining quota.": "将鼠标悬停在托盘图标上，查看所选账号的剩余额度。",
  "Show": "显示",
  "Lowest remaining": "最低剩余额度",
  "Icon only": "仅图标",
  "Language": "语言",
  "Follow system": "跟随系统",
  "Refresh every": "刷新间隔",
  "{minutes} min": "{minutes} 分钟",
  "1 hour": "1 小时",
  "Launch at login": "登录时启动",
  "About": "关于",
  "Version {version}": "版本 {version}",
  "Usage and quota for your coding agents": "查看编程助手的用量和额度",
  "Switch off to hide an account from Overview. It stays in its own tab.": "关闭后，该账号会从总览中隐藏，仍可在对应服务的标签页查看。",
  "Show @{account} in Overview": "在总览中显示 @{account}",
  "Hidden from Overview": "已从总览隐藏",
  "Shown in Overview": "已在总览显示",
  "Sign out?": "确认退出登录？",
  "Sign out": "退出登录",
  "Sign out @{account}": "退出账号 @{account}",
  "No accounts yet": "暂无账号",
  "Copilot model usage": "Copilot 模型用量",
  "Token saved": "令牌已保存",
  "Not set up": "尚未设置",
  "Replace": "替换",
  "Remove": "移除",
  "Set up": "设置",
  "Create a classic token with the user scope while signed in to GitHub as @{account}. It is stored in the system credential store and only used to read billing.": "使用 @{account} 登录 GitHub，创建一个具有 user 权限的经典令牌。令牌保存在系统凭据存储中，仅用于读取账单。",
  "Create token on GitHub ↗": "前往 GitHub 创建令牌 ↗",
  "Checking…": "正在验证…",
  "Save": "保存",
  "Cancel": "取消",
  "All accounts are hidden from the overview.": "所有账号均已从总览隐藏。",
  "Drag to reorder": "拖动排序",
  "Not connected": "未连接",
  "Couldn't load": "加载失败",
  "No usage data": "暂无用量数据",
  "Unlimited": "无限制",
  "{count} hidden": "已隐藏 {count} 个账号",
  "Connect": "连接",
  "{count} accounts": "{count} 个账号",
  "Add account": "添加账号",
  "Enter this code on GitHub": "在 GitHub 上输入此验证码",
  "Copy": "复制",
  "Copied to clipboard": "已复制到剪贴板",
  "Waiting for authorization…": "等待授权…",
  "Sign in with GitHub": "使用 GitHub 登录",
  "Session expired": "登录已过期",
  "Sign out and add the account again.": "请退出登录后重新添加账号。",
  "Couldn't load usage": "无法加载用量",
  "{quota} left": "{quota}剩余",
  "{percent}% used": "已使用 {percent}%",
  "{percent}% left": "剩余 {percent}%",
  "Couldn't load model usage": "无法加载模型用量",
  "Additional usage": "额外用量",
  "By model ({count})": "按模型查看（{count}）",
  "Resets soon": "即将重置",
  "Resets in {duration}": "{duration}后重置",
  "Period resets soon": "当前周期即将重置",
  "Period resets in {duration}": "当前周期将在 {duration}后重置",
  "{days}d {hours}h": "{days} 天 {hours} 小时",
  "{hours}h": "{hours} 小时",
  "Premium requests": "高级请求",
  "Chat": "聊天",
  "Completions": "代码补全",
  "Weekly limit": "每周额度",
  "Monthly limit": "每月额度",
  "Daily limit": "每日额度",
  "Usage limit": "用量额度",
  "Weekly Opus limit": "Opus 每周额度",
  "Weekly Sonnet limit": "Sonnet 每周额度",
  "{hours}-hour limit": "{hours} 小时额度",
  "{days}-day limit": "{days} 天额度",
  "Other": "其他",
  "Run `claude` and sign in with /login, then refresh.": "运行 `claude`，使用 /login 登录，然后刷新。",
  "Run `codex login` in a terminal, then refresh.": "在终端中运行 `codex login`，然后刷新。",
  "Grok CLI credentials were not found. Run `grok login`. Usage updates automatically.": "未找到 Grok CLI 凭据。请运行 `grok login`，用量会自动更新。",
  "Couldn't read Grok CLI credentials. Check the file permissions and GROK_HOME.": "无法读取 Grok CLI 凭据。请检查文件权限和 GROK_HOME。",
  "Grok CLI credentials are incomplete or invalid. Run `grok login`. Usage updates automatically.": "Grok CLI 凭据不完整或无效。请运行 `grok login`，用量会自动更新。",
  "No supported Grok CLI login was found. Run `grok login`. Usage updates automatically.": "未找到支持的 Grok CLI 登录凭据。请运行 `grok login`，用量会自动更新。",
  "Multiple Grok CLI credential entries were found. The active account could not be determined.": "发现多条 Grok CLI 凭据，无法确定当前账号。",
  "Grok didn't report usage for this account.": "Grok 未提供此账号的用量数据。",
  "Grok doesn't report a usage percentage for this plan yet.": "Grok 尚未提供此套餐的用量百分比。",
  "Billing token is invalid or expired": "账单令牌无效或已过期",
  'Billing token lacks the "user" scope': "账单令牌缺少 user 权限",
  "Token is invalid or expired": "令牌无效或已过期",
  "Token was changed while this one was being checked": "验证期间令牌已被更改",
  "Billing request failed (HTTP {status})": "账单请求失败（HTTP {status}）",
  "This token belongs to @{owner}, not @{account}": "此令牌属于 @{owner}，而非 @{account}",
  "Bad response: {detail}": "响应解析失败：{detail}",
  "Bad billing response: {detail}": "账单响应解析失败：{detail}",
  "Couldn't read the token from the system credential store: {detail}": "无法从系统凭据存储中读取令牌：{detail}",
  "Signed out, but the token couldn't be removed from the system credential store: {detail}": "已退出登录，但无法从系统凭据存储中移除令牌：{detail}",
  "login timed out": "登录超时",
  "unexpected response": "响应异常",
} as const;

export type Message = keyof typeof zhCN;
export type Params = Record<string, string | number>;
export type Translator = (message: Message, params?: Params) => string;

export function translate(locale: Locale, message: Message, params: Params = {}): string {
  const template = locale === "zh-CN" ? zhCN[message] : message;
  return template.replace(/\{(\w+)\}/g, (match, key: string) => String(params[key] ?? match));
}

export function lastSuccessText(locale: Locale, timestamp: string, now: number): string {
  const at = Date.parse(timestamp);
  if (!Number.isFinite(at)) return translate(locale, "Previous usage");
  const minutes = Math.max(0, Math.floor((now - at) / 60_000));
  if (minutes < 1) return translate(locale, "Last success just now");
  if (minutes < 60) return translate(locale, "Last success {minutes}m ago", { minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return translate(locale, "Last success {hours}h ago", { hours });
  return translate(locale, "Last success {days}d ago", { days: Math.floor(hours / 24) });
}

export function providerText(locale: Locale, text: string): string {
  if (locale === "en") return text;
  if (Object.prototype.hasOwnProperty.call(zhCN, text)) return translate(locale, text as Message);
  let match = /^(\d+)-hour limit$/.exec(text);
  if (match) return translate(locale, "{hours}-hour limit", { hours: match[1] });
  match = /^(\d+)-day limit$/.exec(text);
  if (match) return translate(locale, "{days}-day limit", { days: match[1] });
  match = /^Billing request failed \(HTTP (\d+)\)$/.exec(text);
  if (match) return translate(locale, "Billing request failed (HTTP {status})", { status: match[1] });
  match = /^This token belongs to @(.+), not @(.+)$/.exec(text);
  if (match) return translate(locale, "This token belongs to @{owner}, not @{account}", { owner: match[1], account: match[2] });
  for (const message of [
    "Bad response: {detail}",
    "Bad billing response: {detail}",
    "Couldn't read the token from the system credential store: {detail}",
    "Signed out, but the token couldn't be removed from the system credential store: {detail}",
  ] as const) {
    const prefix = message.slice(0, -"{detail}".length);
    if (text.startsWith(prefix)) return translate(locale, message, { detail: text.slice(prefix.length) });
  }
  // Unknown API errors, model names and future quota labels retain their detail.
  return text;
}

export function resetText(locale: Locale, iso: string | null, now: number, period = false): string {
  if (!iso) return "";
  const ms = new Date(iso).getTime() - now;
  if (ms <= 0) return translate(locale, period ? "Period resets soon" : "Resets soon");
  const days = Math.floor(ms / 86_400_000);
  const hours = Math.floor((ms % 86_400_000) / 3_600_000);
  const duration = translate(locale, days > 0 ? "{days}d {hours}h" : "{hours}h", { days, hours });
  return translate(locale, period ? "Period resets in {duration}" : "Resets in {duration}", { duration });
}
