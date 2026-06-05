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
