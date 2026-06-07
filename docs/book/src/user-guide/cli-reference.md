# CLI reference

The `supazip` binary exposes five subcommands: `list`, `extract`, `create`, `test`, and `completions`.

## Global flags

| Flag | Description |
|------|-------------|
| `--output <format>` | Output format: `text` (default), `json`, `yaml`. Applies to `list` and `test`. |
| `--help` | Print help for the current subcommand. |
| `--version` | Print the version string. |

## `list`

List the contents of an archive.

```
supazip list <ARCHIVE> [--password <PASSWORD>]
```

| Argument / Flag | Required | Description |
|-----------------|----------|-------------|
| `<ARCHIVE>` | Yes | Path to the archive file. |
| `--password <PASSWORD>` | No | Password for encrypted archives. |

**Output columns (text mode):**

| Column | Width | Alignment | Description |
|--------|-------|-----------|-------------|
| `IDX` | 4 | Right | 0-based entry index |
| `METHOD` | 12 | Left | Compression method (deflate, store, lzma2, etc.) |
| `SIZE` | 12 | Right | Uncompressed size (B / KiB / MiB / GiB) |
| `COMPRESSED` | 12 | Right | Compressed size |
| `CRYPT` | 8 | Left | `yes` if encrypted, `-` otherwise |
| `NAME` | Rest | Left | Entry path within the archive |

**Example:**

```
$ supazip list project.zip
  IDX  METHOD         SIZE       COMPRESSED  CRYPT    NAME
------------------------------------------------------------------------
    0  deflate        4.2 KiB       1.1 KiB  -        src/main.rs
    1  deflate        0.8 KiB         312 B  -        Cargo.toml

2 entries
```

## `extract`

Extract an archive to a directory.

```
supazip extract <ARCHIVE> [--out <DIR>] [--password <PASSWORD>] [--all] [--entry <NAME>]...
```

| Argument / Flag | Required | Default | Description |
|-----------------|----------|---------|-------------|
| `<ARCHIVE>` | Yes | — | Path to the archive file. |
| `--out <DIR>` | No | `.` (current directory) | Destination directory. Created if it does not exist. |
| `--password <PASSWORD>` | No | — | Password for encrypted archives. |
| `--all` | No | `true` | Extract every entry (the default). |
| `--entry <NAME>` | No | — | Extract only the named entry. May be repeated. |

**Example:**

```bash
supazip extract backup.7z --out ./restore --password s3cret
```

## `create`

Create a new archive from one or more files.

```
supazip create <ARCHIVE> <FILES>... [--format <FORMAT>] [--password <PASSWORD>] [--compression <METHOD>]
```

| Argument / Flag | Required | Default | Description |
|-----------------|----------|---------|-------------|
| `<ARCHIVE>` | Yes | — | Destination archive path. Format inferred from extension. |
| `<FILES>...` | Yes | — | One or more files to add. |
| `--format <FORMAT>` | No | Inferred | Force format: `zip` or `7z`. |
| `--password <PASSWORD>` | No | — | Encrypt the archive (7z: AES-256+LZMA2; ZIP: AE-2). |
| `--compression <METHOD>` | No | `deflate` | Compression method for ZIP: `store`, `deflate`, `brotli`, `zstd`. Ignored by 7z (always uses LZMA2). |

**Example:**

```bash
supazip create project.zip src/ Cargo.toml --compression zstd
```

Creation is atomic: the backend writes to a temp file in the same directory, then renames it into place. A crash mid-write never corrupts a previous archive at the target path.

## `test`

Verify the integrity of an archive without extracting it.

```
supazip test <ARCHIVE> [--password <PASSWORD>]
```

| Argument / Flag | Required | Description |
|-----------------|----------|-------------|
| `<ARCHIVE>` | Yes | Path to the archive file. |
| `--password <PASSWORD>` | No | Password for encrypted archives. |

**Example:**

```
$ supazip test archive.zip
OK: archive.zip
```

Exits with code 0 on success, non-zero on failure.

## `completions`

Generate shell completion scripts.

```
supazip completions <SHELL>
```

Supported shells: `bash`, `zsh`, `fish`, `powershell`, `elvish`.

**Example:**

```bash
supazip completions bash > /etc/bash_completion.d/supazip
```
