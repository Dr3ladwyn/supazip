use thiserror::Error;

/// Convenience alias for the boxed-error type that backends put inside
/// `ArchiverError::InvalidArchive::source`. The bounds (`Send + Sync + 'static`)
/// match what `thiserror` needs to expose it as a chained `std::error::Error`.
pub type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

/// Engine-level error type. Every public API on [`crate::traits::ArchiveFormat`]
/// returns `Result<_, ArchiverError>`; the CLI and GUI print it via
/// `Display` and use the `source()` chain for diagnostics.
#[derive(Error, Debug)]
pub enum ArchiverError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid archive: {message}")]
    InvalidArchive {
        message: String,
        #[source]
        source: Option<BoxedError>,
    },

    #[error("Password required")]
    PasswordRequired,

    #[error("Wrong password")]
    WrongPassword,

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Unsupported format: {message}")]
    UnsupportedFormat {
        message: String,
        #[source]
        source: Option<BoxedError>,
    },

    #[error("Archive exceeds resource limit: {0}")]
    TooLarge(String),
}

impl ArchiverError {
    /// Build an `InvalidArchive` from a free-form message.
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidArchive {
            message: message.into(),
            source: None,
        }
    }

    /// Build an `InvalidArchive` from a message and the original error that
    /// triggered it. The source error is preserved for diagnostic chains.
    pub fn invalid_with_source(message: impl Into<String>, source: impl Into<BoxedError>) -> Self {
        Self::InvalidArchive {
            message: message.into(),
            source: Some(source.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    #[test]
    fn display_io_includes_message() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing file");
        let e: ArchiverError = ArchiverError::Io(io);
        let s = e.to_string();
        assert!(s.contains("IO error"), "display: {s}");
        assert!(s.contains("missing file"), "display: {s}");
    }

    #[test]
    fn from_io_error_preserves_source() {
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "nope");
        let e: ArchiverError = io.into();
        assert!(matches!(e, ArchiverError::Io(_)));
        let s = e.source().expect("io error has source");
        assert!(s.to_string().contains("nope"));
    }

    #[test]
    fn display_invalid_archive_contains_message() {
        let e = ArchiverError::invalid("bad sig");
        assert!(e.to_string().contains("bad sig"));
    }

    #[test]
    fn display_password_required() {
        assert_eq!(
            ArchiverError::PasswordRequired.to_string(),
            "Password required"
        );
    }

    #[test]
    fn display_wrong_password() {
        assert_eq!(ArchiverError::WrongPassword.to_string(), "Wrong password");
    }

    #[test]
    fn display_cancelled() {
        assert_eq!(ArchiverError::Cancelled.to_string(), "Operation cancelled");
    }

    #[test]
    fn display_unsupported_format_contains_extension() {
        let e = ArchiverError::UnsupportedFormat {
            message: "rar".into(),
            source: None,
        };
        assert!(e.to_string().contains("rar"));
    }

    #[test]
    fn invalid_archive_preserves_source() {
        let io = std::io::Error::other("boom");
        let e = ArchiverError::invalid_with_source("bad", io);
        assert!(e.to_string().contains("bad"));
        let s = e.source().expect("has source");
        assert!(s.to_string().contains("boom"));
    }

    #[test]
    fn display_too_large_contains_message() {
        let e = ArchiverError::TooLarge("archive > 4 GiB".into());
        assert!(e.to_string().contains("resource limit"));
        assert!(e.to_string().contains("4 GiB"));
    }

    #[test]
    fn debug_is_implemented() {
        // `thiserror`'s `Debug` derive should be in scope; smoke-test it.
        let e = ArchiverError::Cancelled;
        let _ = format!("{e:?}");
    }
}
