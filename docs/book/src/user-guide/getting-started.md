# Getting started

## Prerequisites

- **Rust toolchain.** SupaZip targets MSRV 1.92. Install via [rustup](https://rustup.rs/):

  ```bash
  rustup toolchain install 1.92
  ```

  The repository includes a `rust-toolchain.toml` that pins the channel.

- **Platform.** Windows (x64), Linux (x64), macOS (x64 and arm64). CI tests all four.

## Building

```bash
cd supazip
cargo build --release
```

This produces three binaries:

| Binary | Path |
|--------|------|
| CLI | `target/release/supazip` |
| GUI | `target/release/supazip-gui` |

## First commands

### List archive contents

```bash
supazip list photos.zip
```

Output is a fixed-width table with columns: `IDX`, `METHOD`, `SIZE`, `COMPRESSED`, `CRYPT`, `NAME`.

### Extract to a directory

```bash
supazip extract photos.zip --out ./photos
```

### Create a new archive

```bash
supazip create backup.7z report.pdf data.csv
```

The format is inferred from the file extension. Use `--format zip` to override.

### Test integrity

```bash
supazip test backup.7z
```

Prints `OK: <path>` on success, exits non-zero on failure.

## GUI quick start

Launch the graphical application:

```bash
supazip-gui
```

1. Click **Open…** in the toolbar (or press `Ctrl+O`).
2. Select an archive file (`.zip`, `.7z`, `.tar`, `.tar.gz`, `.tar.xz`).
3. Browse entries in the striped grid.
4. Right-click an entry for context-menu actions (Extract here, Extract to, Copy path).
5. Use **Extract** to unpack the full archive, **Test** to verify integrity, **Create…** to build a new archive.

## Environment variables

| Variable | Effect |
|----------|--------|
| `SUPAZIP_MAX_ARCHIVE_SIZE` | Override the maximum archive size limit. Accepts decimal bytes or `K`/`M`/`G` suffix (e.g. `512M`). |
| `SUPAZIP_LOG` | Set the log level for the CLI (`trace`, `debug`, `info`, `warn`, `error`). Defaults to `info`. |
| `SUPAZIP_LANG` | Override the GUI language (`en`, `ru`). Defaults to `en`. |

## Shell completions

Generate shell completions for your shell:

```bash
# Bash
supazip completions bash > /etc/bash_completion.d/supazip

# Zsh
supazip completions zsh > ~/.zfunc/_supazip

# Fish
supazip completions fish > ~/.config/fish/completions/supazip.fish

# PowerShell
supazip completions powershell > supazip.ps1

# Elvish
supazip completions elvish > supazip.elv
```
