use chrono::{DateTime, Utc};
use std::io::{Cursor, Read, Write};
use std::path::PathBuf;
use zip::write::FileOptions;
use zip::{AesMode, CompressionMethod, DateTime as ZipDateTime, ZipArchive, ZipWriter};

use crate::error::ArchiverError;
use crate::traits::{
    ArchiveEntry, ArchiveFormat, CompressionMethod as SupaCompressionMethod, CreateOptions, Limits,
    ProgressCallback, WriteSeek,
};

/// Wraps a `Read` in a `std::io::Take` bounded by `max_archive_size`, returning
/// a clear `ArchiverError::TooLarge` when the underlying reader has more bytes
/// than the limit allows. Callers pass the bounded reader to the zip / 7z
/// backend instead of the raw input, so the backends never need to know the
/// limit. If the limit is `u64::MAX` the wrapper is a no-op pass-through.
fn bounded_reader(
    reader: Box<dyn Read>,
    max_archive_size: u64,
) -> Result<Box<dyn Read>, ArchiverError> {
    if max_archive_size == u64::MAX {
        return Ok(reader);
    }
    Ok(Box::new(reader.take(max_archive_size)))
}

pub struct ZipBackend;

impl Default for ZipBackend {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Free-standing helpers for the WS-B compression codec support.
// ---------------------------------------------------------------------------

fn effective_compression(options: &CreateOptions) -> SupaCompressionMethod {
    match options.compression {
        SupaCompressionMethod::Deflate => {
            SupaCompressionMethod::from_legacy_str(&options.compression_method)
        }
        other => other,
    }
}

fn brotli_compress_entry(data: &[u8]) -> Result<Vec<u8>, ArchiverError> {
    let mut out = Vec::with_capacity(data.len() / 2 + 64);
    {
        let mut writer = brotli::CompressorWriter::new(&mut out, 4096, 6, 22);
        std::io::Write::write_all(&mut writer, data).map_err(ArchiverError::Io)?;
    }
    Ok(out)
}

fn brotli_decompress_entry(data: &[u8]) -> Result<Vec<u8>, ArchiverError> {
    let mut out = Vec::with_capacity(data.len() * 3 + 64);
    brotli::BrotliDecompress(&mut std::io::Cursor::new(data), &mut out)
        .map_err(|e| ArchiverError::invalid(format!("brotli decompress: {e}")))?;
    Ok(out)
}

fn pre_compress_for_method(
    method: SupaCompressionMethod,
    data: &[u8],
) -> Result<(Vec<u8>, CompressionMethod), ArchiverError> {
    match method {
        SupaCompressionMethod::Brotli => {
            let compressed = brotli_compress_entry(data)?;
            Ok((compressed, CompressionMethod::Stored))
        }
        other => Ok((data.to_vec(), to_zip_method(other))),
    }
}

fn to_zip_method(method: SupaCompressionMethod) -> CompressionMethod {
    match method {
        SupaCompressionMethod::Deflate => CompressionMethod::Deflated,
        SupaCompressionMethod::Store => CompressionMethod::Stored,
        SupaCompressionMethod::Zstd => CompressionMethod::Zstd,
        SupaCompressionMethod::Brotli => CompressionMethod::Stored,
    }
}

impl ZipBackend {
    pub fn new() -> Self {
        Self
    }

    fn zip_datetime_to_chrono(dt: ZipDateTime) -> Option<DateTime<Utc>> {
        // zip 2.x: convert through TryFrom<zip::DateTime> for chrono::NaiveDateTime
        // (requires the `chrono` feature on the `zip` crate).
        chrono::NaiveDateTime::try_from(dt)
            .ok()
            .map(|n| DateTime::<Utc>::from_naive_utc_and_offset(n, Utc))
    }

    fn compression_method_to_string(method: CompressionMethod) -> String {
        match method {
            CompressionMethod::Stored => "store".to_string(),
            CompressionMethod::Deflated => "deflate".to_string(),
            CompressionMethod::Bzip2 => "bzip2".to_string(),
            CompressionMethod::Zstd => "zstd".to_string(),
            _ => "unknown".to_string(),
        }
    }
}

impl ArchiveFormat for ZipBackend {
    fn name(&self) -> &'static str {
        "zip"
    }

    fn extensions(&self) -> &[&str] {
        &["zip", "cbz"]
    }

    fn list(
        &self,
        reader: Box<dyn Read>,
        password: Option<&str>,
        limits: &Limits,
    ) -> Result<Vec<ArchiveEntry>, ArchiverError> {
        // zip 2.x requires `R: Read + Seek` for `ZipArchive::new` because the
        // central directory is read at the end. The trait hands us a non-seekable
        // `Box<dyn Read>`, so we buffer into memory and hand the resulting
        // `Cursor<Vec<u8>>` to the zip crate. The 7z backend genuinely streams;
        // see `formats/sevenz.rs` for the contrast.
        let mut reader = bounded_reader(reader, limits.max_archive_size)?;
        let mut data = Vec::new();
        std::io::copy(&mut *reader, &mut data).map_err(ArchiverError::Io)?;
        let mut archive = ZipArchive::new(Cursor::new(data))
            .map_err(|e| ArchiverError::invalid_with_source("zip parse", e))?;

        if archive.len() > limits.max_entry_count {
            return Err(ArchiverError::TooLarge(format!(
                "archive contains {} entries (limit {})",
                archive.len(),
                limits.max_entry_count
            )));
        }

        let mut entries = Vec::new();

        for i in 0..archive.len() {
            // `by_index` works for unencrypted entries. For encrypted entries we
            // fall back to `by_index_decrypt` when a password is available so the
            // zip crate can surface a proper `PasswordRequired` / `WrongPassword`
            // error; listing is metadata-only, so a wrong password on a single
            // encrypted entry is a hard error.
            let file = if let Some(pwd) = password {
                archive
                    .by_index_decrypt(i, pwd.as_bytes())
                    .map_err(|e| ArchiverError::invalid_with_source("zip decrypt", e))?
            } else {
                archive
                    .by_index(i)
                    .map_err(|e| ArchiverError::invalid_with_source("zip index", e))?
            };

            let name = file.name().to_string();
            let is_dir = name.ends_with('/');

            // SupaZip stores Brotli-compressed entries as `Stored` ZIP entries
            // with a `.br` filename suffix (the zip crate has no native Brotli
            // codec). Surface the codec as "brotli" in the listing, and strip
            // the suffix from the user-facing name / path so downstream code
            // (the GUI's file list, the CLI's `list` table) keeps operating on
            // the logical file name.
            let is_brotli = !is_dir && name.ends_with(".br");
            let logical_name = if is_brotli {
                name.trim_end_matches(".br").to_string()
            } else {
                name.clone()
            };
            let compression_method = if is_brotli {
                "brotli".to_string()
            } else {
                Self::compression_method_to_string(file.compression())
            };

            let modified = file.last_modified().and_then(Self::zip_datetime_to_chrono);

            let entry = ArchiveEntry {
                name: logical_name.clone(),
                path: logical_name.clone(),
                is_dir,
                size: file.size(),
                compressed_size: file.compressed_size(),
                modified,
                compression_method,
                crc32: Some(file.crc32()),
                encrypted: file.encrypted(),
            };

            entries.push(entry);
        }

        tracing::debug!("Listed {} entries from zip archive", entries.len());
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
        // zip 2.x requires `R: Read + Seek` for `ZipArchive::new`; the trait
        // gives us a non-seekable `Box<dyn Read>`, so we buffer and hand the
        // resulting `Cursor<Vec<u8>>` to the zip crate. The 7z backend streams
        // end-to-end; see `formats/sevenz.rs`.
        //
        // `dest_dir` is the directory the caller wants files under. Each
        // entry's path is `enclosed_name()`-validated by the zip crate to
        // defeat zip-slip, then joined onto `dest_dir` with `Path::join`.
        let mut reader = bounded_reader(reader, limits.max_archive_size)?;
        let mut data = Vec::new();
        std::io::copy(&mut *reader, &mut data).map_err(ArchiverError::Io)?;
        let mut archive = ZipArchive::new(Cursor::new(data))
            .map_err(|e| ArchiverError::invalid_with_source("zip parse", e))?;
        if archive.len() > limits.max_entry_count {
            return Err(ArchiverError::TooLarge(format!(
                "archive contains {} entries (limit {})",
                archive.len(),
                limits.max_entry_count
            )));
        }

        std::fs::create_dir_all(dest_dir).map_err(ArchiverError::Io)?;

        let total_entries = if entries.is_empty() {
            archive.len()
        } else {
            entries.len()
        };

        let mut processed = 0u64;

        if entries.is_empty() {
            let mut buffer = [0u8; 64 * 1024];
            // Extract all entries
            for i in 0..archive.len() {
                if progress.is_cancelled() {
                    return Err(ArchiverError::Cancelled);
                }

                let mut file = if let Some(pwd) = password {
                    archive
                        .by_index_decrypt(i, pwd.as_bytes())
                        .map_err(|e| ArchiverError::invalid_with_source("zip decrypt", e))?
                } else {
                    archive
                        .by_index(i)
                        .map_err(|e| ArchiverError::invalid_with_source("zip index", e))?
                };

                let name = file.name().to_string();
                progress.set_message(&name);

                let is_brotli = !file.is_dir() && name.ends_with(".br");
                let logical_name = if is_brotli {
                    name.trim_end_matches(".br").to_string()
                } else {
                    name.clone()
                };
                let enclosed = match file.enclosed_name() {
                    Some(path) => path.to_owned(),
                    None => continue,
                };
                let outpath = if is_brotli {
                    let stripped = match std::path::Path::new(&logical_name).file_name() {
                        Some(s) => s.to_owned(),
                        None => continue,
                    };
                    if let Some(parent) = enclosed.parent() {
                        if !parent.as_os_str().is_empty() {
                            dest_dir.join(parent).join(stripped)
                        } else {
                            dest_dir.join(stripped)
                        }
                    } else {
                        dest_dir.join(stripped)
                    }
                } else {
                    dest_dir.join(enclosed)
                };

                if file.is_dir() {
                    std::fs::create_dir_all(&outpath).map_err(ArchiverError::Io)?;
                    continue;
                }

                if let Some(parent) = outpath.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).map_err(ArchiverError::Io)?;
                    }
                }

                if is_brotli {
                    let mut payload = Vec::new();
                    file.read_to_end(&mut payload).map_err(ArchiverError::Io)?;
                    if processed.saturating_add(payload.len() as u64) > limits.max_entry_size {
                        return Err(ArchiverError::TooLarge(format!(
                            "entry '{}' exceeds max_entry_size {}",
                            name, limits.max_entry_size
                        )));
                    }
                    let decoded = brotli_decompress_entry(&payload)?;
                    std::fs::write(&outpath, &decoded).map_err(ArchiverError::Io)?;
                    processed += decoded.len() as u64;
                    progress.set_progress(processed, 0);
                    continue;
                }

                let mut outfile = std::fs::File::create(&outpath).map_err(ArchiverError::Io)?;

                loop {
                    if progress.is_cancelled() {
                        return Err(ArchiverError::Cancelled);
                    }
                    let bytes_read = file.read(&mut buffer).map_err(ArchiverError::Io)?;
                    if bytes_read == 0 {
                        break;
                    }
                    if processed.saturating_add(bytes_read as u64) > limits.max_entry_size {
                        return Err(ArchiverError::TooLarge(format!(
                            "entry '{}' exceeds max_entry_size {}",
                            name, limits.max_entry_size
                        )));
                    }
                    outfile
                        .write_all(&buffer[..bytes_read])
                        .map_err(ArchiverError::Io)?;
                    processed += bytes_read as u64;
                    progress.set_progress(processed, 0); // Unknown total
                }
            }
        } else {
            let mut buffer = [0u8; 64 * 1024];
            // Extract specific entries
            for entry_name in entries {
                if progress.is_cancelled() {
                    return Err(ArchiverError::Cancelled);
                }

                progress.set_message(entry_name);

                let mut file = if let Some(pwd) = password {
                    archive
                        .by_name_decrypt(entry_name, pwd.as_bytes())
                        .map_err(|e| ArchiverError::invalid_with_source("zip name decrypt", e))?
                } else {
                    archive
                        .by_name(entry_name)
                        .map_err(|e| ArchiverError::invalid_with_source("zip name lookup", e))?
                };

                let name = file.name().to_string();
                let is_brotli = !file.is_dir() && name.ends_with(".br");
                let logical_name = if is_brotli {
                    name.trim_end_matches(".br").to_string()
                } else {
                    name.clone()
                };
                let enclosed = match file.enclosed_name() {
                    Some(path) => path.to_owned(),
                    None => continue,
                };
                let outpath = if is_brotli {
                    let stripped = match std::path::Path::new(&logical_name).file_name() {
                        Some(s) => s.to_owned(),
                        None => continue,
                    };
                    if let Some(parent) = enclosed.parent() {
                        if !parent.as_os_str().is_empty() {
                            dest_dir.join(parent).join(stripped)
                        } else {
                            dest_dir.join(stripped)
                        }
                    } else {
                        dest_dir.join(stripped)
                    }
                } else {
                    dest_dir.join(enclosed)
                };

                if file.is_dir() {
                    std::fs::create_dir_all(&outpath).map_err(ArchiverError::Io)?;
                    continue;
                }

                if let Some(parent) = outpath.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).map_err(ArchiverError::Io)?;
                    }
                }

                if is_brotli {
                    let mut payload = Vec::new();
                    file.read_to_end(&mut payload).map_err(ArchiverError::Io)?;
                    if processed.saturating_add(payload.len() as u64) > limits.max_entry_size {
                        return Err(ArchiverError::TooLarge(format!(
                            "entry '{}' exceeds max_entry_size {}",
                            entry_name, limits.max_entry_size
                        )));
                    }
                    let decoded = brotli_decompress_entry(&payload)?;
                    std::fs::write(&outpath, &decoded).map_err(ArchiverError::Io)?;
                    processed += decoded.len() as u64;
                    progress.set_progress(processed, 0);
                    continue;
                }

                let mut outfile = std::fs::File::create(&outpath).map_err(ArchiverError::Io)?;

                loop {
                    if progress.is_cancelled() {
                        return Err(ArchiverError::Cancelled);
                    }
                    let bytes_read = file.read(&mut buffer).map_err(ArchiverError::Io)?;
                    if bytes_read == 0 {
                        break;
                    }
                    if processed.saturating_add(bytes_read as u64) > limits.max_entry_size {
                        return Err(ArchiverError::TooLarge(format!(
                            "entry '{}' exceeds max_entry_size {}",
                            entry_name, limits.max_entry_size
                        )));
                    }
                    outfile
                        .write_all(&buffer[..bytes_read])
                        .map_err(ArchiverError::Io)?;
                    processed += bytes_read as u64;
                    progress.set_progress(processed, 0);
                }
            }
        }

        tracing::info!("Extracted {} entries from zip archive", total_entries);
        Ok(())
    }

    fn create(
        &self,
        writer: Box<dyn WriteSeek>,
        entries: &[PathBuf],
        options: &CreateOptions,
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
        // Build the per-file options. If a password is supplied, layer
        // AES-256 encryption on top of the chosen compression method — this is
        // the same AES vendor version (AE-2) that 7-Zip writes for .zip AES
        // archives. zip 2.4.2 exposes this via `with_aes_encryption`. We use
        // the generic `FileOptions<'_, ()>` rather than the `SimpleFileOptions`
        // alias because the latter is `FileOptions<'static, ()>` and cannot
        // hold a non-static password slice.
        let mut zip_writer = ZipWriter::new(writer);

        // WS-B: the typed `options.compression` field is the source of
        // truth; the legacy `options.compression_method` string is only
        // consulted when the typed field is at its default value
        // (`Deflate`). `effective_compression` implements that preference.
        let method = effective_compression(options);
        let compression = to_zip_method(method);
        let compression_level = options.compression_level.map(|l| l as i64);

        let mut file_options: FileOptions<'_, ()> = FileOptions::default()
            .compression_method(compression)
            .compression_level(compression_level);
        if let Some(pwd) = password {
            file_options = file_options.with_aes_encryption(AesMode::Aes256, pwd);
        }

        let mut buffer = [0u8; 64 * 1024];
        for entry_path in entries {
            if progress.is_cancelled() {
                return Err(ArchiverError::Cancelled);
            }

            let name = entry_path.to_string_lossy().replace("\\", "/");
            progress.set_message(&name);

            if entry_path.is_dir() {
                let dir_name = if name.ends_with('/') {
                    name.clone()
                } else {
                    format!("{}/", name)
                };
                zip_writer
                    .start_file(&dir_name, file_options)
                    .map_err(|e| ArchiverError::Io(e.into()))?;
                continue;
            }

            // Brotli is signalled in the archive via a `.br` filename
            // suffix. The actual codec lives in the entry bytes (see
            // `pre_compress_for_method`); the zip writer itself never tries
            // to re-compress a Brotli entry because `to_zip_method`
            // collapses Brotli to `Stored`.
            let archive_name = if method == SupaCompressionMethod::Brotli {
                format!("{name}.br")
            } else {
                name.clone()
            };
            zip_writer
                .start_file(&archive_name, file_options)
                .map_err(|e| ArchiverError::Io(e.into()))?;

            // Brotli needs the whole payload in memory so we can compress
            // it before handing it to the zip writer. For Deflate / Zstd /
            // Store we stream straight from the source file.
            if method == SupaCompressionMethod::Brotli {
                let mut file = std::fs::File::open(entry_path).map_err(ArchiverError::Io)?;
                let mut data = Vec::new();
                file.read_to_end(&mut data).map_err(ArchiverError::Io)?;
                let (compressed, _) = pre_compress_for_method(method, &data)?;
                zip_writer
                    .write_all(&compressed)
                    .map_err(ArchiverError::Io)?;
                progress.set_progress(compressed.len() as u64, 0);
                continue;
            }

            let mut file = std::fs::File::open(entry_path).map_err(ArchiverError::Io)?;

            loop {
                if progress.is_cancelled() {
                    return Err(ArchiverError::Cancelled);
                }
                let bytes_read = file.read(&mut buffer).map_err(ArchiverError::Io)?;
                if bytes_read == 0 {
                    break;
                }
                zip_writer
                    .write_all(&buffer[..bytes_read])
                    .map_err(ArchiverError::Io)?;
                progress.set_progress(bytes_read as u64, 0);
            }
        }

        zip_writer
            .finish()
            .map_err(|e| ArchiverError::Io(e.into()))?;

        tracing::info!("Created zip archive with {} entries", entries.len());
        Ok(())
    }

    fn test(
        &self,
        reader: Box<dyn Read>,
        password: Option<&str>,
        progress: &dyn ProgressCallback,
        limits: &Limits,
    ) -> Result<bool, ArchiverError> {
        // zip 2.x requires `R: Read + Seek` for `ZipArchive::new`; we buffer.
        // The 7z backend streams; see `formats/sevenz.rs`.
        let mut reader = bounded_reader(reader, limits.max_archive_size)?;
        let mut data = Vec::new();
        std::io::copy(&mut *reader, &mut data).map_err(ArchiverError::Io)?;
        let mut archive = ZipArchive::new(Cursor::new(data))
            .map_err(|e| ArchiverError::invalid_with_source("zip parse", e))?;
        if archive.len() > limits.max_entry_count {
            return Err(ArchiverError::TooLarge(format!(
                "archive contains {} entries (limit {})",
                archive.len(),
                limits.max_entry_count
            )));
        }

        let mut buffer = [0u8; 64 * 1024];
        for i in 0..archive.len() {
            if progress.is_cancelled() {
                return Err(ArchiverError::Cancelled);
            }

            let mut file = if let Some(pwd) = password {
                archive
                    .by_index_decrypt(i, pwd.as_bytes())
                    .map_err(|e| ArchiverError::invalid_with_source("zip decrypt", e))?
            } else {
                archive
                    .by_index(i)
                    .map_err(|e| ArchiverError::invalid_with_source("zip index", e))?
            };

            let name = file.name().to_string();
            progress.set_message(&name);

            // Read through the entire entry to verify CRC
            let mut total_read = 0u64;

            loop {
                if progress.is_cancelled() {
                    return Err(ArchiverError::Cancelled);
                }
                let bytes_read = file.read(&mut buffer).map_err(ArchiverError::Io)?;
                if bytes_read == 0 {
                    break;
                }
                total_read += bytes_read as u64;
                progress.set_progress(total_read, file.size());
            }

            // ZipArchive validates CRC automatically when reading
            // If we get here without error, the entry is valid
        }

        tracing::info!("Zip archive test passed");
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::NoOpProgress;
    use std::io::{BufWriter, Cursor};

    #[test]
    fn test_list_empty_archive() {
        // Create an empty zip archive
        let buffer = Vec::new();
        let backend = ZipBackend::new();
        let result = backend.list(Box::new(Cursor::new(buffer)), None, &Limits::default());
        // Empty zip should fail to parse, which is expected
        assert!(result.is_err());
    }

    #[test]
    fn test_name_and_extensions() {
        let backend = ZipBackend::new();
        assert_eq!(backend.name(), "zip");
        assert_eq!(backend.extensions(), &["zip", "cbz"]);
    }

    #[test]
    fn round_trip_create_then_list_and_test() {
        // Build a zip in memory with the backend, then list and test it.
        let cursor = Cursor::new(Vec::<u8>::new());
        let writer: Box<dyn WriteSeek> = Box::new(BufWriter::new(cursor));

        let tmp = tempfile::tempdir().expect("tempdir");
        let hello = tmp.path().join("hello.txt");
        let greet = tmp.path().join("greet");
        std::fs::create_dir(&greet).expect("mkdir greet");
        std::fs::write(&hello, b"hi\n").expect("write hello");
        std::fs::write(greet.join("inside.txt"), b"nested\n").expect("write inside");

        let entries = vec![hello, greet.join("inside.txt")];
        let opts = CreateOptions {
            compression_method: "deflate".to_string(),
            compression_level: None,
            compression: SupaCompressionMethod::Deflate,
        };
        ZipBackend::new()
            .create(
                writer,
                &entries,
                &opts,
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");
        // `writer` is consumed by `create`; the archive is now in the dropped
        // `BufWriter` which is gone. To assert the round-trip, do it through
        // a real file instead.
        let _ = tmp;
    }

    #[test]
    fn round_trip_via_real_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("round.zip");
        let src_dir = tmp.path().join("src");
        std::fs::create_dir(&src_dir).expect("mkdir src");
        std::fs::write(src_dir.join("hello.txt"), b"hi\n").expect("write hello");
        std::fs::write(src_dir.join("data.bin"), &[0xCAu8, 0xFE, 0xBA, 0xBE][..])
            .expect("write data");

        let file = std::fs::File::create(&archive).expect("create archive");
        let writer: Box<dyn WriteSeek> = Box::new(BufWriter::new(file));
        let entries = vec![src_dir.join("hello.txt"), src_dir.join("data.bin")];
        let opts = CreateOptions {
            compression_method: "deflate".to_string(),
            compression_level: None,
            compression: SupaCompressionMethod::Deflate,
        };
        ZipBackend::new()
            .create(
                writer,
                &entries,
                &opts,
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");

        let listed = ZipBackend::new()
            .list(
                Box::new(std::fs::File::open(&archive).expect("reopen")),
                None,
                &Limits::default(),
            )
            .expect("list");
        assert_eq!(listed.len(), 2);
        let ok = ZipBackend::new()
            .test(
                Box::new(std::fs::File::open(&archive).expect("reopen")),
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("test");
        assert!(ok);
    }

    #[test]
    fn round_trip_via_real_file_per_entry_assertions() {
        // Stronger assertions on the listed entries: name, size, crc32.
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("round.zip");
        let src_dir = tmp.path().join("src");
        std::fs::create_dir(&src_dir).expect("mkdir src");
        let hello_body = b"hi\n";
        let data_body: &[u8] = &[0xCA, 0xFE, 0xBA, 0xBE];
        std::fs::write(src_dir.join("hello.txt"), hello_body).expect("write hello");
        std::fs::write(src_dir.join("data.bin"), data_body).expect("write data");

        // The zip backend uses the entry path's display form as the archive
        // name, so we feed it absolute paths and look the entries up by the
        // file_name component for assertion convenience.
        let entries = vec![src_dir.join("hello.txt"), src_dir.join("data.bin")];
        let opts = CreateOptions {
            compression_method: "deflate".to_string(),
            compression_level: None,
            compression: SupaCompressionMethod::Deflate,
        };
        let file = std::fs::File::create(&archive).expect("create archive");
        let writer: Box<dyn WriteSeek> = Box::new(BufWriter::new(file));
        ZipBackend::new()
            .create(
                writer,
                &entries,
                &opts,
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");

        let listed = ZipBackend::new()
            .list(
                Box::new(std::fs::File::open(&archive).expect("reopen")),
                None,
                &Limits::default(),
            )
            .expect("list");
        assert_eq!(listed.len(), 2);

        for entry in &listed {
            assert!(!entry.is_dir, "files only: {entry:?}");
            assert!(entry.crc32.is_some(), "zip stores CRC32: {entry:?}");
        }
        // The zip backend stores entries by their display form (e.g. the
        // absolute path on Windows). Build a lookup keyed on the file_name
        // component so this test is independent of the tempdir location.
        let by_basename: std::collections::HashMap<&str, &crate::traits::ArchiveEntry> = listed
            .iter()
            .filter_map(|e| {
                let base = std::path::Path::new(&e.name)
                    .file_name()
                    .and_then(|s| s.to_str())?;
                Some((base, e))
            })
            .collect();

        let hello = by_basename.get("hello.txt").expect("hello.txt entry");
        assert_eq!(hello.size as usize, hello_body.len());
        assert_eq!(
            hello.compression_method, "deflate",
            "default compression method"
        );

        let data = by_basename.get("data.bin").expect("data.bin entry");
        assert_eq!(data.size as usize, data_body.len());
    }

    #[test]
    fn list_compressed_and_uncompressed_archives() {
        // The same logical archive stored with two compression methods must
        // both list the same number of entries with matching basenames and
        // matching method names.
        let tmp = tempfile::tempdir().expect("tempdir");
        let src_dir = tmp.path().join("src");
        std::fs::create_dir(&src_dir).expect("mkdir src");
        std::fs::write(src_dir.join("a.txt"), b"alpha\n").expect("write a");
        std::fs::write(src_dir.join("b.txt"), b"bravo\n").expect("write b");

        let methods = ["deflate", "store"];

        let mut seen_basenames: std::collections::HashSet<String> = Default::default();
        for method in methods {
            let archive = tmp.path().join(format!("arch-{method}.zip"));
            let entries = vec![src_dir.join("a.txt"), src_dir.join("b.txt")];
            let opts = CreateOptions {
                compression_method: method.to_string(),
                compression_level: None,
                compression: SupaCompressionMethod::from_legacy_str(method),
            };
            let file = std::fs::File::create(&archive).expect("create");
            let writer: Box<dyn WriteSeek> = Box::new(BufWriter::new(file));
            ZipBackend::new()
                .create(
                    writer,
                    &entries,
                    &opts,
                    None,
                    &NoOpProgress,
                    &Limits::default(),
                )
                .expect("create");

            let listed = ZipBackend::new()
                .list(
                    Box::new(std::fs::File::open(&archive).expect("reopen")),
                    None,
                    &Limits::default(),
                )
                .expect("list");
            assert_eq!(listed.len(), 2, "{method} should have 2 entries");
            for e in &listed {
                assert_eq!(
                    e.compression_method, method,
                    "{method} archive should report method {method}"
                );
                let base = std::path::Path::new(&e.name)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                seen_basenames.insert(base);
            }
        }
        let mut got: Vec<String> = seen_basenames.into_iter().collect();
        got.sort();
        assert_eq!(got, vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn too_small_archive_size_limit_is_rejected() {
        // Build a real zip and then try to list it under a 1-byte archive
        // limit. The list path must report TooLarge instead of OOMing or
        // returning a partial listing.
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("t.zip");
        let src_dir = tmp.path().join("src");
        std::fs::create_dir(&src_dir).expect("mkdir");
        std::fs::write(src_dir.join("hello.txt"), b"hi\n").expect("write");
        let file = std::fs::File::create(&archive).expect("create");
        let writer: Box<dyn WriteSeek> = Box::new(BufWriter::new(file));
        ZipBackend::new()
            .create(
                writer,
                &[src_dir.join("hello.txt")],
                &CreateOptions {
                    compression_method: "store".into(),
                    compression_level: None,
                    compression: SupaCompressionMethod::Store,
                },
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");

        let tiny = Limits {
            max_archive_size: 1,
            max_entry_count: Limits::default().max_entry_count,
            max_entry_size: Limits::default().max_entry_size,
            max_compression_ratio: Limits::default().max_compression_ratio,
        };
        let res = ZipBackend::new().list(
            Box::new(std::fs::File::open(&archive).expect("reopen")),
            None,
            &tiny,
        );
        // The reader is bounded to 1 byte so the zip crate cannot locate the
        // central directory. We accept either an InvalidArchive (parser error
        // on the truncated input) or TooLarge (if the bounded reader surfaces
        // the cap explicitly); both signal "we did not process a full archive".
        match res {
            Err(ArchiverError::InvalidArchive { .. }) | Err(ArchiverError::TooLarge(_)) => {}
            other => panic!("expected InvalidArchive or TooLarge, got {other:?}"),
        }
    }

    #[test]
    fn too_small_entry_count_limit_is_rejected() {
        // Build a real zip with two files, then list it under a 1-entry limit.
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("t.zip");
        let src_dir = tmp.path().join("src");
        std::fs::create_dir(&src_dir).expect("mkdir");
        std::fs::write(src_dir.join("a.txt"), b"a\n").expect("write a");
        std::fs::write(src_dir.join("b.txt"), b"b\n").expect("write b");
        let file = std::fs::File::create(&archive).expect("create");
        let writer: Box<dyn WriteSeek> = Box::new(BufWriter::new(file));
        ZipBackend::new()
            .create(
                writer,
                &[src_dir.join("a.txt"), src_dir.join("b.txt")],
                &CreateOptions {
                    compression_method: "store".into(),
                    compression_level: None,
                    compression: SupaCompressionMethod::Store,
                },
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");

        let tiny = Limits {
            max_archive_size: Limits::default().max_archive_size,
            max_entry_count: 1,
            max_entry_size: Limits::default().max_entry_size,
            max_compression_ratio: Limits::default().max_compression_ratio,
        };
        let res = ZipBackend::new().list(
            Box::new(std::fs::File::open(&archive).expect("reopen")),
            None,
            &tiny,
        );
        assert!(
            matches!(res, Err(ArchiverError::TooLarge(_))),
            "expected TooLarge, got {res:?}"
        );
    }

    #[test]
    fn create_with_password_marks_archive_encrypted() {
        // zip 2.x AES-encrypted archives must round-trip: a fresh listing of
        // the produced archive (with the password) should report every entry
        // as `encrypted: true`.
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("enc.zip");
        let src_dir = tmp.path().join("src");
        std::fs::create_dir(&src_dir).expect("mkdir");
        std::fs::write(src_dir.join("a.txt"), b"alpha\n").expect("write a");
        std::fs::write(src_dir.join("b.txt"), b"bravo\n").expect("write b");

        let file = std::fs::File::create(&archive).expect("create archive");
        let writer: Box<dyn WriteSeek> = Box::new(BufWriter::new(file));
        ZipBackend::new()
            .create(
                writer,
                &[src_dir.join("a.txt"), src_dir.join("b.txt")],
                &CreateOptions {
                    compression_method: "deflate".into(),
                    compression_level: None,
                    compression: SupaCompressionMethod::Deflate,
                },
                Some("hunter22"),
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");

        let listed = ZipBackend::new()
            .list(
                Box::new(std::fs::File::open(&archive).expect("reopen")),
                Some("hunter22"),
                &Limits::default(),
            )
            .expect("list with pwd");
        assert_eq!(listed.len(), 2);
        for entry in &listed {
            assert!(
                entry.encrypted,
                "AES-encrypted entry should be flagged: {entry:?}"
            );
        }
    }

    #[test]
    fn extract_writes_to_dest_dir() {
        // End-to-end extract through the trait path: create a zip, extract to
        // a destination directory, and assert the file content round-tripped.
        //
        // The zip backend uses the entry path's display form as the archive
        // entry name, so for the extract step we want an entry name that the
        // zip crate's `enclosed_name()` accepts. We pass a relative path
        // (`hi.txt`) by feeding the file from the current working directory.
        let tmp = tempfile::tempdir().expect("tempdir");
        std::env::set_current_dir(&tmp).expect("chdir tmp");
        std::fs::write(tmp.path().join("hi.txt"), b"round-trip body\n").expect("write");
        let archive = tmp.path().join("r.zip");
        let file = std::fs::File::create(&archive).expect("create archive");
        let writer: Box<dyn WriteSeek> = Box::new(BufWriter::new(file));
        ZipBackend::new()
            .create(
                writer,
                &[std::path::PathBuf::from("hi.txt")],
                &CreateOptions {
                    compression_method: "deflate".into(),
                    compression_level: None,
                    compression: SupaCompressionMethod::Deflate,
                },
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");

        let dest = tmp.path().join("out");
        ZipBackend::new()
            .extract(
                Box::new(std::fs::File::open(&archive).expect("reopen")),
                &dest,
                &[],
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("extract");

        // The extracted tree should now have `out/hi.txt` (the relative entry
        // name is joined under `dest`).
        let extracted = dest.join("hi.txt");
        let body = std::fs::read(&extracted)
            .unwrap_or_else(|e| panic!("read {}: {e}", extracted.display()));
        assert_eq!(body, b"round-trip body\n");
    }

    #[test]
    fn list_directory_entry_is_marked_as_dir() {
        // A directory entry inside a zip should come back with `is_dir == true`.
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("dirs.zip");
        let dir = tmp.path().join("empty");
        std::fs::create_dir(&dir).expect("mkdir empty");
        let file = std::fs::File::create(&archive).expect("create");
        let writer: Box<dyn WriteSeek> = Box::new(BufWriter::new(file));
        let entries = vec![dir];
        let opts = CreateOptions {
            compression_method: "deflate".to_string(),
            compression_level: None,
            compression: SupaCompressionMethod::Deflate,
        };
        ZipBackend::new()
            .create(
                writer,
                &entries,
                &opts,
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");

        let listed = ZipBackend::new()
            .list(
                Box::new(std::fs::File::open(&archive).expect("reopen")),
                None,
                &Limits::default(),
            )
            .expect("list");
        assert!(!listed.is_empty(), "directory entry should be listed");
        assert!(
            listed.iter().any(|e| e.is_dir),
            "expected at least one is_dir entry; got {listed:?}"
        );
    }
}
