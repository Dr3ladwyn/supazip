pub mod error;
pub mod formats;
pub mod traits;

pub use error::ArchiverError;
pub use formats::{
    detect_format, get_backend, supported_extensions, SevenZBackend, ZipBackend, BACKENDS,
};
pub use traits::{
    ArchiveEntry, ArchiveFormat, ChannelProgress, CreateOptions, NoOpProgress, ProgressCallback,
    ProgressState, ProgressUpdate, WriteSeek,
};
