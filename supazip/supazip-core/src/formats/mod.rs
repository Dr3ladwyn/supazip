mod sevenz;
mod tar;
mod tar_gz;
mod tar_xz;
mod zip;

pub use sevenz::SevenZBackend;
pub use tar::TarBackend;
pub use tar_gz::TarGzBackend;
pub use tar_xz::TarXzBackend;
pub use zip::ZipBackend;

use crate::error::ArchiverError;
use crate::traits::ArchiveFormat;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

// Global registry of backends by extension
pub static BACKENDS: LazyLock<HashMap<&'static str, &'static dyn ArchiveFormat>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();
        let sevenz: &'static SevenZBackend = Box::leak(Box::new(SevenZBackend::new()));
        let zip: &'static ZipBackend = Box::leak(Box::new(ZipBackend::new()));
        let tar: &'static TarBackend = Box::leak(Box::new(TarBackend::new()));
        let tar_gz: &'static TarGzBackend = Box::leak(Box::new(TarGzBackend::new()));
        let tar_xz: &'static TarXzBackend = Box::leak(Box::new(TarXzBackend::new()));

        for ext in sevenz.extensions() {
            map.insert(*ext, sevenz as &dyn ArchiveFormat);
        }
        for ext in zip.extensions() {
            map.insert(*ext, zip as &dyn ArchiveFormat);
        }
        for ext in tar.extensions() {
            map.insert(*ext, tar as &dyn ArchiveFormat);
        }
        for ext in tar_gz.extensions() {
            map.insert(*ext, tar_gz as &dyn ArchiveFormat);
        }
        for ext in tar_xz.extensions() {
            map.insert(*ext, tar_xz as &dyn ArchiveFormat);
        }

        map
    });

/// Detect archive format from filename. The reader argument is reserved for a
/// future magic-byte-sniffing implementation; today the detection is purely
/// filename-driven.
pub fn detect_format<R: std::io::Read>(
    _reader: &mut R,
    filename: Option<&str>,
) -> Option<&'static dyn ArchiveFormat> {
    if let Some(name) = filename {
        let lower = name.to_ascii_lowercase();
        // Compound extensions first so `foo.tar.gz` does not collapse to a
        // bare "gz" lookup. We check every suffix from the longest
        // two-component form down to the single-component one.
        for ext in ["tar.gz", "tar.xz", "tar.bz2", "tar.zst"] {
            if lower.ends_with(&format!(".{ext}")) {
                if let Some(backend) = BACKENDS.get(ext) {
                    return Some(*backend);
                }
            }
        }
        if let Some(ext) = Path::new(name).extension().and_then(|s| s.to_str()) {
            if let Some(backend) = BACKENDS.get(ext.to_ascii_lowercase().as_str()) {
                return Some(*backend);
            }
        }
    }

    // Magic-byte sniffing is not implemented yet; the reader argument is kept
    // in the signature for the future implementation that will read the first
    // few bytes to disambiguate archives without a recognised extension.
    None
}

/// Get backend by extension
pub fn get_backend(extension: &str) -> Option<&'static dyn ArchiveFormat> {
    let lower = extension.to_ascii_lowercase();
    BACKENDS.get(lower.as_str()).copied()
}

/// List all supported extensions
pub fn supported_extensions() -> Vec<&'static str> {
    BACKENDS.keys().copied().collect()
}

/// Join `entry` onto `base` while refusing path traversal. Used by every
/// extract path that materialises user-supplied archive entries on disk so a
/// `../escape` payload cannot be smuggled past the engine.
///
/// Returns `ArchiverError::InvalidArchive` with a free-form message when the
/// path is empty, contains `..`, or escapes `base` once joined.
pub(crate) fn safe_join(base: &Path, entry: &str) -> Result<PathBuf, ArchiverError> {
    if entry.is_empty() {
        return Err(crate::error::ArchiverError::invalid("empty entry name"));
    }
    if entry.contains("..") {
        return Err(crate::error::ArchiverError::invalid(format!(
            "unsafe path: path traversal in {entry:?}"
        )));
    }
    let p = base.join(entry);
    if !p.starts_with(base) {
        return Err(crate::error::ArchiverError::invalid(format!(
            "unsafe path: escape attempt in {entry:?}"
        )));
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backends_contains_zip_and_sevenz() {
        let exts: Vec<&str> = supported_extensions();
        assert!(exts.contains(&"zip"), "zip backend missing: {exts:?}");
        assert!(exts.contains(&"7z"), "7z backend missing: {exts:?}");
    }

    #[test]
    fn backends_contains_tar_variants() {
        let exts: Vec<&str> = supported_extensions();
        assert!(exts.contains(&"tar"), "tar backend missing: {exts:?}");
        assert!(exts.contains(&"tar.gz"), "tar.gz missing: {exts:?}");
        assert!(exts.contains(&"tgz"), "tgz missing: {exts:?}");
        assert!(exts.contains(&"tar.xz"), "tar.xz missing: {exts:?}");
        assert!(exts.contains(&"txz"), "txz missing: {exts:?}");
    }

    #[test]
    fn get_backend_is_case_insensitive() {
        assert!(get_backend("ZIP").is_some());
        assert!(get_backend("Zip").is_some());
        assert!(get_backend("7z").is_some());
        assert!(get_backend("7Z").is_some());
        assert!(get_backend("TAR").is_some());
        assert!(get_backend("TAR.GZ").is_some());
        assert!(get_backend("TAR.XZ").is_some());
    }

    #[test]
    fn get_backend_unknown_returns_none() {
        assert!(get_backend("rar").is_none());
        assert!(get_backend("").is_none());
    }

    #[test]
    fn detect_format_picks_by_extension() {
        let mut empty: &[u8] = &[];
        let z = detect_format(&mut empty, Some("foo.zip")).expect("zip backend");
        assert_eq!(z.name(), "zip");
        let s = detect_format(&mut empty, Some("foo.7z")).expect("7z backend");
        assert_eq!(s.name(), "7z");
        let t = detect_format(&mut empty, Some("foo.tar")).expect("tar backend");
        assert_eq!(t.name(), "tar");
        let tg = detect_format(&mut empty, Some("foo.tar.gz")).expect("tar.gz backend");
        assert_eq!(tg.name(), "tar.gz");
        let tx = detect_format(&mut empty, Some("foo.tar.xz")).expect("tar.xz backend");
        assert_eq!(tx.name(), "tar.xz");
    }

    #[test]
    fn detect_format_compound_extension_is_case_insensitive() {
        let mut empty: &[u8] = &[];
        let upper = detect_format(&mut empty, Some("FOO.TAR.GZ")).expect("upper tar.gz");
        assert_eq!(upper.name(), "tar.gz");
        let mixed = detect_format(&mut empty, Some("archive.Tar.Xz")).expect("mixed tar.xz");
        assert_eq!(mixed.name(), "tar.xz");
    }

    #[test]
    fn detect_format_unknown_extension_returns_none() {
        let mut empty: &[u8] = &[];
        assert!(detect_format(&mut empty, Some("foo.rar")).is_none());
    }

    #[test]
    fn detect_format_no_extension_returns_none() {
        let mut empty: &[u8] = &[];
        assert!(detect_format(&mut empty, Some("plain_file")).is_none());
    }

    #[test]
    fn supported_extensions_is_non_empty() {
        assert!(!supported_extensions().is_empty());
    }

    #[test]
    fn safe_join_accepts_normal_relative_path() {
        let base = Path::new("/tmp/work");
        let p = safe_join(base, "sub/file.txt").expect("ok");
        assert_eq!(p, Path::new("/tmp/work/sub/file.txt").to_path_buf());
    }

    #[test]
    fn safe_join_rejects_dotdot() {
        let base = Path::new("/tmp/work");
        assert!(safe_join(base, "../escape").is_err());
        assert!(safe_join(base, "sub/../../etc/passwd").is_err());
    }

    #[test]
    fn safe_join_rejects_empty() {
        let base = Path::new("/tmp/work");
        assert!(safe_join(base, "").is_err());
    }

    #[test]
    fn safe_join_error_message_mentions_traversal() {
        let base = Path::new("/tmp/work");
        let err = safe_join(base, "../escape").expect_err("must fail");
        let msg = err.to_string();
        assert!(
            msg.contains("path traversal") || msg.contains("unsafe path"),
            "message should mention traversal: {msg}"
        );
    }
}
