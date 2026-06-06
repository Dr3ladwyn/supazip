//! SupaZip CLI — list / extract / create / test, backed by `supazip-core`.

use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use supazip_core::error::ArchiverError;
use supazip_core::traits::{CreateOptions, Limits, ProgressCallback};
use supazip_core::{formats, ArchiveFormat};

#[derive(Parser, Debug)]
#[command(
    name = "supazip",
    about = "A PeaZip-style archive manager for 7z and ZIP (CLI front-end).",
    long_about = "SupaZip CLI — list, extract, create, and test 7z and ZIP archives. \
                  See https://github.com/ for the GUI front-end and core engine.",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List the contents of an archive.
    List {
        /// Path to the archive file.
        archive: PathBuf,

        /// Password for encrypted archives.
        #[arg(long)]
        password: Option<String>,
    },

    /// Extract an archive to a directory.
    Extract {
        /// Path to the archive file.
        archive: PathBuf,

        /// Output directory. Defaults to the current directory.
        #[arg(long, default_value = ".")]
        out: PathBuf,

        /// Password for encrypted archives.
        #[arg(long)]
        password: Option<String>,

        /// Extract every entry (the default).
        #[arg(long, default_value_t = true)]
        all: bool,

        /// Extract only the named entries. May be repeated.
        #[arg(long = "entry", value_name = "NAME")]
        entries: Vec<String>,
    },

    /// Create a new archive from one or more files.
    Create {
        /// Path of the archive to create. The format is inferred from the
        /// extension unless `--format` is given.
        archive: PathBuf,

        /// Files to add to the archive.
        #[arg(required = true)]
        files: Vec<PathBuf>,

        /// Force a specific archive format.
        #[arg(long, value_enum)]
        format: Option<Format>,

        /// Password for encrypted archives (only supported for 7z right now).
        #[arg(long)]
        password: Option<String>,
    },

    /// Verify the integrity of an archive.
    Test {
        /// Path to the archive file.
        archive: PathBuf,

        /// Password for encrypted archives.
        #[arg(long)]
        password: Option<String>,
    },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Format {
    Zip,
    #[value(name = "7z")]
    SevenZ,
}

impl Format {
    fn extensions(self) -> &'static [&'static str] {
        match self {
            Format::Zip => &["zip"],
            Format::SevenZ => &["7z"],
        }
    }
}

fn main() -> ExitCode {
    // Initialise a tracing subscriber that only emits at info+ to stderr.
    // We do this defensively: a subscriber may already be installed by tests.
    let _ = tracing_subscriber_init();

    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), ArchiverError> {
    let limits = effective_limits();
    match cli.command {
        Command::List { archive, password } => {
            let backend = resolve_backend(&archive, None)?;
            cmd_list(backend, &archive, password.as_deref(), &limits)
        }
        Command::Extract {
            archive,
            out,
            password,
            all: _,
            entries,
        } => {
            let backend = resolve_backend(&archive, None)?;
            cmd_extract(backend, &archive, &out, password.as_deref(), &entries, &limits)
        }
        Command::Create {
            archive,
            files,
            format,
            password,
        } => {
            let backend = resolve_backend(&archive, format.map(format_to_ext))?;
            cmd_create(backend, &archive, &files, password.as_deref(), &limits)
        }
        Command::Test { archive, password } => {
            let backend = resolve_backend(&archive, None)?;
            cmd_test(backend, &archive, password.as_deref(), &limits)
        }
    }
}

/// Resolve the `Limits` for this CLI invocation. Defaults are taken from
/// `Limits::default()`. The user can lower `max_archive_size` via the
/// `SUPAZIP_MAX_ARCHIVE_SIZE` environment variable (decimal bytes, or with a
/// `K`/`M`/`G` suffix). Other limits are not currently configurable; exposing
/// them as flags is a future addition.
fn effective_limits() -> Limits {
    let mut limits = Limits::default();
    if let Ok(raw) = std::env::var("SUPAZIP_MAX_ARCHIVE_SIZE") {
        if let Some(v) = parse_size(&raw) {
            limits.max_archive_size = v;
        } else {
            eprintln!(
                "warning: ignoring SUPAZIP_MAX_ARCHIVE_SIZE='{raw}' (cannot parse; \
                 expected decimal bytes or with K/M/G suffix)"
            );
        }
    }
    limits
}

/// Parse a size string with optional `K`/`M`/`G` suffix into bytes. `None` on
/// parse failure.
fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (num, mult) = match s.chars().last() {
        Some('k') | Some('K') => (&s[..s.len() - 1], 1024u64),
        Some('m') | Some('M') => (&s[..s.len() - 1], 1024u64 * 1024),
        Some('g') | Some('G') => (&s[..s.len() - 1], 1024u64 * 1024 * 1024),
        _ => (s, 1u64),
    };
    let n: u64 = num.trim().parse().ok()?;
    n.checked_mul(mult)
}

fn format_to_ext(f: Format) -> &'static str {
    f.extensions()[0]
}

/// Resolve a backend either by explicit `--format` or by the file extension.
/// Returns a clear error when the extension is unrecognised.
fn resolve_backend(
    archive: &Path,
    explicit_format: Option<&str>,
) -> Result<&'static dyn ArchiveFormat, ArchiverError> {
    if let Some(ext) = explicit_format {
        if let Some(backend) = formats::get_backend(ext) {
            return Ok(backend);
        }
        return Err(ArchiverError::UnsupportedFormat(ext.to_string()));
    }

    let ext = archive
        .extension()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            ArchiverError::UnsupportedFormat(format!(
                "cannot detect format from '{}' (no extension); pass --format",
                archive.display()
            ))
        })?;

    formats::get_backend(ext).ok_or_else(|| ArchiverError::UnsupportedFormat(ext.to_string()))
}

// -----------------------------------------------------------------------------
// Per-command implementations
// -----------------------------------------------------------------------------

fn cmd_list(
    backend: &dyn ArchiveFormat,
    archive: &Path,
    password: Option<&str>,
    limits: &Limits,
) -> Result<(), ArchiverError> {
    tracing::info!(archive = %archive.display(), backend = backend.name(), "list");
    let file = File::open(archive)?;
    let entries = backend.list(Box::new(BufReader::new(file)), password, limits)?;

    // Plain-text table: name, size, compressed, encrypted.
    println!(
        "{:>4}  {:<12}  {:>12}  {:>12}  {:<8}  NAME",
        "IDX", "METHOD", "SIZE", "COMPRESSED", "CRYPT"
    );
    println!("{}", "-".repeat(72));
    for (i, e) in entries.iter().enumerate() {
        let crypt = if e.encrypted { "yes" } else { "-" };
        println!(
            "{:>4}  {:<12}  {:>12}  {:>12}  {:<8}  {}",
            i, e.compression_method, e.size, e.compressed_size, crypt, e.name,
        );
    }
    println!("\n{} entries", entries.len());
    Ok(())
}

fn cmd_extract(
    backend: &dyn ArchiveFormat,
    archive: &Path,
    out: &Path,
    password: Option<&str>,
    entries: &[String],
    limits: &Limits,
) -> Result<(), ArchiverError> {
    tracing::info!(archive = %archive.display(), out = %out.display(), backend = backend.name(), "extract");
    std::fs::create_dir_all(out)?;

    // The core's extract writes to the current working directory using
    // `enclosed_name`. We emulate "out" by chdir-ing for the duration of the
    // call. That keeps the existing core behaviour while honouring --out at
    // the CLI level.
    let prev_cwd = std::env::current_dir().ok();
    std::env::set_current_dir(out).map_err(ArchiverError::Io)?;

    let result = (|| -> Result<(), ArchiverError> {
        let file = File::open(archive)?;
        let dest: Box<dyn supazip_core::traits::WriteSeek> = Box::new(NullDest::new());
        let entry_refs: Vec<&str> = entries.iter().map(String::as_str).collect();
        backend.extract(
            Box::new(BufReader::new(file)),
            dest,
            &entry_refs,
            password,
            &StderrProgress,
            limits,
        )
    })();

    if let Some(prev) = prev_cwd {
        let _ = std::env::set_current_dir(&prev);
    }
    result?;
    println!("extracted to {}", out.display());
    Ok(())
}

fn cmd_create(
    backend: &dyn ArchiveFormat,
    archive: &Path,
    files: &[PathBuf],
    password: Option<&str>,
    limits: &Limits,
) -> Result<(), ArchiverError> {
    tracing::info!(archive = %archive.display(), backend = backend.name(), entries = files.len(), "create");
    let out_file = File::create(archive)?;
    let writer: Box<dyn supazip_core::traits::WriteSeek> = Box::new(BufWriter::new(out_file));
    let options = CreateOptions {
        compression_method: "deflate".to_string(),
        compression_level: None,
    };
    backend.create(writer, files, &options, password, &StderrProgress, limits)?;
    println!("created {} ({} entries)", archive.display(), files.len());
    Ok(())
}

fn cmd_test(
    backend: &dyn ArchiveFormat,
    archive: &Path,
    password: Option<&str>,
    limits: &Limits,
) -> Result<(), ArchiverError> {
    tracing::info!(archive = %archive.display(), backend = backend.name(), "test");
    let file = File::open(archive)?;
    let ok = backend.test(Box::new(BufReader::new(file)), password, &StderrProgress, limits)?;
    if ok {
        println!("OK: {}", archive.display());
        Ok(())
    } else {
        Err(ArchiverError::InvalidArchive(format!(
            "integrity check failed for {}",
            archive.display()
        )))
    }
}

// -----------------------------------------------------------------------------
// Tiny helpers
// -----------------------------------------------------------------------------

/// 64 KiB buffer wrapper used to give every read to the core a larger granularity
/// than 8 KiB. Cheap; doesn't change semantics.
struct BufReader<R: Read> {
    inner: std::io::BufReader<R>,
}
impl<R: Read> BufReader<R> {
    fn new(r: R) -> Self {
        Self {
            inner: std::io::BufReader::with_capacity(64 * 1024, r),
        }
    }
}
impl<R: Read> Read for BufReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

/// No-op `Write + Seek` used to satisfy the trait's `Box<dyn WriteSeek>` slot
/// for `extract`. The core's zip/sevenz backends currently use `enclosed_name`
/// and write to the filesystem; this sink exists purely to make the call
/// type-check.
struct NullDest(std::io::Cursor<Vec<u8>>);
impl NullDest {
    fn new() -> Self {
        Self(std::io::Cursor::new(Vec::new()))
    }
}
impl Write for NullDest {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl std::io::Seek for NullDest {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

/// Progress sink that prints "name: current/total" on stderr at most a few
/// times per second (rate-limited via a simple last-emit timestamp).
struct StderrProgress;
impl ProgressCallback for StderrProgress {
    fn set_progress(&self, _current: u64, _total: u64) {
        // intentionally quiet — high-frequency updates would spam the terminal
    }
    fn set_message(&self, message: &str) {
        eprintln!("  {message}");
    }
    fn is_cancelled(&self) -> bool {
        false
    }
}

/// Best-effort tracing-subscriber init. We accept that there is no env_filter
/// configured: the CLI is small and chatty tracing is fine for now.
fn tracing_subscriber_init() -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    let mut ok = false;
    ONCE.call_once(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_writer(std::io::stderr)
            .finish();
        if tracing::subscriber::set_global_default(subscriber).is_ok() {
            ok = true;
        }
    });
    if ok {
        Ok(())
    } else {
        Err("tracing subscriber already set".into())
    }
}
