mod sevenz;
mod zip;

pub use sevenz::SevenZBackend;
pub use zip::ZipBackend;

use crate::traits::ArchiveFormat;
use std::collections::HashMap;
use std::sync::LazyLock;

// Global registry of backends by extension
pub static BACKENDS: LazyLock<HashMap<&'static str, &'static dyn ArchiveFormat>> =
    LazyLock::new(|| {
        let mut map = HashMap::new();
        let sevenz: &'static SevenZBackend = Box::leak(Box::new(SevenZBackend::new()));
        let zip: &'static ZipBackend = Box::leak(Box::new(ZipBackend::new()));

        for ext in sevenz.extensions() {
            map.insert(*ext, sevenz as &dyn ArchiveFormat);
        }
        for ext in zip.extensions() {
            map.insert(*ext, zip as &dyn ArchiveFormat);
        }

        map
    });

/// Detect archive format from filename or magic bytes
pub fn detect_format<R: std::io::Read>(
    _reader: &mut R,
    filename: Option<&str>,
) -> Option<&'static dyn ArchiveFormat> {
    // Check extension first
    if let Some(name) = filename {
        if let Some(ext) = name.rsplit('.').next() {
            if let Some(backend) = BACKENDS.get(ext.to_lowercase().as_str()) {
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
    BACKENDS.get(extension.to_lowercase().as_str()).copied()
}

/// List all supported extensions
pub fn supported_extensions() -> Vec<&'static str> {
    BACKENDS.keys().copied().collect()
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
    fn get_backend_is_case_insensitive() {
        assert!(get_backend("ZIP").is_some());
        assert!(get_backend("Zip").is_some());
        assert!(get_backend("7z").is_some());
        assert!(get_backend("7Z").is_some());
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
}
