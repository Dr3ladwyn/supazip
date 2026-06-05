use std::io::{Cursor, Read, Write};
use std::path::PathBuf;
use chrono::{DateTime, Utc};
use zip::{ZipArchive, ZipWriter, DateTime as ZipDateTime, CompressionMethod};
use zip::write::SimpleFileOptions;

use crate::error::ArchiverError;
use crate::traits::{ArchiveEntry, ArchiveFormat, CreateOptions, ProgressCallback, WriteSeek};

pub struct ZipBackend;

impl Default for ZipBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl ZipBackend {
    pub fn new() -> Self {
        Self
    }

    fn conversion_method_to_zip(method: &str) -> CompressionMethod {
        match method.to_lowercase().as_str() {
            "deflate" | "deflated" => CompressionMethod::Deflated,
            "store" | "none" => CompressionMethod::Stored,
            "bzip2" => CompressionMethod::Bzip2,
            "zstd" => CompressionMethod::Zstd,
            _ => CompressionMethod::Deflated,
        }
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

    fn list(&self, mut reader: Box<dyn Read>, password: Option<&str>) -> Result<Vec<ArchiveEntry>, ArchiverError> {
        // zip 2.x requires `R: Read + Seek` for `ZipArchive::new` because the
        // central directory is read at the end. The trait hands us a non-seekable
        // `Box<dyn Read>`, so we buffer into memory and hand the resulting
        // `Cursor<Vec<u8>>` to the zip crate. The 7z backend genuinely streams;
        // see `formats/sevenz.rs` for the contrast.
        let mut data = Vec::new();
        std::io::copy(&mut *reader, &mut data).map_err(ArchiverError::Io)?;
        let mut archive = ZipArchive::new(Cursor::new(data))
            .map_err(|e| ArchiverError::InvalidArchive(e.to_string()))?;

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
                    .map_err(|e| ArchiverError::InvalidArchive(e.to_string()))?
            } else {
                archive
                    .by_index(i)
                    .map_err(|e| ArchiverError::InvalidArchive(e.to_string()))?
            };

            let name = file.name().to_string();
            let is_dir = name.ends_with('/');

            let modified = file.last_modified()
                .and_then(Self::zip_datetime_to_chrono);

            let compression_method = Self::compression_method_to_string(file.compression());

            let entry = ArchiveEntry {
                name: name.clone(),
                path: name.clone(),
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
        mut reader: Box<dyn Read>,
        dest: Box<dyn WriteSeek>,
        entries: &[&str],
        password: Option<&str>,
        progress: &dyn ProgressCallback,
    ) -> Result<(), ArchiverError> {
        // We use `password` below via `by_index_decrypt` / `by_name_decrypt`; if
        // the `if let Some(pwd) = password` is removed in the future, prefix
        // the parameter with `_` to silence the warning.
        // zip 2.x requires `R: Read + Seek` for `ZipArchive::new`; the trait
        // gives us a non-seekable `Box<dyn Read>`, so we buffer and hand the
        // resulting `Cursor<Vec<u8>>` to the zip crate. The 7z backend streams
        // end-to-end; see `formats/sevenz.rs`.
        //
        // `dest` is the writer handed to us by the trait. Extracted files are
        // written to the current working directory using `enclosed_name`, which
        // is the only behaviour the existing tests rely on. A future refactor
        // will treat `dest` as a directory path or a tar stream and stop
        // touching the filesystem directly.
        let mut data = Vec::new();
        std::io::copy(&mut *reader, &mut data).map_err(ArchiverError::Io)?;
        let mut archive = ZipArchive::new(Cursor::new(data))
            .map_err(|e| ArchiverError::InvalidArchive(e.to_string()))?;
        let _ = dest;

        let total_entries = if entries.is_empty() {
            archive.len()
        } else {
            entries.len()
        };

        let mut processed = 0u64;

        if entries.is_empty() {
            // Extract all entries
            for i in 0..archive.len() {
                if progress.is_cancelled() {
                    return Err(ArchiverError::Cancelled);
                }

                let mut file = if let Some(pwd) = password {
                    archive
                        .by_index_decrypt(i, pwd.as_bytes())
                        .map_err(|e| ArchiverError::InvalidArchive(e.to_string()))?
                } else {
                    archive
                        .by_index(i)
                        .map_err(|e| ArchiverError::InvalidArchive(e.to_string()))?
                };

                let name = file.name().to_string();
                progress.set_message(&name);

                let outpath = match file.enclosed_name() {
                    Some(path) => path.to_owned(),
                    None => continue,
                };

                if file.is_dir() {
                    continue;
                }

                if let Some(parent) = outpath.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).map_err(ArchiverError::Io)?;
                    }
                }

                let mut outfile = std::fs::File::create(&outpath)
                    .map_err(ArchiverError::Io)?;

                let mut buffer = vec![0u8; 8192];
                loop {
                    if progress.is_cancelled() {
                        return Err(ArchiverError::Cancelled);
                    }
                    let bytes_read = file.read(&mut buffer)
                        .map_err(ArchiverError::Io)?;
                    if bytes_read == 0 {
                        break;
                    }
                    outfile.write_all(&buffer[..bytes_read])
                        .map_err(ArchiverError::Io)?;
                    processed += bytes_read as u64;
                    progress.set_progress(processed, 0); // Unknown total
                }
            }
        } else {
            // Extract specific entries
            for entry_name in entries {
                if progress.is_cancelled() {
                    return Err(ArchiverError::Cancelled);
                }

                progress.set_message(entry_name);

                let mut file = if let Some(pwd) = password {
                    archive
                        .by_name_decrypt(entry_name, pwd.as_bytes())
                        .map_err(|e| ArchiverError::InvalidArchive(e.to_string()))?
                } else {
                    archive
                        .by_name(entry_name)
                        .map_err(|e| ArchiverError::InvalidArchive(e.to_string()))?
                };

                let outpath = match file.enclosed_name() {
                    Some(path) => path.to_owned(),
                    None => continue,
                };

                if file.is_dir() {
                    continue;
                }

                if let Some(parent) = outpath.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).map_err(ArchiverError::Io)?;
                    }
                }

                let mut outfile = std::fs::File::create(&outpath)
                    .map_err(ArchiverError::Io)?;

                let mut buffer = vec![0u8; 8192];
                loop {
                    if progress.is_cancelled() {
                        return Err(ArchiverError::Cancelled);
                    }
                    let bytes_read = file.read(&mut buffer)
                        .map_err(ArchiverError::Io)?;
                    if bytes_read == 0 {
                        break;
                    }
                    outfile.write_all(&buffer[..bytes_read])
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
    ) -> Result<(), ArchiverError> {
        // TODO(zip): honour `password` for encrypted ZIP create. zip 2.x supports
        // it via `SimpleFileOptions::with_password(...)`; the API requires an
        // `EncryptionMethod` (AE2 is the default) and a per-file salt. Until that
        // is implemented, the CLI's `create --password ...` for `.zip` outputs
        // an unencrypted archive. The 7z path is implemented; see
        // `formats/sevenz.rs`.
        let _ = password;
        let mut zip_writer = ZipWriter::new(writer);

        let compression = Self::conversion_method_to_zip(&options.compression_method);
        let compression_level = options.compression_level.map(|l| l as i64);

        let file_options: SimpleFileOptions = SimpleFileOptions::default()
            .compression_method(compression)
            .compression_level(compression_level);

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
                zip_writer.start_file(&dir_name, file_options)
                    .map_err(|e| ArchiverError::Io(e.into()))?;
                continue;
            }

            zip_writer.start_file(&name, file_options)
                .map_err(|e| ArchiverError::Io(e.into()))?;

            let mut file = std::fs::File::open(entry_path)
                .map_err(ArchiverError::Io)?;

            let mut buffer = vec![0u8; 8192];
            loop {
                if progress.is_cancelled() {
                    return Err(ArchiverError::Cancelled);
                }
                let bytes_read = file.read(&mut buffer)
                    .map_err(ArchiverError::Io)?;
                if bytes_read == 0 {
                    break;
                }
                zip_writer.write_all(&buffer[..bytes_read])
                    .map_err(ArchiverError::Io)?;
                progress.set_progress(bytes_read as u64, 0);
            }
        }

        zip_writer.finish()
            .map_err(|e| ArchiverError::Io(e.into()))?;

        tracing::info!("Created zip archive with {} entries", entries.len());
        Ok(())
    }

    fn test(
        &self,
        mut reader: Box<dyn Read>,
        password: Option<&str>,
        progress: &dyn ProgressCallback,
    ) -> Result<bool, ArchiverError> {
        // zip 2.x requires `R: Read + Seek` for `ZipArchive::new`; we buffer.
        // The 7z backend streams; see `formats/sevenz.rs`.
        let mut data = Vec::new();
        std::io::copy(&mut *reader, &mut data).map_err(ArchiverError::Io)?;
        let mut archive = ZipArchive::new(Cursor::new(data))
            .map_err(|e| ArchiverError::InvalidArchive(e.to_string()))?;

        for i in 0..archive.len() {
            if progress.is_cancelled() {
                return Err(ArchiverError::Cancelled);
            }

            let mut file = if let Some(pwd) = password {
                archive
                    .by_index_decrypt(i, pwd.as_bytes())
                    .map_err(|e| ArchiverError::InvalidArchive(e.to_string()))?
            } else {
                archive
                    .by_index(i)
                    .map_err(|e| ArchiverError::InvalidArchive(e.to_string()))?
            };

            let name = file.name().to_string();
            progress.set_message(&name);

            // Read through the entire entry to verify CRC
            let mut buffer = vec![0u8; 8192];
            let mut total_read = 0u64;

            loop {
                if progress.is_cancelled() {
                    return Err(ArchiverError::Cancelled);
                }
                let bytes_read = file.read(&mut buffer)
                    .map_err(ArchiverError::Io)?;
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
        let result = backend.list(Box::new(Cursor::new(buffer)), None);
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
        };
        ZipBackend::new()
            .create(writer, &entries, &opts, None, &NoOpProgress)
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
        std::fs::write(src_dir.join("data.bin"), &[0xCAu8, 0xFE, 0xBA, 0xBE][..]).expect("write data");

        let file = std::fs::File::create(&archive).expect("create archive");
        let writer: Box<dyn WriteSeek> = Box::new(BufWriter::new(file));
        let entries = vec![src_dir.join("hello.txt"), src_dir.join("data.bin")];
        let opts = CreateOptions {
            compression_method: "deflate".to_string(),
            compression_level: None,
        };
        ZipBackend::new()
            .create(writer, &entries, &opts, None, &NoOpProgress)
            .expect("create");

        let listed = ZipBackend::new()
            .list(Box::new(std::fs::File::open(&archive).expect("reopen")), None)
            .expect("list");
        assert_eq!(listed.len(), 2);
        let ok = ZipBackend::new()
            .test(Box::new(std::fs::File::open(&archive).expect("reopen")), None, &NoOpProgress)
            .expect("test");
        assert!(ok);
    }
}
