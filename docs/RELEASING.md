# Releasing

GitHub Actions (`.github/workflows/release.yml`) produces a signed, notarized
universal macOS build and an unsigned Windows x64 NSIS installer, attaching both
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
   The Windows job then builds an x64 `-setup.exe` and uploads it to that exact
   draft. Wait for both jobs before publishing.
5. Check the draft on the [Releases page](../../releases), test the Windows
   installer using the checklist below, edit the notes, then publish:
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

### Windows acceptance

Download the `*-setup.exe` release asset. For changes before a release, download
`agent-usage-windows-x64` from the CI workflow artifacts (PRs, `main`, or manual
dispatch). CI builds and tests Windows independently of Apple's signing secrets.

Test on Windows 10 and Windows 11 before calling the Windows version stable:

- Install as a standard user, launch from Start, and verify the tray icon in
  light/dark themes and the hidden-icons overflow. Check WebView2 setup on a
  machine without the runtime (network required).
- Left-click to show/hide; right-click to open, refresh, and quit. Expand cards
  and Settings near a screen edge, on multiple monitors, and at 100% / 150% /
  200% display scaling. Check that the panel remains visible.
- Check lowest/pinned/icon-only preferences in the tooltip. Switch the system
  theme while the app is running.
- Sign in to Copilot and open the billing token creation link; confirm both
  query parameters reach the browser. Restart to verify credential persistence.
- Verify native Windows CLI discovery for Codex, Claude Code, and Grok. WSL
  credentials are not automatically discovered; Claude remains unverified until
  tested against a real account.
- Enable/disable Launch at login and sign back into Windows. Start the app twice
  to verify a single tray icon; Alt+F4 should hide the panel, and Quit should exit.
- Upgrade an existing install and uninstall; verify shortcuts and installer behavior.

The Windows installer is currently **unsigned** and may show SmartScreen or an
unknown-publisher prompt. Windows code signing must be configured separately
before distributing a signed build; the Apple secrets do not sign Windows.

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
  `pnpm tauri build --target x86_64-pc-windows-msvc --bundles nsis` produces
  `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/*-setup.exe`.
  The Windows-specific config is merged automatically. It uses a current-user
  install and downloads WebView2 when missing.

See [Tauri's Windows installer documentation](https://v2.tauri.app/distribute/windows-installer/)
for installer and signing options. Prefer a Windows machine or the CI runner for
Windows packaging; a successful macOS build does not validate Windows behavior.
