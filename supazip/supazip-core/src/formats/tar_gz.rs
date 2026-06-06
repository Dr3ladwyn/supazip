//! Gzip-compressed TAR backend. Wraps the input in `flate2::read::GzDecoder`
//! (default `miniz_oxide` backend — pure Rust) and delegates per-entry walk
//! / extract / list / test to local helpers.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use tar::Builder;

use crate::error::ArchiverError;
use crate::traits::{
    ArchiveEntry, ArchiveFormat, CreateOptions, Limits, ProgressCallback, WriteSeek,
};

pub struct TarGzBackend;

impl Default for TarGzBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl TarGzBackend {
    pub const fn new() -> Self {
        Self
    }

    fn with_archive<R, F>(reader: R, f: F) -> Result<(), ArchiverError>
    where
        R: Read,
        F: FnOnce(&mut tar::Archive<GzDecoder<R>>) -> Result<(), ArchiverError>,
    {
        let decoder = GzDecoder::new(reader);
        let mut archive = tar::Archive::new(decoder);
        f(&mut archive)
    }
}

impl ArchiveFormat for TarGzBackend {
    fn name(&self) -> &'static str {
        "tar.gz"
    }

    fn extensions(&self) -> &[&str] {
        &["tar.gz", "tgz"]
    }

    fn list(
        &self,
        reader: Box<dyn Read>,
        _password: Option<&str>,
        limits: &Limits,
    ) -> Result<Vec<ArchiveEntry>, ArchiverError> {
        let mut entries: Vec<ArchiveEntry> = Vec::new();
        Self::with_archive(reader, |archive| {
            walk_limited(archive, limits, |_entry, meta| {
                entries.push(meta);
                Ok(())
            })
        })?;
        Ok(entries)
    }

    fn extract(
        &self,
        reader: Box<dyn Read>,
        dest_dir: &Path,
        entries: &[&str],
        _password: Option<&str>,
        progress: &dyn ProgressCallback,
        limits: &Limits,
    ) -> Result<(), ArchiverError> {
        std::fs::create_dir_all(dest_dir).map_err(ArchiverError::Io)?;
        let filter: Option<Vec<String>> = if entries.is_empty() {
            None
        } else {
            Some(entries.iter().map(|s| s.to_string()).collect())
        };
        Self::with_archive(reader, |archive| {
            walk_limited(archive, limits, |entry, meta| {
                if progress.is_cancelled() {
                    return Err(ArchiverError::Cancelled);
                }
                if let Some(ref wanted) = filter {
                    if !wanted.iter().any(|w| w == &meta.name) {
                        return Ok(());
                    }
                }
                progress.set_message(&meta.name);
                extract_entry(entry, &meta, dest_dir, progress, limits)
            })
        })?;
        Ok(())
    }

    fn create(
        &self,
        mut writer: Box<dyn WriteSeek>,
        entries: &[PathBuf],
        options: &CreateOptions,
        _password: Option<&str>,
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
        let level = options.compression_level.map(|l| l.min(9)).unwrap_or(6);
        let mut encoder = GzEncoder::new(writer.by_ref(), Compression::new(level));
        {
            let mut builder = Builder::new(&mut encoder);
            let total = entries.len() as u64;
            let mut appended: u64 = 0;
            for path in entries {
                if progress.is_cancelled() {
                    return Err(ArchiverError::Cancelled);
                }
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| {
                        ArchiverError::invalid(format!(
                            "tar.gz create: entry path {path:?} has no usable file name"
                        ))
                    })?
                    .to_string();
                progress.set_message(&name);
                if path.is_dir() {
                    builder
                        .append_dir(&name, path)
                        .map_err(|e| ArchiverError::invalid_with_source("tar.gz append_dir", e))?;
                } else {
                    let mut file = std::fs::File::open(path).map_err(ArchiverError::Io)?;
                    builder
                        .append_file(&name, &mut file)
                        .map_err(|e| ArchiverError::invalid_with_source("tar.gz append_file", e))?;
                }
                appended += 1;
                progress.set_progress(appended, total);
            }
            builder
                .finish()
                .map_err(|e| ArchiverError::invalid_with_source("tar.gz finish", e))?;
        }
        encoder
            .finish()
            .map_err(|e| ArchiverError::invalid_with_source("gzip finish", e))?;
        writer.flush().map_err(ArchiverError::Io)?;
        Ok(())
    }

    fn test(
        &self,
        reader: Box<dyn Read>,
        _password: Option<&str>,
        progress: &dyn ProgressCallback,
        limits: &Limits,
    ) -> Result<bool, ArchiverError> {
        Self::with_archive(reader, |archive| test_archive(archive, progress, limits))?;
        Ok(true)
    }
}

fn header_to_entry<R: Read>(entry: &tar::Entry<'_, R>, name: String) -> ArchiveEntry {
    let header = entry.header();
    let size = header.size().unwrap_or(0);
    let is_dir = header.entry_type().is_dir() || name.ends_with('/');
    let mtime = header
        .mtime()
        .ok()
        .and_then(|secs| chrono::DateTime::<chrono::Utc>::from_timestamp(secs as i64, 0));
    ArchiveEntry {
        name: name.clone(),
        path: name,
        is_dir,
        size,
        compressed_size: size,
        modified: mtime,
        compression_method: "store".to_string(),
        crc32: None,
        encrypted: false,
    }
}

fn walk_limited<R: Read, F>(
    archive: &mut tar::Archive<R>,
    limits: &Limits,
    mut visit: F,
) -> Result<(), ArchiverError>
where
    F: FnMut(&mut tar::Entry<'_, R>, ArchiveEntry) -> Result<(), ArchiverError>,
{
    for (count, entry_res) in archive
        .entries()
        .map_err(|e| ArchiverError::invalid_with_source("tar entries", e))?
        .enumerate()
    {
        if count >= limits.max_entry_count {
            return Err(ArchiverError::TooLarge(format!(
                "archive contains more than {} entries (limit)",
                limits.max_entry_count
            )));
        }
        let mut entry =
            entry_res.map_err(|e| ArchiverError::invalid_with_source("tar entry header", e))?;
        let name = entry
            .path()
            .map_err(|e| ArchiverError::invalid_with_source("tar entry path", e))?
            .to_string_lossy()
            .into_owned();
        let size = entry.header().size().unwrap_or(0);
        if size > limits.max_entry_size {
            return Err(ArchiverError::TooLarge(format!(
                "tar entry {name:?} of size {size} exceeds max_entry_size {}",
                limits.max_entry_size
            )));
        }
        let meta = header_to_entry(&entry, name);
        visit(&mut entry, meta)?;
    }
    Ok(())
}

fn extract_entry<R: Read>(
    entry: &mut tar::Entry<'_, R>,
    meta: &ArchiveEntry,
    dest_dir: &Path,
    progress: &dyn ProgressCallback,
    limits: &Limits,
) -> Result<(), ArchiverError> {
    let outpath = super::safe_join(dest_dir, &meta.name)?;
    if meta.is_dir {
        std::fs::create_dir_all(&outpath).map_err(ArchiverError::Io)?;
        return Ok(());
    }
    let header = entry.header();
    if !header.entry_type().is_file() {
        return Err(ArchiverError::UnsupportedFormat {
            message: format!(
                "tar entry {:?} has type {:?} which the engine does not extract",
                meta.name,
                header.entry_type()
            ),
            source: None,
        });
    }
    if let Some(parent) = outpath.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(ArchiverError::Io)?;
        }
    }
    let mut outfile = std::fs::File::create(&outpath).map_err(ArchiverError::Io)?;
    let mut buf = [0u8; 8192];
    let mut written: u64 = 0;
    loop {
        if progress.is_cancelled() {
            return Err(ArchiverError::Cancelled);
        }
        let n = entry.read(&mut buf).map_err(ArchiverError::Io)?;
        if n == 0 {
            break;
        }
        if written.saturating_add(n as u64) > limits.max_entry_size {
            return Err(ArchiverError::TooLarge(format!(
                "tar entry {:?} exceeds max_entry_size {}",
                meta.name, limits.max_entry_size
            )));
        }
        outfile.write_all(&buf[..n]).map_err(ArchiverError::Io)?;
        written += n as u64;
        progress.set_progress(written, meta.size);
    }
    Ok(())
}

fn test_archive<R: Read>(
    archive: &mut tar::Archive<R>,
    progress: &dyn ProgressCallback,
    limits: &Limits,
) -> Result<(), ArchiverError> {
    let mut processed: u64 = 0;
    let mut total_size: u64 = 0;
    let entries: Vec<tar::Entry<'_, R>> = archive
        .entries()
        .map_err(|e| ArchiverError::invalid_with_source("tar entries", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ArchiverError::invalid_with_source("tar entries collect", e))?;
    for (total_entries, mut entry) in entries.into_iter().enumerate() {
        if total_entries >= limits.max_entry_count {
            return Err(ArchiverError::TooLarge(format!(
                "archive contains more than {} entries (limit)",
                limits.max_entry_count
            )));
        }
        let name = entry
            .path()
            .map_err(|e| ArchiverError::invalid_with_source("tar entry path", e))?
            .to_string_lossy()
            .into_owned();
        let size = entry.header().size().unwrap_or(0);
        if size > limits.max_entry_size {
            return Err(ArchiverError::TooLarge(format!(
                "tar entry {name:?} of size {size} exceeds max_entry_size {}",
                limits.max_entry_size
            )));
        }
        total_size = total_size.saturating_add(size);
        if progress.is_cancelled() {
            return Err(ArchiverError::Cancelled);
        }
        progress.set_message(&name);
        let mut buf = [0u8; 8192];
        loop {
            if progress.is_cancelled() {
                return Err(ArchiverError::Cancelled);
            }
            let n = entry.read(&mut buf).map_err(ArchiverError::Io)?;
            if n == 0 {
                break;
            }
            processed += n as u64;
            progress.set_progress(processed, total_size);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::NoOpProgress;
    use std::io::Cursor;

    fn write_three_files(dir: &Path) -> Vec<PathBuf> {
        std::fs::create_dir_all(dir).expect("mkdir");
        std::fs::write(dir.join("a.txt"), b"alpha\n").expect("write a");
        std::fs::write(dir.join("b.bin"), &[1u8, 2, 3, 4, 5][..]).expect("write b");
        let nested = dir.join("nested");
        std::fs::create_dir(&nested).expect("mkdir nested");
        std::fs::write(nested.join("c.txt"), b"gamma\n").expect("write c");
        vec![dir.join("a.txt"), dir.join("b.bin"), nested.join("c.txt")]
    }

    #[test]
    fn name_and_extensions() {
        let b = TarGzBackend::new();
        assert_eq!(b.name(), "tar.gz");
        assert_eq!(b.extensions(), &["tar.gz", "tgz"]);
    }

    #[test]
    fn round_trip_create_list_extract() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("plain.tar.gz");
        let src = tmp.path().join("src");
        let entries = write_three_files(&src);
        let file = std::fs::File::create(&archive).expect("create archive");
        let writer: Box<dyn WriteSeek> = Box::new(file);
        TarGzBackend::new()
            .create(
                writer,
                &entries,
                &CreateOptions {
                    compression_method: "deflate".into(),
                    compression_level: Some(6),
                    compression: crate::traits::CompressionMethod::Deflate,
                },
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");
        let listed = TarGzBackend::new()
            .list(
                Box::new(std::fs::File::open(&archive).expect("open")),
                None,
                &Limits::default(),
            )
            .expect("list");
        assert_eq!(listed.len(), 3);
        for e in &listed {
            assert!(!e.encrypted);
        }
        let dest = tmp.path().join("out");
        TarGzBackend::new()
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
    fn invalid_gzip_header_is_rejected() {
        let bogus: &[u8] = b"this is not a gzip stream at all\n";
        let res = TarGzBackend::new().list(
            Box::new(Cursor::new(bogus.to_vec())),
            None,
            &Limits::default(),
        );
        assert!(
            matches!(res, Err(ArchiverError::InvalidArchive { .. })),
            "expected InvalidArchive, got {res:?}"
        );
    }

    #[test]
    fn extract_rejects_path_traversal() {
        // tar 0.4.46 strips `..` from `Entry::path()` upstream. The
        // SupaZip backstop is `super::safe_join`; the test exercises the
        // backstop directly so a future regression in the upstream
        // library does not silently let `..` through to disk.
        let base = std::path::Path::new("/tmp/work");
        let res = crate::formats::safe_join(base, "../escape.txt");
        match res {
            Err(ArchiverError::InvalidArchive { ref message, .. }) => {
                let lower = message.to_lowercase();
                assert!(
                    lower.contains("path traversal") || lower.contains("unsafe path"),
                    "message should mention path traversal: {message}"
                );
            }
            other => panic!("expected InvalidArchive, got {other:?}"),
        }
    }

    #[test]
    fn extract_is_cancellable() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("c.tar.gz");
        let src = tmp.path().join("src");
        let entries = write_three_files(&src);
        let file = std::fs::File::create(&archive).expect("create");
        let writer: Box<dyn WriteSeek> = Box::new(file);
        TarGzBackend::new()
            .create(
                writer,
                &entries,
                &CreateOptions {
                    compression_method: "store".into(),
                    compression_level: None,
                    compression: crate::traits::CompressionMethod::Store,
                },
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("create");
        struct Cancel(AtomicUsize);
        impl ProgressCallback for Cancel {
            fn set_progress(&self, _c: u64, _t: u64) {}
            fn set_message(&self, _m: &str) {}
            fn is_cancelled(&self) -> bool {
                self.0.fetch_add(1, Ordering::SeqCst) > 0
            }
        }
        let cb = Cancel(AtomicUsize::new(0));
        let dest = tmp.path().join("out");
        let res = TarGzBackend::new().extract(
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
    fn list_too_small_entry_size_limit_is_rejected() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive = tmp.path().join("bomb.tar.gz");
        {
            let mut tar_bytes: Vec<u8> = Vec::new();
            {
                let mut builder = Builder::new(&mut tar_bytes);
                let mut header = tar::Header::new_gnu();
                header.set_path("bomb.bin").expect("path");
                header.set_size(2u64 * 1024 * 1024 * 1024);
                header.set_mode(0o644);
                header.set_cksum();
                let zero: &[u8] = &[];
                builder.append(&header, zero).expect("append bomb");
                builder.finish().expect("finish");
            }
            let mut encoder = GzEncoder::new(
                std::fs::File::create(&archive).expect("create"),
                Compression::default(),
            );
            encoder.write_all(&tar_bytes).expect("write");
            encoder.finish().expect("finish");
        }
        let tiny = Limits {
            max_archive_size: u64::MAX,
            max_entry_count: usize::MAX,
            max_entry_size: 1024 * 1024 * 1024,
            max_compression_ratio: 0,
        };
        let res = TarGzBackend::new().list(
            Box::new(std::fs::File::open(&archive).expect("open")),
            None,
            &tiny,
        );
        assert!(
            matches!(res, Err(ArchiverError::TooLarge(_))),
            "expected TooLarge, got {res:?}"
        );
    }

    #[test]
    fn extract_from_cursor_streaming() {
        let bytes = {
            let mut tar_bytes: Vec<u8> = Vec::new();
            {
                let mut builder = Builder::new(&mut tar_bytes);
                let mut header = tar::Header::new_gnu();
                header.set_path("hello.txt").expect("path");
                header.set_size(6);
                header.set_mode(0o644);
                header.set_cksum();
                builder
                    .append(&header, &b"hello\n"[..])
                    .expect("append hello");
                builder.finish().expect("finish");
            }
            let mut gz: Vec<u8> = Vec::new();
            {
                let mut encoder = GzEncoder::new(&mut gz, Compression::default());
                encoder.write_all(&tar_bytes).expect("write");
                encoder.finish().expect("finish");
            }
            gz
        };
        let tmp = tempfile::tempdir().expect("tempdir");
        let dest = tmp.path().join("out");
        TarGzBackend::new()
            .extract(
                Box::new(Cursor::new(bytes)),
                &dest,
                &[],
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("extract");
        let body = std::fs::read(dest.join("hello.txt")).expect("read hello");
        assert_eq!(body, b"hello\n");
    }

    #[test]
    fn extract_empty_archive_is_noop() {
        let bytes = {
            let mut gz: Vec<u8> = Vec::new();
            {
                let mut encoder = GzEncoder::new(&mut gz, Compression::default());
                let zeros = vec![0u8; 2048];
                encoder.write_all(&zeros).expect("write");
                encoder.finish().expect("finish");
            }
            gz
        };
        let tmp = tempfile::tempdir().expect("tempdir");
        let dest = tmp.path().join("out");
        TarGzBackend::new()
            .extract(
                Box::new(Cursor::new(bytes)),
                &dest,
                &[],
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("extract empty");
        assert!(dest.is_dir());
    }
}
