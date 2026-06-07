# SupaZip release process

This document is the runbook for cutting a SupaZip release through GitHub
Actions. It covers the **binary distribution** path (the
`.github/workflows/release.yml` pipeline); the crates.io path is documented
separately in [`docs/publishing.md`](publishing.md).

The pipeline emits a draft GitHub Release with:

- the `supazip-cli` binary for 5 targets:
  `x86_64-pc-windows-msvc`, `x86_64-unknown-linux-gnu`,
  `x86_64-apple-darwin`, `aarch64-apple-darwin`,
  `aarch64-unknown-linux-gnu` (cross-compiled),
- a `SHA256SUMS` file with one line per binary,
- a `SHA256SUMS.minisign` detached signature over the SHA-256 file,
- a CycloneDX `supazip-cli.spdx.json` SBOM generated from `Cargo.lock`.

The release is **draft** — a maintainer reviews the notes, the SHA-256
list, the signature, and the SBOM, then clicks *Publish* in the GitHub
Releases UI. Nothing is auto-published.

> **Status (1.0-rc.1, WS-H):** the pipeline is finalized with 5 build
> targets, SBOM generation, and minisign signing. It is exercised on
> every `v*.*.*` tag push.

## Release pipeline overview

```text
tag vX.Y.Z pushed
  └─► build job (matrix × 5 targets)
        ├─ x86_64-pc-windows-msvc     (windows-latest)
        ├─ x86_64-unknown-linux-gnu   (ubuntu-latest)
        ├─ x86_64-apple-darwin        (macos-latest)
        ├─ aarch64-apple-darwin       (macos-latest)
        └─ aarch64-unknown-linux-gnu  (ubuntu-latest, cross)
        each uploads binary as workflow artifact
  └─► sign job (depends on build)
        ├─ download all binaries → dist/
        ├─ sha256sum → SHA256SUMS
        ├─ minisign sign → SHA256SUMS.minisig
        ├─ anchore/sbom-action → dist/supazip-cli.spdx.json
        └─ softprops/action-gh-release → draft GitHub Release
  └─► maintainer reviews draft → clicks Publish
  └─► post-release: verify checksums, signature, SBOM, smoke-test
```

## Pre-release checklist

Tick every box before pushing a `v*.*.*` tag.

- [ ] All `windows-latest` and `ubuntu-latest` CI jobs green on
      `master` (see `docs/CONTRIBUTING.md` → "Before opening a PR").
- [ ] `CHANGELOG.md` has a fresh `## [X.Y.Z] - YYYY-MM-DD` section at
      the top; the `[Unreleased]` content moved into it.
- [ ] `Cargo.lock` is committed.
- [ ] `memory-bank/decisionLog.md` reflects any decision taken since
      the previous release.
- [ ] The `assets/minisign.pub` public key has been **rotated** if the
      signing key changed since the last release.
- [ ] `secrets.MINISIGN_SECRET_KEY`, `secrets.MINISIGN_PASSPHRASE`, and
      `vars.SUPAAZIP_MINISIGN_PUB` are present in
      `Settings → Secrets and variables → Actions` (see "Secrets and
      variables" below).
- [ ] The maintainer who will publish has `Maintain` or `Admin` on the
      repository, so the `contents: write` permission in
      `release.yml` is honoured.

Pushing the tag:

```bash
git tag vX.Y.Z            # exact commit that bumped Cargo.toml + CHANGELOG
git push origin vX.Y.Z    # triggers release.yml
```

## Signing key setup (one-time, then rotate per release-train)

`minisign` is the signing tool of choice for 0.3.0 — small, single-binary,
ISC-licensed, the de-facto standard for archive releases (decision
recorded in
[`docs/milestones/m0.3.0-gui-feature-complete.md`](milestones/m0.3.0-gui-feature-complete.md) →
Open question #4). The release workflow reads the key from
`secrets.MINISIGN_SECRET_KEY`; the public half is checked in to
`assets/minisign.pub`.

### Generate a keypair

```bash
# On a maintainer's laptop (offline, ideally).
minisign -G -p minisign.pub -s minisign.key -W
# -W   prompt for a passphrase (recommended).
# -p   write the public key to minisign.pub (committed to the repo).
# -s   write the secret key to minisign.key (NEVER committed).
```

`minisign -G` prints the **key number** (8 base64 characters on the
second line of `minisign.pub`). That key number is the fingerprint
referenced in the README's "Verifying releases" section.

### Repository layout after setup

- `assets/minisign.pub` — committed public key, served verbatim by the
  pipeline's release notes.
- Local `minisign.key` (NOT committed) — pasted into
  `secrets.MINISIGN_SECRET_KEY`. The file is a plain ASCII base64
  blob, multi-line; the workflow reads it from the secret verbatim and
  writes it to a temp file on the runner, `chmod 600`, signs, then
  removes it.

### Rotation

Rotate the key per release-train (every minor version, at most). A
rotation is a single PR that swaps `assets/minisign.pub`; old releases
keep verifying against the key fingerprint they shipped with. The
README's "Verifying releases" section lists every historical key
fingerprint with the version range it covers.

## Secrets and variables

Configure once in the GitHub UI: *Settings → Secrets and variables →
Actions*.

| Name                       | Type    | Source                                         | Used by               |
|----------------------------|---------|------------------------------------------------|-----------------------|
| `MINISIGN_SECRET_KEY`      | Secret  | `cat minisign.key`                             | `sign` step in `release.yml` |
| `MINISIGN_PASSPHRASE`      | Secret  | The passphrase entered at `minisign -G -W`     | `sign` step in `release.yml` |
| `SUPAAZIP_MINISIGN_PUB`    | Variable| `cat assets/minisign.pub`                      | Future "Verifying releases" badge / release notes link |
| `GITHUB_TOKEN`             | Secret  | (automatic)                                    | `softprops/action-gh-release`, `actions/upload-artifact` |

Never echo `MINISIGN_SECRET_KEY` or `MINISIGN_PASSPHRASE` from a step —
`set -euo pipefail` is set in the sign step, and the key file is
`rm`'d immediately after `minisign` returns. The risk that the key
leaks through runner logs is logged in the 0.3.0 Risk register as
#4 ("minisign secret key leaks through the GitHub Actions logs or
runner cache"), with the present mitigation.

## Verifying a release (downstream users)

```bash
# 1. Download the artefacts for a release tag (e.g. v0.3.0).
gh release download v0.3.0 -D supazip-v0.3.0
cd supazip-v0.3.0

# 2. Verify the signature against the checked-in public key.
minisign -Vm SHA256SUMS.minisig -P "$(curl -fsSL https://raw.githubusercontent.com/<owner>/supazip/main/assets/minisign.pub)"

# 3. Verify the per-binary checksums.
sha256sum --ignore-missing -c SHA256SUMS

# 4. Validate the SBOM (requires cyclonedx-cli).
cyclonedx-cli validate --input-file supazip-cli.spdx.json --input-format json

# 5. (Optional) Smoke-test the CLI on your platform.
./supazip-cli-x86_64-unknown-linux-gnu --version
```

Equivalent one-liner against a local clone:

```bash
minisign -Vm SHA256SUMS.minisig -P "$(cat ../assets/minisign.pub)"
```

The `-P` flag embeds the public key inline; the `-V` flag verifies the
detached signature against `SHA256SUMS`. A passing run prints:

```
Signature and comment signature verified
Trusted comment: SupaZip release vX.Y.Z
```

A failing run prints the offending line and exits non-zero — do **not**
run the binary in that case.

### Reproducible build verification

For platforms where `scripts/build-reproducible.sh` is available:

```bash
# Clone the tagged commit, build, compare checksums.
git checkout v0.3.0
scripts/build-reproducible.sh
# Compare the locally-built binary against the release artefact:
sha256sum supazip-cli-<target>
# The hash must match the corresponding line in SHA256SUMS.
```

If the hashes do not match, the binary was not built from the exact
tagged commit, or the build environment differs. File an issue.

## Why minisign and not GPG / cosign

| Tool      | Why not (for 0.3.0)                                                                                  |
|-----------|------------------------------------------------------------------------------------------------------|
| **GPG**   | Heavy web-of-trust, harder to rotate, the keyserver ecosystem is brittle. Pinned in the milestone plan as a fallback if minisign integration proves fragile. |
| **cosign** (sigstore) | OIDC-based, but requires a transparency-log entry per signature and a Fulcio / Rekor URL at verification time. Defensible once the project publishes OIDC provenance in 1.0-rc.1; overkill for the 0.3.0 release where the chain of trust is a single GitHub Release + a checked-in public key. |
| **minisign** | Single ~1 MB binary, no keyserver, no CA, signing and verification are two CLI invocations. The standard for `*.tar.gz` releases in the wider open-source ecosystem (e.g. `sudo`, `borgbackup`). |

If `minisign` integration in the workflow becomes a maintenance
burden, the fallback path is `crazy-max/ghaction-import-gpg@v6` +
`gpg --detach-sign --armor SHA256SUMS`, which would emit a
`SHA256SUMS.asc` instead of `SHA256SUMS.minisig`. The downstream recipe
in "Verifying a release" is the only piece that changes.

## Post-release

After the maintainer publishes the draft:

1. Verify the released artefacts on each target platform:

   ```bash
   ./supazip-cli-x86_64-unknown-linux-gnu --version
   ./supazip-cli-aarch64-unknown-linux-gnu --version
   ./supazip-cli-x86_64-apple-darwin --version
   ./supazip-cli-aarch64-apple-darwin --version
   supazip-cli-x86_64-pc-windows-msvc.exe --version
   ```

2. Smoke-test `list` on the in-tree fixture:

   ```bash
   ./supazip-cli-x86_64-unknown-linux-gnu list supazip/supazip-core/tests/fixtures/sample.zip
   ```

3. Sanity-check the SBOM by re-running `cyclonedx-cli validate`:

   ```bash
   cyclonedx-cli validate --input-file supazip-cli.spdx.json --input-format json
   ```

4. Confirm the public-key fingerprint in
   `assets/minisign.pub` matches the fingerprint published in the
   release notes and the README's "Verifying releases" section.

5. (Optional) Verify reproducibility — build from the tagged commit
   and compare SHA-256 hashes against the release `SHA256SUMS`:

   ```bash
   git checkout <tag>
   scripts/build-reproducible.sh
   sha256sum supazip-cli-<target>
   # Must match the corresponding line in SHA256SUMS.
   ```

## Open questions (inherited from 0.3.0, resolved by 1.0-rc.1)

The following items were tracked from the 0.3.0 milestone and are
addressed or superseded as of the 1.0-rc.1 (WS-H) work:

- **CI smoke job for signature verification** — deferred to 1.0.0
  milestone. A `continue-on-error: true` job that re-verifies
  `SHA256SUMS.minisig` on every push to `master` is planned but not
  yet implemented.
- **`script/check-minisign-pub.sh`** — deferred to 1.0.0. Validates
  the public key file format and key-number cross-reference.
- **Migrate to cosign / sigstore** — re-evaluated for 1.0.0. The
  current minisign setup is sufficient for the 1.0-rc.1 release
  pipeline. OIDC provenance via cosign is tracked as a future
  enhancement if the project adopts GitHub's artifact attestations.
