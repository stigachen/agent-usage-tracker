# Agent Usage Tracker

macOS 菜单栏和 Windows 托盘应用，用于查看各家 coding agent 的用量余额（Copilot、Claude Code、Codex、Grok 等）。

技术栈：Tauri 2 + Svelte 5 + Rust

## 支持的 Provider

| Provider | 状态 |
|---|---|
| GitHub Copilot | 可用 |
| Codex | 可用（凭据来自 Codex CLI） |
| Grok | 可用 |
| Claude Code | ⚠️ 未经实机测试（凭据来自 Claude Code，用量接口未公开，可能随时变化） |

### Copilot 按模型用量（可选）

Copilot 主额度只需在应用内用 GitHub 登录。若还想看按模型的 AI credits 明细和额外用量（additional usage），需为每个账号单独提供一个 GitHub token：

1. 打开 Settings → Accounts → Copilot model usage，点 **Set up**
2. 点 **Create token on GitHub ↗**，确认浏览器当前登录的是该账号，生成一个带 `user` scope 的 classic token
3. 粘贴并保存。应用会校验 token 属于该账号且能读取账单，通过后存入系统钥匙串

token 只用于读取 `GET /users/{login}/settings/billing/ai_credit/usage`；失效时只影响明细，主额度照常显示。多账号各自配置，互不影响。

## 开发

```bash
pnpm install
pnpm tauri dev
```

## 发布

见 [docs/RELEASING.md](docs/RELEASING.md)。
