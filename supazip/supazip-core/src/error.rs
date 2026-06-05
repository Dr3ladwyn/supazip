use thiserror::Error;

#[derive(Error, Debug)]
pub enum ArchiverError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid archive: {0}")]
    InvalidArchive(String),

    #[error("Password required")]
    PasswordRequired,

    #[error("Wrong password")]
    WrongPassword,

    #[error("Operation cancelled")]
    Cancelled,

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),
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
        let e = ArchiverError::InvalidArchive("bad sig".into());
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
        let e = ArchiverError::UnsupportedFormat("rar".into());
        assert!(e.to_string().contains("rar"));
    }

    #[test]
    fn debug_is_implemented() {
        // `thiserror`'s `Debug` derive should be in scope; smoke-test it.
        let e = ArchiverError::Cancelled;
        let _ = format!("{e:?}");
    }
}
