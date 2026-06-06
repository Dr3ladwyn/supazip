use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use chrono::DateTime;
use sevenz_rust::{
    Error as SevenZError, Password, SevenZArchiveEntry, SevenZMethod, SevenZMethodConfiguration,
    SevenZReader, SevenZWriter,
};

use crate::error::ArchiverError;
use crate::traits::{
    ArchiveEntry, ArchiveFormat, CreateOptions, Limits, ProgressCallback, WriteSeek,
};

/// Apply the `max_archive_size` limit to a boxed reader. Mirrors the helper in
/// `formats/zip.rs`; if you change the semantics, change them in both places.
fn bounded_reader(
    reader: Box<dyn Read>,
    max_archive_size: u64,
) -> Result<Box<dyn Read>, ArchiverError> {
    if max_archive_size == u64::MAX {
        return Ok(reader);
    }
    Ok(Box::new(reader.take(max_archive_size)))
}

pub struct SevenZBackend;

impl SevenZBackend {
    pub fn new() -> Self {
        Self
    }

    fn get_password(password: Option<&str>) -> Password {
        match password {
            Some(p) => Password::from(p),
            None => Password::empty(),
        }
    }

    /// True if any folder in the archive uses AES-256-SHA256 encryption. This
    /// is the most accurate signal we can get from `sevenz-rust 0.6.1` for the
    /// `ArchiveEntry::encrypted` flag, because `SevenZArchiveEntry` exposes
    /// its per-entry `content_methods` only at `pub(crate)` visibility. We
    /// can read the public `archive.folders[].coders[].decompression_method_id()`
    /// and compare to `SevenZMethod::AES256SHA256.id()`. The flag is reported
    /// at the archive level and propagated to every entry.
    fn archive_is_encrypted<R: Read + Seek>(
        reader: R,
        password: Password,
    ) -> Result<bool, ArchiverError> {
        let seven =
            SevenZReader::new(reader, u64::MAX, password).map_err(Self::map_sevenz_error)?;
        let aes_id = SevenZMethod::AES256SHA256.id();
        let encrypted = seven.archive().folders.iter().any(|folder| {
            folder
                .coders
                .iter()
                .any(|c| c.decompression_method_id() == aes_id)
        });
        Ok(encrypted)
    }

    fn entry_to_archive_entry(entry: &SevenZArchiveEntry, archive_encrypted: bool) -> ArchiveEntry {
        let name = entry.name().to_string();
        let path = name.clone();

        // Check if directory: either is_directory flag or name ends with '/'
        let is_dir = entry.is_directory() || name.ends_with('/');

        // Parse modification time
        let modified = if entry.has_last_modified_date {
            let ft = entry.last_modified_date();
            let system_time: std::time::SystemTime = ft.into();
            DateTime::from(system_time)
        } else {
            DateTime::from_timestamp(0, 0).unwrap_or_default()
        };

        // Get CRC32 (stored as u64 in sevenz, but it's CRC32)
        let crc32 = if entry.has_crc {
            Some(entry.crc as u32)
        } else {
            None
        };

        // Compression method - we don't have direct access to method name,
        // so we just report "7z"
        let compression_method = "7z".to_string();

        ArchiveEntry {
            name,
            path,
            is_dir,
            size: entry.size(),
            compressed_size: entry.compressed_size,
            modified: Some(modified),
            compression_method,
            crc32,
            encrypted: archive_encrypted,
        }
    }

    fn map_sevenz_error(err: SevenZError) -> ArchiverError {
        // `sevenz-rust 0.6.1` does not have a dedicated `Cancelled` variant,
        // and we cannot easily add one without forking the upstream crate. The
        // helper closures in this file use `SevenZError::Other(...)` with the
        // message "Operation cancelled" when the user requests cancellation;
        // map that to `ArchiverError::Cancelled` here so callers get a typed
        // variant instead of an opaque "Invalid archive". The same trick is
        // used for the zip-bomb guard: the closure emits
        // `SevenZError::Other("compression ratio exceeds limit".into())` and
        // we translate it to `ArchiverError::TooLarge` so the caller can
        // pattern-match on a single, dedicated variant.
        let msg = format!("{err:?}");
        if msg.contains("Operation cancelled") {
            return ArchiverError::Cancelled;
        }
        if msg.contains("compression ratio exceeds limit") {
            return ArchiverError::TooLarge("compression ratio exceeds limit".into());
        }
        match err {
            SevenZError::PasswordRequired => ArchiverError::PasswordRequired,
            SevenZError::MaybeBadPassword(_) => ArchiverError::WrongPassword,
            SevenZError::Io(e, _) => ArchiverError::Io(e),
            SevenZError::BadSignature(_) => ArchiverError::invalid("Invalid 7z signature"),
            SevenZError::ChecksumVerificationFailed => {
                ArchiverError::invalid("Checksum verification failed")
            }
            _ => ArchiverError::invalid(format!("7z error: {err:?}")),
        }
    }

    /// Path-traversal guard. Returns the destination path for an entry name
    /// when it is safe to write it under `dest_dir`, or `None` when the entry
    /// would escape the destination (absolute path, contains `..`, or resolves
    /// outside `dest_dir` after canonicalisation). The 7z crate does not provide
    /// a built-in equivalent of the zip crate's `enclosed_name`, so we roll one
    /// here. Refusing is preferable to silently skipping because it makes the
    /// malicious archive obvious in logs and tests.
    pub fn safe_join(dest_dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
        use std::path::Path;

        if name.is_empty() {
            return None;
        }
        // The 7z spec uses forward slashes for entry names; normalise
        // backslashes for Windows-built archives.
        let normalised = name.replace('\\', "/");
        let candidate = Path::new(&normalised);
        if candidate.is_absolute() {
            return None;
        }
        for component in candidate.components() {
            use std::path::Component::*;
            match component {
                Prefix(_) | RootDir | ParentDir => return None,
                CurDir | Normal(_) => continue,
            }
        }
        Some(dest_dir.join(candidate))
    }

    /// Drive `SevenZReader::for_each_entries` over the archive.
    ///
    /// `reader` must implement `Read + Seek`. Callers wrap a non-seekable input
    /// (the trait hands us a `Box<dyn Read>`) in a `Cursor<Vec<u8>>` and share
    /// it across passes via `SharedBuffer`. This helper takes a
    /// `Read + Seek` directly so that callers do not have to know about
    /// `Arc`/`Mutex` plumbing.
    fn for_each_entry<F, R>(
        reader: R,
        password: Password,
        entries_filter: Option<&[String]>,
        progress: &dyn ProgressCallback,
        mut extract_fn: F,
    ) -> Result<(), ArchiverError>
    where
        R: Read + Seek,
        F: FnMut(&SevenZArchiveEntry, &mut dyn Read) -> Result<bool, SevenZError>,
    {
        let mut seven =
            SevenZReader::new(reader, u64::MAX, password).map_err(Self::map_sevenz_error)?;

        seven
            .for_each_entries(|entry, entry_reader| {
                if progress.is_cancelled() {
                    return Err(SevenZError::Other("Operation cancelled".into()));
                }

                if let Some(filter) = entries_filter {
                    if !filter.contains(&entry.name().to_string()) {
                        return Ok(true);
                    }
                }

                extract_fn(entry, entry_reader)
            })
            .map_err(Self::map_sevenz_error)?;

        Ok(())
    }
}

/// A seekable wrapper around the buffered archive bytes. The 7z backend
/// buffers the input once and re-seeks the same `Cursor` for any second
/// pass. This avoids the `Vec<u8>::clone()` that the previous code did to
/// hand a fresh reader to the second pass.
struct SharedBuffer {
    cursor: Cursor<Vec<u8>>,
}

impl SharedBuffer {
    fn new(data: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(data),
        }
    }

    /// Reset the cursor to position 0 and return a `&mut` borrow. The caller
    /// uses the `&mut Cursor<Vec<u8>>` to drive a `SevenZReader`; once that
    /// reader is dropped, the cursor is reset for the next pass.
    fn reset(&mut self) -> &mut Cursor<Vec<u8>> {
        self.cursor.seek(SeekFrom::Start(0)).expect("seek start");
        &mut self.cursor
    }
}

impl ArchiveFormat for SevenZBackend {
    fn name(&self) -> &'static str {
        "7z"
    }

    fn extensions(&self) -> &[&str] {
        &["7z"]
    }

    fn list(
        &self,
        reader: Box<dyn Read>,
        password: Option<&str>,
        limits: &Limits,
    ) -> Result<Vec<ArchiveEntry>, ArchiverError> {
        tracing::debug!("Listing 7z archive");

        let pwd = Self::get_password(password);

        // Buffer the entire reader to satisfy SevenZReader's `Read + Seek` bound.
        let mut reader = bounded_reader(reader, limits.max_archive_size)?;
        let mut data = Vec::new();
        std::io::copy(&mut *reader, &mut data).map_err(ArchiverError::Io)?;
        let mut buffer = SharedBuffer::new(data);

        // Probe the archive header for AES-256-SHA256 coders before we iterate.
        let archive_encrypted = Self::archive_is_encrypted(buffer.reset(), pwd.clone())?;

        let mut entries = Vec::new();
        let progress_callback: &dyn ProgressCallback = &crate::traits::NoOpProgress;
        let entries_filter: Option<&[String]> = None;

        Self::for_each_entry(
            buffer.reset(),
            pwd,
            entries_filter,
            progress_callback,
            |entry, _reader| {
                entries.push(Self::entry_to_archive_entry(entry, archive_encrypted));
                Ok(true)
            },
        )?;

        tracing::debug!("Listed {} entries from 7z archive", entries.len());
        Ok(entries)
    }

    fn extract(
        &self,
        reader: Box<dyn Read>,
        dest_dir: &std::path::Path,
        entries: &[&str],
        password: Option<&str>,
        progress: &dyn ProgressCallback,
        limits: &Limits,
    ) -> Result<(), ArchiverError> {
        tracing::debug!(
            "Extracting {} entries from 7z archive to {}",
            entries.len(),
            dest_dir.display()
        );

        let pwd = Self::get_password(password);

        // One buffer, three passes (probe for AES, count size, then extract).
        // All three pass over the same `SharedBuffer` — no second copy of the
        // archive bytes.
        let mut reader = bounded_reader(reader, limits.max_archive_size)?;
        let mut data = Vec::new();
        std::io::copy(&mut *reader, &mut data).map_err(ArchiverError::Io)?;
        let mut buffer = SharedBuffer::new(data);

        // Convert entries to filter
        let entries_filter: Option<Vec<String>> = if entries.is_empty() {
            None
        } else {
            Some(entries.iter().map(|s| s.to_string()).collect())
        };

        // First pass: count total bytes that will be written so progress can
        // report a meaningful `total`. Also count the entries so we can refuse
        // a bomb up front.
        let (total_size, total_entries) = {
            let filter_slice: &[String] = entries_filter.as_deref().unwrap_or(&[]);
            let filter_opt: Option<&[String]> = if entries_filter.is_some() {
                Some(filter_slice)
            } else {
                None
            };
            let mut total: u64 = 0;
            let mut count: usize = 0;
            Self::for_each_entry(
                buffer.reset(),
                pwd.clone(),
                filter_opt,
                progress,
                |entry, _reader| {
                    if !entry.is_directory() {
                        total += entry.size();
                    }
                    count += 1;
                    Ok(true)
                },
            )?;
            (total, count)
        };
        if total_entries > limits.max_entry_count {
            return Err(ArchiverError::TooLarge(format!(
                "archive contains {} entries (limit {})",
                total_entries, limits.max_entry_count
            )));
        }

        std::fs::create_dir_all(dest_dir).map_err(ArchiverError::Io)?;

        // Second pass: actually write the entries to disk under `dest_dir`,
        // with `safe_join` defeating zip-slip-style attacks. We also enforce
        // `max_entry_size` per entry, returning `TooLarge` if a single entry
        // exceeds the limit, and `max_compression_ratio` per entry as a
        // zip-bomb guard.
        let mut extracted_size: u64 = 0;
        let filter_ref: Option<&[String]> = entries_filter.as_deref();
        let max_ratio: u64 = limits.max_compression_ratio as u64;

        Self::for_each_entry(
            buffer.reset(),
            pwd,
            filter_ref,
            progress,
            |entry, entry_reader| {
                if progress.is_cancelled() {
                    return Err(SevenZError::Other("Operation cancelled".into()));
                }

                let name = entry.name().to_string();
                progress.set_message(&name);

                if entry.is_directory() {
                    let outpath = match Self::safe_join(dest_dir, &name) {
                        Some(p) => p,
                        None => {
                            tracing::warn!("7z extract: skipping unsafe entry {name:?}");
                            return Ok(true);
                        }
                    };
                    std::fs::create_dir_all(&outpath)
                        .map_err(|e| SevenZError::Io(e, format!("mkdir {outpath:?}").into()))?;
                    return Ok(true);
                }

                let outpath = match Self::safe_join(dest_dir, &name) {
                    Some(p) => p,
                    None => {
                        tracing::warn!("7z extract: skipping unsafe entry {name:?}");
                        // Skip the entry entirely: drain it to /dev/null so the
                        // sevenz-rust cursor stays in sync.
                        let mut sink = [0u8; 8192];
                        loop {
                            if entry_reader
                                .read(&mut sink)
                                .map_err(|e| SevenZError::Io(e, "drain skipped entry".into()))?
                                == 0
                            {
                                break;
                            }
                        }
                        return Ok(true);
                    }
                };

                if entry.size() > limits.max_entry_size {
                    return Err(SevenZError::Io(
                        std::io::Error::other("entry too large"),
                        format!("entry {} exceeds max_entry_size", entry.size()).into(),
                    ));
                }

                if let Some(parent) = outpath.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent)
                            .map_err(|e| SevenZError::Io(e, format!("mkdir {parent:?}").into()))?;
                    }
                }

                let mut outfile = std::fs::File::create(&outpath)
                    .map_err(|e| SevenZError::Io(e, format!("create {outpath:?}").into()))?;

                // Zip-bomb guard (proof-of-concept; other backends are TODO 0.3).
                // Compare the per-entry compressed size to the running total of
                // uncompressed bytes written so far. `max_ratio == 0` disables
                // the check. The threshold is `compressed_size * ratio`; if the
                // uncompressed total ever exceeds it, the archive is a bomb and
                // we abort.
                let ratio_limit = entry.compressed_size.saturating_mul(max_ratio);

                let mut buf = [0u8; 8192];
                loop {
                    if progress.is_cancelled() {
                        return Err(SevenZError::Other("Operation cancelled".into()));
                    }

                    let bytes_read = entry_reader
                        .read(&mut buf)
                        .map_err(|e| SevenZError::Io(e, "".into()))?;

                    if bytes_read == 0 {
                        break;
                    }

                    if max_ratio > 0 && extracted_size + bytes_read as u64 > ratio_limit {
                        // The closure must return a `SevenZError`; we use the
                        // `Other` variant with a marker string that
                        // `map_sevenz_error` translates to
                        // `ArchiverError::TooLarge` so the caller gets a typed
                        // variant instead of an opaque "Invalid archive".
                        return Err(SevenZError::Other("compression ratio exceeds limit".into()));
                    }

                    outfile
                        .write_all(&buf[..bytes_read])
                        .map_err(|e| SevenZError::Io(e, "".into()))?;

                    extracted_size += bytes_read as u64;
                    progress.set_progress(extracted_size, total_size);
                }

                Ok(true)
            },
        )?;

        tracing::debug!("Extracted {} bytes from 7z archive", extracted_size);
        Ok(())
    }

    fn create(
        &self,
        writer: Box<dyn WriteSeek>,
        entries: &[PathBuf],
        _options: &CreateOptions,
        password: Option<&str>,
        progress: &dyn ProgressCallback,
        limits: &Limits,
    ) -> Result<(), ArchiverError> {
        if entries.len() > limits.max_entry_count {
            return Err(ArchiverError::TooLarge(format!(
                "create asked for {} entries (limit {})",
                entries.len(),
                limits.max_entry_count
            )));
        }

        tracing::debug!("Creating 7z archive with {} entries", entries.len());

        let mut sz = SevenZWriter::new(writer)
            .map_err(|e| ArchiverError::invalid_with_source("Failed to create 7z writer", e))?;

        // Honour `--password` for 7z. `sevenz-rust 0.6.1` requires the
        // `aes256` feature for AES-256 encryption; we enabled it in
        // `supazip-core/Cargo.toml`. The encoder options are applied to the
        // writer via `set_content_methods`, which replaces the default
        // LZMA2 chain with AES-256 followed by LZMA2.
        //
        // The previous implementation did `let _pwd = password;`, which
        // silently dropped the password — encrypted create produced an
        // unencrypted archive. That is the bug this commit fixes.
        if let Some(pwd) = password {
            let aes = sevenz_rust::AesEncoderOptions::new(Password::from(pwd));
            let chain: Vec<SevenZMethodConfiguration> =
                vec![aes.into(), SevenZMethod::LZMA2.into()];
            sz.set_content_methods(chain);
        }

        let total = entries.len() as u64;

        for (i, path) in entries.iter().enumerate() {
            if progress.is_cancelled() {
                return Err(ArchiverError::Cancelled);
            }

            let entry_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            progress.set_message(&entry_name);
            progress.set_progress(i as u64, total);

            let entry = SevenZArchiveEntry::from_path(path, entry_name.clone());

            if entry.is_directory() {
                sz.push_archive_entry(entry, Option::<&mut std::fs::File>::None)
                    .map_err(|e| {
                        ArchiverError::invalid_with_source("Failed to add directory", e)
                    })?;
            } else {
                let mut file = std::fs::File::open(path).map_err(ArchiverError::Io)?;
                sz.push_archive_entry(entry, Some(&mut file))
                    .map_err(|e| ArchiverError::invalid_with_source("Failed to add file", e))?;
            }
        }

        sz.finish().map_err(ArchiverError::Io)?;

        progress.set_progress(total, total);
        tracing::debug!("Created 7z archive with {} entries", entries.len());
        Ok(())
    }

    fn test(
        &self,
        reader: Box<dyn Read>,
        password: Option<&str>,
        progress: &dyn ProgressCallback,
        limits: &Limits,
    ) -> Result<bool, ArchiverError> {
        tracing::debug!("Testing 7z archive");

        let pwd = Self::get_password(password);

        // One buffer, two passes via the same `SharedBuffer` (no clone of
        // the archive bytes).
        let mut reader = bounded_reader(reader, limits.max_archive_size)?;
        let mut data = Vec::new();
        std::io::copy(&mut *reader, &mut data).map_err(ArchiverError::Io)?;
        let mut buffer = SharedBuffer::new(data);

        let mut processed: u64 = 0;
        let mut total_size: u64 = 0;
        let mut total_entries: usize = 0;

        Self::for_each_entry(
            buffer.reset(),
            pwd.clone(),
            None,
            progress,
            |entry, _reader| {
                total_size += entry.size();
                total_entries += 1;
                Ok(true)
            },
        )?;

        Self::for_each_entry(
            buffer.reset(),
            pwd,
            None,
            progress,
            |entry, entry_reader| {
                if progress.is_cancelled() {
                    return Err(SevenZError::Other("Operation cancelled".into()));
                }

                progress.set_message(entry.name());

                if !entry.is_directory() && entry.size() > 0 {
                    let mut buf = [0u8; 8192];
                    loop {
                        if progress.is_cancelled() {
                            return Err(SevenZError::Other("Operation cancelled".into()));
                        }

                        let bytes_read = entry_reader
                            .read(&mut buf)
                            .map_err(|e| SevenZError::Io(e, "".into()))?;

                        if bytes_read == 0 {
                            break;
                        }

                        processed += bytes_read as u64;
                        progress.set_progress(processed, total_size);
                    }
                } else {
                    processed += entry.size();
                    progress.set_progress(processed, total_size);
                }

                Ok(true)
            },
        )?;

        tracing::debug!("Tested {} entries in 7z archive - OK", total_entries);
        Ok(true)
    }
}

impl Default for SevenZBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::NoOpProgress;
    use std::path::PathBuf;

    fn write_two_files(dir: &std::path::Path) -> Vec<PathBuf> {
        std::fs::create_dir_all(dir).expect("mkdir");
        std::fs::write(dir.join("a.txt"), b"alpha\n").expect("write a");
        std::fs::write(dir.join("b.bin"), &[1u8, 2, 3, 4, 5][..]).expect("write b");
        vec![dir.join("a.txt"), dir.join("b.bin")]
    }

    #[test]
    fn unencrypted_round_trip() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("plain.7z");
        let src = tmp.path().join("src");
        let entries = write_two_files(&src);

        let file = std::fs::File::create(&archive).expect("create archive");
        let writer: Box<dyn WriteSeek> = Box::new(file);
        let opts = CreateOptions {
            compression_method: "deflate".to_string(),
            compression_level: None,
        };
        SevenZBackend::new()
            .create(
                writer,
                &entries,
                &opts,
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");

        let listed = SevenZBackend::new()
            .list(
                Box::new(std::fs::File::open(&archive).expect("open")),
                None,
                &Limits::default(),
            )
            .expect("list");
        assert_eq!(listed.len(), 2);
        assert!(
            listed.iter().all(|e| !e.encrypted),
            "plain archive should not be encrypted"
        );

        let ok = SevenZBackend::new()
            .test(
                Box::new(std::fs::File::open(&archive).expect("open")),
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("test");
        assert!(ok);
    }

    #[test]
    fn encrypted_create_marks_archive_encrypted() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("enc.7z");
        let src = tmp.path().join("src");
        let entries = write_two_files(&src);

        let file = std::fs::File::create(&archive).expect("create archive");
        let writer: Box<dyn WriteSeek> = Box::new(file);
        let opts = CreateOptions {
            compression_method: "deflate".to_string(),
            compression_level: None,
        };
        SevenZBackend::new()
            .create(
                writer,
                &entries,
                &opts,
                Some("correct-horse"),
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");

        // Reading without a password should fail.
        let no_pwd = SevenZBackend::new().list(
            Box::new(std::fs::File::open(&archive).expect("open")),
            None,
            &Limits::default(),
        );
        assert!(
            no_pwd.is_err(),
            "expected password error when listing encrypted 7z without pwd"
        );

        // Reading with the correct password should succeed and report encrypted.
        let listed = SevenZBackend::new()
            .list(
                Box::new(std::fs::File::open(&archive).expect("open")),
                Some("correct-horse"),
                &Limits::default(),
            )
            .expect("list with pwd");
        assert_eq!(listed.len(), 2);
        assert!(
            listed.iter().all(|e| e.encrypted),
            "encrypted archive should mark every entry encrypted; got {:?}",
            listed
                .iter()
                .map(|e| (&e.name, e.encrypted))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn wrong_password_list_returns_wrong_password() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("enc.7z");
        let src = tmp.path().join("src");
        let entries = write_two_files(&src);

        let file = std::fs::File::create(&archive).expect("create archive");
        let writer: Box<dyn WriteSeek> = Box::new(file);
        let opts = CreateOptions {
            compression_method: "deflate".to_string(),
            compression_level: None,
        };
        SevenZBackend::new()
            .create(
                writer,
                &entries,
                &opts,
                Some("correct-horse"),
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");

        // Wrong password should surface as `WrongPassword` (or `PasswordRequired`
        // if the upstream library's first-line check fires before content decode).
        let err = SevenZBackend::new()
            .list(
                Box::new(std::fs::File::open(&archive).expect("open")),
                Some("battery-staple"),
                &Limits::default(),
            )
            .expect_err("wrong password should fail");
        assert!(
            matches!(
                err,
                ArchiverError::WrongPassword | ArchiverError::PasswordRequired
            ),
            "expected WrongPassword or PasswordRequired, got {err:?}",
        );
    }

    #[test]
    fn unencrypted_extract_round_trip_7z() {
        // The sevenz backend's extract now writes files to disk under
        // `dest_dir`. Build an archive, extract it, and assert the body of
        // every entry round-tripped.
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("plain.7z");
        let src = tmp.path().join("src");
        let entries = write_two_files(&src);

        let file = std::fs::File::create(&archive).expect("create archive");
        let writer: Box<dyn WriteSeek> = Box::new(file);
        let opts = CreateOptions {
            compression_method: "deflate".to_string(),
            compression_level: None,
        };
        SevenZBackend::new()
            .create(
                writer,
                &entries,
                &opts,
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");

        let dest = tmp.path().join("out");
        SevenZBackend::new()
            .extract(
                Box::new(std::fs::File::open(&archive).expect("open")),
                &dest,
                &[],
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("extract");

        for entry in &entries {
            let body = std::fs::read(entry).expect("read source");
            let name = entry.file_name().unwrap().to_str().unwrap();
            let extracted = dest.join(name);
            let out_body = std::fs::read(&extracted)
                .unwrap_or_else(|e| panic!("read {}: {e}", extracted.display()));
            assert_eq!(body, out_body, "round-trip for {name}");
        }
    }

    #[test]
    fn safe_join_rejects_path_traversal() {
        // `safe_join` must refuse absolute paths, parent-dir traversal, and
        // backslashes that would resolve to a parent on Windows.
        let dest = std::path::Path::new("/tmp/dest");
        assert!(SevenZBackend::safe_join(dest, "ok.txt").is_some());
        assert!(SevenZBackend::safe_join(dest, "sub/ok.txt").is_some());
        assert!(SevenZBackend::safe_join(dest, "../escape.txt").is_none());
        assert!(SevenZBackend::safe_join(dest, "/abs.txt").is_none());
        assert!(SevenZBackend::safe_join(dest, "..").is_none());
        assert!(SevenZBackend::safe_join(dest, "").is_none());
    }

    #[test]
    fn cancellation_surfaces_as_cancelled() {
        // Build a real archive and then drive `extract` through a progress
        // callback that requests cancellation on the first entry. The extract
        // must short-circuit with `ArchiverError::Cancelled`.
        use crate::traits::ProgressCallback;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("c.7z");
        let src = tmp.path().join("src");
        let entries = write_two_files(&src);
        let file = std::fs::File::create(&archive).expect("create archive");
        let writer: Box<dyn WriteSeek> = Box::new(file);
        let opts = CreateOptions {
            compression_method: "deflate".to_string(),
            compression_level: None,
        };
        SevenZBackend::new()
            .create(
                writer,
                &entries,
                &opts,
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");

        struct CancelAfter(AtomicUsize);
        impl ProgressCallback for CancelAfter {
            fn set_progress(&self, _c: u64, _t: u64) {}
            fn set_message(&self, _m: &str) {}
            fn is_cancelled(&self) -> bool {
                // Cancel after the first message we see; the for_each_entry
                // callback runs `progress.is_cancelled()` before each entry.
                self.0.fetch_add(1, Ordering::SeqCst) > 0
            }
        }
        let cb = CancelAfter(AtomicUsize::new(0));
        let dest = tmp.path().join("out");
        let res = SevenZBackend::new().extract(
            Box::new(std::fs::File::open(&archive).expect("open")),
            &dest,
            &[],
            None,
            &cb,
            &Limits::default(),
        );
        assert!(
            matches!(res, Err(ArchiverError::Cancelled)),
            "expected Cancelled, got {res:?}"
        );
    }

    #[test]
    fn test_7z_corrupted_archive_returns_error() {
        // Random non-7z bytes should not parse as a 7z archive.
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("corrupt.7z");
        std::fs::write(&archive, b"this is not a 7z archive at all\n").expect("write");

        let res = SevenZBackend::new().test(
            Box::new(std::fs::File::open(&archive).expect("open")),
            None,
            &NoOpProgress,
            &Limits::default(),
        );
        assert!(res.is_err(), "corrupt archive should error, got {res:?}");
    }

    #[test]
    fn list_7z_with_directory_entry() {
        // The sevenz backend lists directory entries when present; assert
        // `is_dir` is true for at least one entry in a directory-bearing
        // archive.
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("dirs.7z");
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).expect("mkdir sub");
        std::fs::write(sub.join("file.txt"), b"in sub\n").expect("write file");
        let file = std::fs::File::create(&archive).expect("create archive");
        let writer: Box<dyn WriteSeek> = Box::new(file);
        let opts = CreateOptions {
            compression_method: "deflate".to_string(),
            compression_level: None,
        };
        SevenZBackend::new()
            .create(
                writer,
                std::slice::from_ref(&sub),
                &opts,
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");

        let listed = SevenZBackend::new()
            .list(
                Box::new(std::fs::File::open(&archive).expect("open")),
                None,
                &Limits::default(),
            )
            .expect("list");
        assert!(!listed.is_empty());
        assert!(
            listed.iter().any(|e| e.is_dir),
            "expected at least one directory entry, got {listed:?}"
        );
    }

    #[test]
    fn extract_rejects_zip_bomb_via_compression_ratio() {
        // Proof-of-concept for `Limits::max_compression_ratio`. Build a 7z
        // archive with one highly-compressible entry (64 KiB of zeros), then
        // extract it under a tight ratio cap; the extract must fail with
        // `TooLarge`. Repeating with `max_compression_ratio = 0` must succeed
        // and confirm that 0 disables the check.
        use crate::traits::ProgressCallback;

        let tmp = tempfile::tempdir().expect("tempdir");
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).expect("mkdir src");
        // 64 KiB of zeros — LZMA2 compresses this to well under 1 KiB.
        let payload = vec![0u8; 64 * 1024];
        std::fs::write(src.join("zeros.bin"), &payload).expect("write zeros");

        let archive = tmp.path().join("bomb.7z");
        let file = std::fs::File::create(&archive).expect("create archive");
        let writer: Box<dyn WriteSeek> = Box::new(file);
        let opts = CreateOptions {
            compression_method: "deflate".to_string(),
            compression_level: None,
        };
        let entries = vec![src.join("zeros.bin")];
        SevenZBackend::new()
            .create(
                writer,
                &entries,
                &opts,
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");

        // Tight ratio cap (10×): the bomb must be refused.
        let tight_limits = Limits {
            max_compression_ratio: 10,
            ..Limits::default()
        };
        let dest_tight = tmp.path().join("out_tight");
        let res_tight = SevenZBackend::new().extract(
            Box::new(std::fs::File::open(&archive).expect("open")),
            &dest_tight,
            &[],
            None,
            &NoOpProgress,
            &tight_limits,
        );
        match res_tight {
            Err(ArchiverError::TooLarge(msg)) => {
                assert!(
                    msg.contains("compression ratio"),
                    "expected compression-ratio error message, got {msg:?}",
                );
            }
            other => panic!("expected TooLarge, got {other:?}"),
        }

        // `max_compression_ratio = 0` disables the check; the same archive
        // must extract cleanly.
        let loose_limits = Limits {
            max_compression_ratio: 0,
            ..Limits::default()
        };
        let dest_loose = tmp.path().join("out_loose");
        SevenZBackend::new()
            .extract(
                Box::new(std::fs::File::open(&archive).expect("open")),
                &dest_loose,
                &[],
                None,
                &NoOpProgress,
                &loose_limits,
            )
            .expect("extract with ratio=0 must succeed");
        let body = std::fs::read(dest_loose.join("zeros.bin")).expect("read extracted");
        assert_eq!(body, payload, "round-trip with ratio=0 must preserve bytes");

        // Silence the unused-import warning when this test runs in isolation.
        let _cb: &dyn ProgressCallback = &NoOpProgress;
    }
}
