# macOS Code-Signing & Notarization

This document describes how SupaZip's macOS binaries are code-signed and
notarized in CI, and how to reproduce the process locally.

> **Status**: integrated into the `release.yml` workflow (WS-H, milestone
> 0.4.0).  Signing is optional — if the required secrets are not configured,
> the build falls back to an unsigned binary (see [Fallback](#fallback)
> below).

---

## Pre-requisites

| Requirement | Details |
|-------------|---------|
| **Apple Developer Account** | Paid membership at <https://developer.apple.com>. |
| **Developer ID certificate** | Type **"Developer ID Application"** — created in the Apple Developer portal or Xcode. |
| **Certificate + private key** | Exported as a `.p12` file with a password. |

The `.p12` and its password are stored as CI secrets (see next section).

---

## CI Secrets

| Secret | Content |
|--------|---------|
| `APPLE_CERTIFICATE` | Base64-encoded `.p12` file (`base64 -i cert.p12`). |
| `APPLE_CERTIFICATE_PASSWORD` | Password used when exporting the `.p12`. |
| `APPLE_ID` | Apple-ID email used for notarization. |
| `APPLE_ID_PASSWORD` | App-specific password (not your iCloud password).  Generate one at <https://appleid.apple.com/account/manage>. |
| `APPLE_TEAM_ID` | 10-character team identifier (visible in the Apple Developer portal). |

---

## Import the Certificate

```bash
# Decode the base64 secret into a .p12 file
echo "$APPLE_CERTIFICATE" | base64 -d > cert.p12

# Create an ephemeral keychain for CI
security create-keychain -p "" build.keychain
security import cert.p12 \
  -k build.keychain \
  -P "$APPLE_CERTIFICATE_PASSWORD" \
  -T /usr/bin/codesign
security set-keychain-settings build.keychain
security unlock-keychain -p "" build.keychain

# Find the certificate common-name
CERT_NAME=$(security find-identity -v build.keychain \
  | grep "Developer ID" | head -1 \
  | sed 's/.*"\(.*\)"/\1/')
```

> **Note**: The ephemeral `build.keychain` is deleted at the end of the
> signing step to avoid leaking credentials.

---

## Sign

```bash
codesign --deep --force --options runtime \
  --sign "$CERT_NAME" \
  target/release/supazip-cli
```

Flags:

- `--deep` — sign nested code (dynamic libraries, helpers).
- `--force` — re-sign if already signed.
- `--options runtime` — enable the Hardened Runtime, required for
  notarization.
- `--sign "$CERT_NAME"` — the "Developer ID Application: …" identity.

---

## Notarize

Submit the signed binary to Apple's notarization service and wait for
approval:

```bash
xcrun notarytool submit target/release/supazip-cli \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_ID_PASSWORD" \
  --team-id "$TEAM_ID" \
  --wait
```

`--wait` blocks until Apple completes the scan (typically 1-5 min).
If notarization fails, inspect the log:

```bash
xcrun notarytool log <submission-id> \
  --apple-id "$APPLE_ID" \
  --password "$APPLE_ID_PASSWORD" \
  --team-id "$TEAM_ID"
```

---

## Staple

Attach the notarization ticket to the binary so it can be verified
offline:

```bash
xcrun stapler staple target/release/supazip-cli
```

---

## Verify

Confirm the signature and Gatekeeper acceptance:

```bash
# Detailed signature info
codesign -dv --verbose=4 target/release/supazip-cli

# Gatekeeper assessment
spctl -a -v target/release/supazip-cli
# Expected: "source=Notarized Developer ID"
```

---

## CI Integration

The signing step lives in `.github/workflows/release.yml`, inside the
`build` job.  It runs **only** on macOS runners **and** only when the
`APPLE_CERTIFICATE` secret is present:

```yaml
- name: macOS sign + notarize
  if: runner.os == 'macOS' && env.APPLE_CERTIFICATE != ''
  env:
    APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
    APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
    APPLE_ID: ${{ secrets.APPLE_ID }}
    APPLE_ID_PASSWORD: ${{ secrets.APPLE_ID_PASSWORD }}
    APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
  shell: bash
  working-directory: supazip
  run: |
    echo "$APPLE_CERTIFICATE" | base64 -d > cert.p12
    security create-keychain -p "" build.keychain
    security import cert.p12 -k build.keychain \
      -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
    security set-keychain-settings build.keychain
    security unlock-keychain -p "" build.keychain
    CERT_NAME=$(security find-identity -v build.keychain \
      | grep "Developer ID" | head -1 \
      | sed 's/.*"\(.*\)"/\1/')
    BIN="artifacts/supazip-cli-${{ matrix.target }}"
    codesign --deep --force --options runtime --sign "$CERT_NAME" "$BIN"
    xcrun notarytool submit "$BIN" \
      --apple-id "$APPLE_ID" \
      --password "$APPLE_ID_PASSWORD" \
      --team-id "$APPLE_TEAM_ID" --wait
    xcrun stapler staple "$BIN"
    rm -f cert.p12 build.keychain
```

The step is placed **after** `Package` and **before** `Upload artifacts`,
so the signed binary is what gets published in the release.

---

## Fallback — No Apple Developer Account

If the secrets are not configured the signing step is skipped entirely.
The binary ships unsigned and users will see a Gatekeeper warning on
first launch.

As a lightweight alternative for local or ad-hoc builds:

```bash
# Ad-hoc signature (no identity, no notarization)
codesign --force --deep - target/release/supazip-cli
```

An ad-hoc signature suppresses some warnings but does **not** satisfy
Gatekeeper on macOS 10.15+.  Users would need to right-click → Open or
run `xattr -cr /path/to/supazip-cli`.

---

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `errSecInternalComponent` during import | Keychain locked | `security unlock-keychain -p "" build.keychain` |
| `no identity found` | Certificate not in keychain | Verify `.p12` contains the "Developer ID Application" cert |
| Notarization rejected | Binary has unsigned dynamic libs | Ensure `--deep` is used; check `otool -L` output |
| `spctl` says "rejected" | Not stapled or not notarized | Run `xcrun stapler staple` again; check notarization log |
| CI step skipped | `APPLE_CERTIFICATE` secret not set | Add the secret in repo Settings → Secrets |
