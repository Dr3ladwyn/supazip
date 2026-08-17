# SupaZip — Package Manager Manifests

This directory contains manifests and formulae for publishing SupaZip CLI
through five package managers.

**GitHub identity placeholder:** `your-org` (repository `your-org/supazip`).
Use this one string in every URL until a real GitHub account or organisation
exists. Do not invent a handle, and do not mix `<owner>`, `your-org`, and
`supazip/supazip`.

**SHA-256 hashes** in these manifests stay `PLACEHOLDER` until they are
filled from the first GitHub Release assets. Do not guess hashes.

---

## 1. Homebrew (macOS + Linux)

**File:** `homebrew/supazip.rb`

### Steps to publish

1. Fork [homebrew-core](https://github.com/Homebrew/homebrew-core).
2. Replace `your-org` in the formula with the real GitHub handle.
3. Download each release tarball and compute SHA-256:
   ```
   shasum -a 256 supazip-cli-x86_64-apple-darwin.tar.gz
   ```
4. Replace every `PLACEHOLDER` hash in the formula (after the first GitHub
   Release).
5. Create a branch, commit, and open a PR to `homebrew-core`.
6. After merge, users install with:
   ```
   brew install supazip
   ```

### Updating the version

Bump `version`, update download URLs, recompute SHA-256 hashes, open a new PR.

### References

- [Homebrew Formula Cookbook](https://docs.brew.sh/Formula-Cookbook)
- [Acceptable Formulae](https://docs.brew.sh/Acceptable-Formulae)

---

## 2. AUR (Arch Linux)

**File:** `aur/PKGBUILD`

### Steps to publish

1. Create an AUR account at <https://aur.archlinux.org>.
2. Replace `your-org`, `<your name>`, and `<your email>` in the PKGBUILD.
3. Download the source tarball from GitHub and compute SHA-256:
   ```
   sha256sum supazip-1.0.0.tar.gz
   ```
4. Replace `PLACEHOLDER` in `sha256sums` after the first GitHub Release.
5. Build and test locally:
   ```
   makepkg -si
   ```
6. Generate `.SRCINFO` and push to AUR:
   ```
   makepkg --printsrcinfo > .SRCINFO
   git init && git add PKGBUILD .SRCINFO
   git commit -m "supazip 1.0.0"
   git remote add origin ssh://aur@aur.archlinux.org/supazip.git
   git push -u origin master
   ```
7. Users install with:
   ```
   yay -S supazip        # or any AUR helper
   ```

### Updating the version

Bump `pkgver`, update `sha256sums`, regenerate `.SRCINFO`, push to AUR.

### References

- [AUR Submission Guidelines](https://wiki.archlinux.org/title/AUR_submission_guidelines)
- [PKGBUILD reference](https://wiki.archlinux.org/title/PKGBUILD)

---

## 3. winget (Windows)

**File:** `winget/SupaZip.SupaZip.yaml`

### Steps to publish

1. Replace `your-org` in the manifest with the real GitHub handle.
2. Download the release zip and compute SHA-256:
   ```
   Get-FileHash supazip-cli-x86_64-pc-windows-msvc.exe.zip -Algorithm SHA256
   ```
3. Replace `PLACEHOLDER` with the hash after the first GitHub Release.
4. Validate the manifest:
   ```
   winget validate --manifest packaging/winget
   ```
5. Submit to [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs):
   - Fork the repo.
   - Place the manifest under `manifests/s/SupaZip/SupaZip/1.0.0/`.
   - Open a PR.
6. Users install with:
   ```
   winget install SupaZip
   ```

### Updating the version

Create a new manifest directory with the updated version, hashes, and URLs.

### References

- [winget manifest spec](https://github.com/microsoft/winget-cli/blob/master/doc/ManifestSpecv1.0.md)
- [wingetcreate CLI](https://github.com/microsoft/winget-create)

---

## 4. Scoop (Windows)

**File:** `scoop/supazip.json`

### Steps to publish

1. Replace `your-org` in the JSON manifest with the real GitHub handle.
2. Download the release zip and compute SHA-256:
   ```
   Get-FileHash supazip-cli-x86_64-pc-windows-msvc.exe.zip -Algorithm SHA256
   ```
3. Replace `PLACEHOLDER` in the `"hash"` field after the first GitHub Release.
4. Test locally:
   ```
   scoop install ./packaging/scoop/supazip.json
   ```
5. Publish to a custom bucket or submit to the `main` bucket:
   - Fork [scoopInstaller/Main](https://github.com/ScoopInstaller/Main).
   - Add `supazip.json` to `bucket/`.
   - Open a PR.
6. Users install with:
   ```
   scoop bucket add supazip <bucket-url>
   scoop install supazip
   ```

### Updating the version

Scoop's `autoupdate` block handles URL templates. After a release, update the
`version` field and `hash`; the `checkver` block can auto-detect new GitHub
releases.

### References

- [Scoop App Manifest reference](https://github.com/ScoopInstaller/Scoop/wiki/App-Manifests)
- [Scoop Buckets](https://github.com/ScoopInstaller/Scoop/wiki/Buckets)

---

## 5. Nix (NixOS + nix-darwin)

**File:** `nix/flake.nix`

### Steps to publish

1. Replace `your-org` references if building from a fork.
2. Test locally from the repo root:
   ```
   nix build ./packaging/nix#supazip
   ```
3. To run directly:
   ```
   nix run ./packaging/nix#supazip -- --version
   ```
4. To publish as an overlay, add to the flake outputs:
   ```nix
   overlays.default = final: prev: {
     supazip = self.packages.${final.system}.supazip;
   };
   ```
5. Users add SupaZip to their flake inputs or install with:
   ```
   nix profile install github:your-org/supazip#supazip
   ```

### Updating the version

Bump `version` in the flake. If `Cargo.lock` changes, Nix picks it up
automatically via `cargoLock.lockFile`.

### References

- [Nix Flakes guide](https://nixos.wiki/wiki/Flakes)
- [buildRustPackage docs](https://nixos.org/manual/nixpkgs/stable/#compiling-rust-applications-with-cargo)

---

## General release checklist

1. Tag the release: `git tag v1.0.0 && git push --tags`.
2. Create a GitHub Release with binary assets attached
   (`.tar.gz` on Unix, `.zip` on Windows — see `.github/workflows/release.yml`).
3. Compute SHA-256 for every asset:
   ```
   shasum -a 256 supazip-cli-*
   ```
4. Fill `PLACEHOLDER` hashes in all five manifests from those assets
   (after the first GitHub Release).
5. Replace `your-org` with the actual GitHub handle in all files.
6. Submit PRs / push to each package manager registry (see sections above).
7. Verify each installation path works on the target platform.
