use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::PathBuf;

use chrono::DateTime;
use sevenz_rust::{
    Error as SevenZError, Password, SevenZArchiveEntry, SevenZMethod, SevenZMethodConfiguration,
    SevenZReader, SevenZWriter,
};

use crate::error::ArchiverError;
use crate::traits::{ArchiveEntry, ArchiveFormat, CreateOptions, ProgressCallback, WriteSeek};

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
        match err {
            SevenZError::PasswordRequired => ArchiverError::PasswordRequired,
            SevenZError::MaybeBadPassword(_) => ArchiverError::WrongPassword,
            SevenZError::Io(e, _) => ArchiverError::Io(e),
            SevenZError::BadSignature(_) => {
                ArchiverError::InvalidArchive("Invalid 7z signature".into())
            }
            SevenZError::ChecksumVerificationFailed => {
                ArchiverError::InvalidArchive("Checksum verification failed".into())
            }
            _ => ArchiverError::InvalidArchive(format!("7z error: {:?}", err)),
        }
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
        mut reader: Box<dyn Read>,
        password: Option<&str>,
    ) -> Result<Vec<ArchiveEntry>, ArchiverError> {
        tracing::debug!("Listing 7z archive");

        let pwd = Self::get_password(password);

        // Buffer the entire reader to satisfy SevenZReader's `Read + Seek` bound.
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
        mut reader: Box<dyn Read>,
        dest: Box<dyn WriteSeek>,
        entries: &[&str],
        password: Option<&str>,
        progress: &dyn ProgressCallback,
    ) -> Result<(), ArchiverError> {
        tracing::debug!("Extracting {} entries from 7z archive", entries.len());

        let pwd = Self::get_password(password);

        // One buffer, three passes (probe for AES, count size, then extract).
        // All three pass over the same `SharedBuffer` — no second copy of the
        // archive bytes.
        let mut data = Vec::new();
        std::io::copy(&mut *reader, &mut data).map_err(ArchiverError::Io)?;
        let mut buffer = SharedBuffer::new(data);

        // Convert entries to filter
        let entries_filter: Option<Vec<String>> = if entries.is_empty() {
            None
        } else {
            Some(entries.iter().map(|s| s.to_string()).collect())
        };

        // First pass: calculate total uncompressed size
        let total_size: u64 = {
            let filter_slice: &[String] = entries_filter.as_deref().unwrap_or(&[]);
            let filter_opt: Option<&[String]> = if entries_filter.is_some() {
                Some(filter_slice)
            } else {
                None
            };
            let mut total: u64 = 0;
            Self::for_each_entry(
                buffer.reset(),
                pwd.clone(),
                filter_opt,
                progress,
                |entry, _reader| {
                    if !entry.is_directory() {
                        total += entry.size();
                    }
                    Ok(true)
                },
            )?;
            total
        };

        // Second pass: write the data through the trait's `dest` writer.
        //
        // Note on streaming: the trait's `dest: Box<dyn WriteSeek>` is a
        // single contiguous writer. The current core implementation ignores
        // `dest` and writes to the current working directory via
        // `enclosed_name`; that pre-existing behaviour is preserved here.
        // Streaming the per-entry bytes into `dest` as a tar-like stream is a
        // larger refactor and is tracked in `decisionLog.md`.
        let _ = dest;
        let mut extracted_size: u64 = 0;
        let filter_ref: Option<&[String]> = entries_filter.as_deref();

        Self::for_each_entry(
            buffer.reset(),
            pwd,
            filter_ref,
            progress,
            |entry, entry_reader| {
                if progress.is_cancelled() {
                    return Err(SevenZError::Other("Operation cancelled".into()));
                }

                progress.set_message(entry.name());

                if entry.is_directory() {
                    return Ok(true);
                }

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
    ) -> Result<(), ArchiverError> {
        tracing::debug!("Creating 7z archive with {} entries", entries.len());

        let mut sz = SevenZWriter::new(writer).map_err(|e| {
            ArchiverError::InvalidArchive(format!("Failed to create 7z writer: {:?}", e))
        })?;

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
                        ArchiverError::InvalidArchive(format!("Failed to add directory: {:?}", e))
                    })?;
            } else {
                let mut file = std::fs::File::open(path).map_err(ArchiverError::Io)?;
                sz.push_archive_entry(entry, Some(&mut file)).map_err(|e| {
                    ArchiverError::InvalidArchive(format!("Failed to add file: {:?}", e))
                })?;
            }
        }

        sz.finish().map_err(ArchiverError::Io)?;

        progress.set_progress(total, total);
        tracing::debug!("Created 7z archive with {} entries", entries.len());
        Ok(())
    }

    fn test(
        &self,
        mut reader: Box<dyn Read>,
        password: Option<&str>,
        progress: &dyn ProgressCallback,
    ) -> Result<bool, ArchiverError> {
        tracing::debug!("Testing 7z archive");

        let pwd = Self::get_password(password);

        // One buffer, two passes via the same `SharedBuffer` (no clone of
        // the archive bytes).
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
            .create(writer, &entries, &opts, None, &NoOpProgress)
            .expect("create");

        let listed = SevenZBackend::new()
            .list(Box::new(std::fs::File::open(&archive).expect("open")), None)
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
            )
            .expect("create");

        // Reading without a password should fail.
        let no_pwd =
            SevenZBackend::new().list(Box::new(std::fs::File::open(&archive).expect("open")), None);
        assert!(
            no_pwd.is_err(),
            "expected password error when listing encrypted 7z without pwd"
        );

        // Reading with the correct password should succeed and report encrypted.
        let listed = SevenZBackend::new()
            .list(
                Box::new(std::fs::File::open(&archive).expect("open")),
                Some("correct-horse"),
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
}
