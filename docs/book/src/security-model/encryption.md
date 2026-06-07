# Encryption

SupaZip supports AES-256 encryption for both ZIP and 7z formats. This chapter describes the encryption schemes, key handling, and security properties.

## Encryption schemes

### ZIP: AES-256 (AE-2)

| Property | Value |
|----------|-------|
| Cipher | AES-256 |
| Mode | AE-2 (WinZip-compatible) |
| Key derivation | PBKDF2 with SHA-1 |
| Salt | Per-archive, randomly generated |
| Authentication | HMAC-SHA-1 per entry |

ZIP AE-2 encrypts each entry individually. The archive header (entry names, sizes) is **not** encrypted — only the entry data is protected. An attacker can see the entry list but not the contents.

### 7z: AES-256 + LZMA2

| Property | Value |
|----------|-------|
| Cipher | AES-256 |
| Mode | CBC |
| Key derivation | SHA-256 based |
| Compression | LZMA2 (applied before encryption) |
| Header encryption | Optional (hides entry list) |

7z encryption wraps the LZMA2-compressed data in AES-256-CBC. When header encryption is enabled, the entire archive header (entry names, sizes, timestamps) is encrypted, hiding the archive's structure from attackers.

**Header encryption vs. data-only encryption:**

| Mode | What is encrypted | Use case |
|------|-------------------|----------|
| Data only | Entry contents | Faster listing; names visible |
| Header + data | Entry contents + header | Maximum privacy |

SupaZip enables header encryption by default when a password is provided for 7z archives.

## Key handling

### Password flow

```
User (CLI --password or GUI dialog)
  │
  ▼
Option<&str>  ──▶  ArchiveFormat::create() / extract() / list()
  │
  ▼
Backend passes password to library (zip / sevenz-rust)
  │
  ▼
Library derives encryption key internally
  │
  ▼
Password reference is dropped at end of scope
```

### Properties

1. **No persistent storage.** Passwords are never written to disk by SupaZip. They exist only in memory for the duration of the operation.

2. **No long-lived buffers.** The password is passed as `Option<&str>` through the engine. It is not copied into `String` or `Vec<u8>` unless the backend library requires it internally.

3. **Scope-bounded.** The `&str` reference lives only for the duration of the `list`, `extract`, `create`, or `test` call. After the call returns, the reference is dropped.

4. **GUI masked input.** The GUI password dialog uses a masked input field with a show/hide toggle. The password is stored in `PasswordDialogState::password: String` while the dialog is visible, then cleared when the dialog is closed or the password is submitted.

5. **CLI flag exposure.** The CLI accepts passwords via `--password <value>`. This means the password may appear in shell history (`~/.bash_history`, etc.). Users should:
   - Use environment variables: `supazip extract archive.7z --password "$SECRET"`
   - Pipe from a password manager: `supazip extract archive.7z --password "$(pass show archive)"`
   - Use the GUI for interactive password entry

### What SupaZip does NOT do

- **No key derivation in SupaZip.** Key derivation is handled by the `zip` and `sevenz-rust` libraries internally. SupaZip passes the raw password string.
- **No key caching.** SupaZip does not cache derived keys between operations. Each operation derives the key fresh.
- **No secure memory wiping.** Rust does not guarantee memory zeroing on drop. The password string may remain in deallocated memory until the OS reclaims it. This is a known limitation.

## Creating encrypted archives

### CLI

```bash
# 7z with password (AES-256 + LZMA2, header encrypted)
supazip create secret.7z file1.txt file2.txt --password s3cret

# ZIP with password (AES-256 AE-2)
supazip create secret.zip file1.txt file2.txt --password s3cret
```

### GUI

The GUI does not currently expose a password field in the Create dialog. Encrypted archive creation is CLI-only in the current release. GUI support is planned for a future milestone.

## Extracting encrypted archives

### CLI

```bash
supazip extract secret.7z --out ./output --password s3cret
```

### GUI

When the GUI encounters an encrypted archive:

1. The `list` operation returns `ArchiverError::PasswordRequired`.
2. The GUI translates this to `EngineEvent::PasswordRequired`.
3. The password dialog opens automatically.
4. The user enters the password and clicks OK.
5. The GUI re-dispatches the operation with the password.

For context-menu `Extract to…` on an encrypted archive, the GUI opens the password dialog **before** spawning the extract worker, ensuring the password is available when extraction begins.

## Listing encrypted archives

### ZIP

ZIP AE-2 encrypts entry data only. The entry list (names, sizes, timestamps) is readable without a password. `supazip list archive.zip` works without `--password`.

### 7z

7z with header encryption hides the entry list. `supazip list archive.7z` requires `--password`. Without it, the operation returns `PasswordRequired`.

## Integrity verification

### ZIP AE-2

Each entry has an HMAC-SHA-1 authentication tag. The `test` subcommand verifies these tags:

```bash
supazip test secret.zip --password s3cret
```

### 7z

7z uses CRC-32 and SHA-256 checksums. The `test` subcommand verifies decompressed data against these checksums:

```bash
supazip test secret.7z --password s3cret
```

## Limitations

| Limitation | Scope | Status |
|-----------|-------|--------|
| No secure memory wiping | All | Known limitation; Rust does not guarantee zeroing |
| CLI password in shell history | CLI | Mitigated by env vars or password manager piping |
| No GUI password field for Create | GUI | Planned for future milestone |
| No key file support | All | Not planned for 1.0 |
| No asymmetric encryption | All | Not planned; SupaZip uses symmetric AES-256 only |

## Comparison

| Feature | ZIP AE-2 | 7z AES-256 |
|---------|----------|------------|
| Data encryption | AES-256 | AES-256 |
| Header encryption | No | Yes (optional, enabled by default) |
| Key derivation | PBKDF2-SHA-1 | SHA-256 based |
| Authentication | HMAC-SHA-1 per entry | CRC-32 + SHA-256 |
| Compression | Before encryption | Before encryption (LZMA2) |
| Standard | WinZip AE-2 | 7z format spec |
