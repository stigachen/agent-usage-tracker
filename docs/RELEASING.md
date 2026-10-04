# Releasing

GitHub Actions (`.github/workflows/release.yml`) produces a signed, notarized
universal macOS build and an unsigned Windows x64 portable ZIP, attaching both
to the same draft release. Nothing needs to be signed locally.

## Cut a release

1. Bump `version` in `src-tauri/tauri.conf.json`, `package.json`, and
   `src-tauri/Cargo.toml`; run `cargo check --manifest-path src-tauri/Cargo.toml`
   to update the app version in `src-tauri/Cargo.lock`.
2. Open a PR with the version bump and merge it (`main` only accepts changes via PR).
3. Update local `main`, then tag and push:
   ```bash
   git checkout main && git pull
   git tag v0.1.1
   git push origin v0.1.1
   ```
4. The macOS job builds a universal (Intel + Apple Silicon) app,
   signs it with the Developer ID certificate, notarizes and staples both the
   `.app` and the `.dmg`, then attaches them to a **draft** GitHub Release.
   The Windows job then builds an x64 executable and uploads
   `Agent.Usage_<version>_windows_x64_portable.zip` to that exact draft.
   Wait for both jobs before publishing.
5. Check the draft on the [Releases page](../../releases), test the Windows
   portable build using the checklist below, edit the notes, then publish:
   ```bash
   gh release edit v0.1.1 --draft=false --notes "..."
   ```

The workflow can also be started by hand from the Actions tab (`workflow_dispatch`);
that produces a draft tagged `v<version>-<run number>`.

## Verify a build

```bash
gh release download v0.1.1 -p "*.dmg"
xcrun stapler validate Agent.Usage_*.dmg
spctl -a -vv -t open --context context:primary-signature Agent.Usage_*.dmg
```

Both should report `accepted` / `source=Notarized Developer ID`.

### macOS panel regression checks

With menu bars available on two displays, open and dismiss the panel on display
A, then open it from the menu bar icon on display B. Check that it stays beside
that icon, including after expanding cards or opening Settings. Repeat in both
directions and with different display scales. This checks native asynchronous
window movement; the pure geometry tests do not exercise that event ordering.

### Windows acceptance

Download the `*_windows_x64_portable.zip` release asset. For changes before a
release, download `agent-usage-windows-x64-portable` from the CI workflow artifacts
(PRs, `main`, or manual dispatch). Extract the ZIP and run `Agent Usage.exe`.
CI builds and tests Windows independently of Apple's signing secrets. It also
extracts the release ZIP to a separate folder and checks that the executable
stays running during startup, without running an installer. This is a startup
smoke test, not validation of tray interaction or provider logins.

Test on Windows 10 and Windows 11 before calling the Windows version stable:

- Extract as a standard user and double-click `Agent Usage.exe`, including from
  a path containing spaces. Verify the tray icon in light/dark themes and the
  hidden-icons overflow. No application install, elevation, or Start shortcut
  is needed. WebView2 Runtime must already be present; the ZIP does not install it.
- Left-click to show/hide; right-click to open, refresh, and quit. Expand cards
  and Settings near a screen edge, on multiple monitors, and at 100% / 150% /
  200% display scaling. Open a tall panel from the hidden-icons overflow and
  check that it stays within the work area, clear of bottom/top/side taskbars.
- Check lowest/pinned/icon-only preferences in the tooltip. Switch the system
  theme while the app is running.
- Open both Settings dropdowns (Show and Refresh every) in light and dark mode,
  including after changing the system theme without restarting. All options,
  including unselected accounts, must stay readable. Check mouse selection and
  keyboard navigation, and repeat with Windows contrast themes enabled.
- Sign in to Copilot and open the billing token creation link; confirm both
  query parameters reach the browser. Restart to verify credential persistence.
- Verify native Windows CLI discovery for Codex, Claude Code, and Grok. WSL
  credentials are not automatically discovered; Claude remains unverified until
  tested against a real account.
- Enable/disable Launch at login and sign back into Windows. Start the app twice
  to verify a single tray icon; Alt+F4 should hide the panel, and Quit should exit.
- Minimize the panel through the system menu, then restore it by left-clicking
  the tray, selecting Open, and launching the app again. Each path should restore
  and focus the panel without needing a taskbar button.
- Quit and replace the executable in the same folder; verify settings and tokens
  persist. Disable Launch at login before moving or deleting the executable, then
  re-enable it from the new location if needed. Copying the program to another
  computer does not copy settings or credentials.

The Windows executable is currently **unsigned** and may show SmartScreen or an
unknown-publisher prompt. Windows code signing must be configured separately
before distributing a signed build; the Apple secrets do not sign Windows.

The app uses the system's Evergreen WebView2 Runtime. Windows 11 and most
Windows 10 systems already include it; on systems without it, install Microsoft's
[WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/#download-section)
separately. See Microsoft's [runtime distribution documentation](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution).
The Visual C++ runtime is statically linked and the MSVC WebView2 loader is linked
into the executable, so the portable ZIP needs no additional app DLLs.

## Redo a failed release

```bash
gh release delete v0.1.1 -y
git tag -f v0.1.1 && git push -f origin v0.1.1
```

## Required repository secrets

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | base64 of the Developer ID Application `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | password set when exporting the `.p12` |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: Guang Chen (P9P5K7M2C4)` |
| `APPLE_ID` | Apple ID email used for notarization |
| `APPLE_PASSWORD` | app-specific password for that Apple ID |
| `APPLE_TEAM_ID` | `P9P5K7M2C4` |

To rotate the certificate: in Keychain Access, export the certificate with its
private key as `.p12`, then

```bash
base64 -i cert.p12 | gh secret set APPLE_CERTIFICATE
gh secret set APPLE_CERTIFICATE_PASSWORD
rm cert.p12
```

App-specific passwords are created at <https://account.apple.com> → Sign-In and Security.

## Local builds

- macOS: `pnpm tauri build --bundles app` produces an unsigned app in
  `src-tauri/target/release/bundle/macos/`.
- Windows (PowerShell, with Tauri's C++/MSVC prerequisites installed):
  `pnpm tauri build --target x86_64-pc-windows-msvc --no-bundle` produces
  `src-tauri/target/x86_64-pc-windows-msvc/release/agent-usage-tracker.exe`.
  Run `./scripts/package-windows.ps1` from the repository root to create
  `src-tauri/target/Agent.Usage_<version>_windows_x64_portable.zip`, containing
  `Agent Usage.exe` and `README.txt`. The Windows-specific config disables
  installer bundling and explicitly enables static Visual C++ runtime linking.

Use a Windows machine or the CI runner for Windows builds and packaging;
a successful macOS build does not validate Windows behavior.
