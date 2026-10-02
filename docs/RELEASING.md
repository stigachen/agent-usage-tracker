# Releasing

macOS builds are signed, notarized and published by GitHub Actions
(`.github/workflows/release.yml`). Nothing needs to be signed locally.

## Cut a release

1. Bump `version` in `src-tauri/tauri.conf.json` (and `package.json` to keep them in sync).
2. Open a PR with the version bump and merge it (`main` only accepts changes via PR).
3. Update local `main`, then tag and push:
   ```bash
   git checkout main && git pull
   git tag v0.1.1
   git push origin v0.1.1
   ```
4. The workflow (about 6 minutes) builds a universal (Intel + Apple Silicon) app,
   signs it with the Developer ID certificate, notarizes and staples both the
   `.app` and the `.dmg`, then attaches them to a **draft** GitHub Release.
5. Check the draft on the [Releases page](../../releases), edit the notes, then publish:
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

`pnpm tauri build --bundles app` produces an unsigned app in
`src-tauri/target/release/bundle/macos/`. Good for testing, not for distribution.
