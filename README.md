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

## 开发

```bash
pnpm install
pnpm tauri dev
```

## 发布

见 [docs/RELEASING.md](docs/RELEASING.md)。
