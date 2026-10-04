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
3. 粘贴并保存。应用会校验 token 属于该账号且能读取账单，通过后存入系统凭据存储（macOS Keychain / Windows Credential Manager）

token 只用于读取 `GET /users/{login}/settings/billing/ai_credit/usage`；失效时只影响明细，主额度照常显示。多账号各自配置，互不影响。

## 开发

需要 Node.js LTS、项目 `packageManager` 指定版本的 pnpm，以及 Rust stable。
系统依赖见 [Tauri 环境准备](https://v2.tauri.app/start/prerequisites/)。

```bash
pnpm install
pnpm tauri dev
```

### Windows 首版（免安装）

下载 Release 中的 `Agent.Usage_<版本>_windows_x64_portable.zip`，或 CI 的
`agent-usage-windows-x64-portable` 产物，解压后双击 **Agent Usage.exe**。
应用启动后位于系统托盘，不需要安装应用或管理员权限。

运行依赖系统的 [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/#download-section)。
Windows 11 和大多数 Windows 10 已有该组件；缺失时需单独安装。便携包不附带或自动安装
WebView2，Visual C++ 运行库已静态链接到程序中。

目标平台为 **Windows 10/11 x64**，当前仍需 Windows 实机验收。开发时安装
Visual Studio Build Tools 的 **Desktop development with C++**（含 Windows SDK）、
Rust MSVC 工具链和 Microsoft Edge WebView2 Runtime，再执行上面的开发命令。

```powershell
pnpm check
cargo test --locked --manifest-path src-tauri/Cargo.toml
pnpm tauri build --target x86_64-pc-windows-msvc --no-bundle
./scripts/package-windows.ps1
```

便携 ZIP 输出在 `src-tauri/target/Agent.Usage_<版本>_windows_x64_portable.zip`。
`tauri.windows.conf.json` 会自动合并，关闭安装包生成。

- 左键点击托盘图标展开/收起面板，右键菜单提供打开、刷新和退出。
- Windows 的剩余额度显示在托盘悬停提示中，可在 Settings → System tray 选择账号。
- 关闭面板后继续驻留托盘；再次启动会打开已有实例。
- Copilot 在应用内登录，token 存入 Windows Credential Manager。
- Codex、Claude Code、Grok 读取当前 Windows 用户的 CLI 凭据文件：

  | Provider | 默认读取路径 | 可覆盖的环境变量 |
  |---|---|---|
  | Codex | `%USERPROFILE%\.codex\auth.json` | `CODEX_HOME` |
  | Claude Code | `%USERPROFILE%\.claude\.credentials.json` | `CLAUDE_CONFIG_DIR` |
  | Grok | `%USERPROFILE%\.grok\auth.json` | — |

仅在 WSL 中登录的凭据不会自动发现。环境变量须对运行应用的 Windows 进程可见。
应用配置位于 `%APPDATA%\com.stigachen.agentusage\config.json`。
免安装不代表数据随程序目录移动；换电脑仍需重新登录。
更新时从托盘退出应用，再覆盖原目录中的 `.exe`。若开启 Launch at login，需保持程序路径；
移动或删除前先关闭该选项，移动后可重新开启。

PR 和 `main` 的 CI 会在 macOS / Windows 上运行检查，在 Windows 上解压并启动便携程序，
再上传 `agent-usage-windows-x64-portable` 供测试；也可手动运行 CI，无需发布版本。
首版 Windows 程序尚未配置代码签名，系统可能显示未知发布者或 SmartScreen 提示。

## 工程结构

- `src/routes/+page.svelte`：托盘面板入口、刷新事件、动态窗口高度。
- `src/lib/`：总览、Provider 卡片、设置和拖拽排序。
- `src-tauri/src/lib.rs`：Tauri 命令、后台刷新、托盘和窗口生命周期。
- `src-tauri/src/providers/`：统一 Provider 接口，以及各家的认证发现和用量请求。
- `src-tauri/src/store.rs`：非敏感配置写入 JSON，token 交给系统凭据存储。
- `.github/workflows/`：跨平台检查、Windows 便携测试包，以及正式发布流程。
- `scripts/package-windows.ps1`：将 Windows 可执行文件和使用说明打包为 ZIP。

## 发布

见 [docs/RELEASING.md](docs/RELEASING.md)。
