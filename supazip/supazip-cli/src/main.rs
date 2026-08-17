//! SupaZip CLI — list / extract / create / test, backed by `supazip-core`.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod completions;
mod man;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use supazip_core::error::ArchiverError;
use supazip_core::traits::{CreateOptions, Limits, ProgressCallback};
use supazip_core::{formats, ArchiveFormat};

mod output;
mod table;
use output::OutputFormat;

#[derive(Parser, Debug)]
#[command(
    name = "supazip",
    about = "A PeaZip-style archive manager for 7z and ZIP (CLI front-end).",
    long_about = "SupaZip CLI — list, extract, create, and test 7z and ZIP archives. \
                  See https://github.com/your-org/supazip for the GUI front-end and core engine.",
    version
)]
pub(crate) struct Cli {
    #[arg(long, global = true, default_value = "text", value_enum)]
    output: OutputFormat,

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

        /// Compression method (`store`, `deflate`, `bzip2`, `zstd` for ZIP;
        /// ignored by 7z, which uses LZMA2 / AES-256 depending on password).
        #[arg(long, default_value = "deflate")]
        compression: String,
    },

    /// Verify the integrity of an archive.
    Test {
        /// Path to the archive file.
        archive: PathBuf,

        /// Password for encrypted archives.
        #[arg(long)]
        password: Option<String>,
    },

    /// Generate shell completion scripts (bash, zsh, fish, powershell, elvish).
    Completions {
        /// Target shell.
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Generate man page.
    Man {
        #[arg(long, default_value = ".")]
        out_dir: std::path::PathBuf,
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

    // Best-effort Ctrl-C handling: when the user hits Ctrl-C we want to
    // surface `ArchiverError::Cancelled` from in-flight operations instead
    // of letting the process die mid-write (which can leave a half-written
    // archive on disk). The check here is a soft guard for short-running
    // commands; long operations in the core honour `progress.is_cancelled`.
    let cli = Cli::parse();
    let result = run(cli);
    if let Err(ArchiverError::Cancelled) = &result {
        eprintln!("interrupted");
        return ExitCode::from(130);
    }
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), ArchiverError> {
    let limits = effective_limits();
    let fmt = cli.output;
    match cli.command {
        Command::List { archive, password } => {
            let backend = resolve_backend(&archive, None)?;
            cmd_list(backend, &archive, password.as_deref(), &limits, fmt)
        }
        Command::Extract {
            archive,
            out,
            password,
            all: _,
            entries,
        } => {
            let backend = resolve_backend(&archive, None)?;
            cmd_extract(
                backend,
                &archive,
                &out,
                password.as_deref(),
                &entries,
                &limits,
            )
        }
        Command::Create {
            archive,
            files,
            format,
            password,
            compression,
        } => {
            let backend = resolve_backend(&archive, format.map(format_to_ext))?;
            cmd_create(
                backend,
                &archive,
                &files,
                password.as_deref(),
                &compression,
                &limits,
            )
        }
        Command::Test { archive, password } => {
            let backend = resolve_backend(&archive, None)?;
            cmd_test(backend, &archive, password.as_deref(), &limits, fmt)
        }
        Command::Completions { shell } => {
            let mut cmd = Cli::command();
            completions::generate_completions(shell, &mut cmd);
            Ok(())
        }
        Command::Man { out_dir } => {
            man::generate_man_pages(&out_dir).map_err(|e| ArchiverError::invalid(format!("{e}")))
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
        return Err(ArchiverError::UnsupportedFormat {
            message: ext.to_string(),
            source: None,
        });
    }

    let ext = archive
        .extension()
        .and_then(|s| s.to_str())
        .ok_or_else(|| ArchiverError::UnsupportedFormat {
            message: format!(
                "cannot detect format from '{}' (no extension); pass --format",
                archive.display()
            ),
            source: None,
        })?;

    formats::get_backend(ext).ok_or_else(|| ArchiverError::UnsupportedFormat {
        message: ext.to_string(),
        source: None,
    })
}

// -----------------------------------------------------------------------------
// Per-command implementations
// -----------------------------------------------------------------------------

fn cmd_list(
    backend: &dyn ArchiveFormat,
    archive: &Path,
    password: Option<&str>,
    limits: &Limits,
    fmt: OutputFormat,
) -> Result<(), ArchiverError> {
    tracing::info!(archive = %archive.display(), backend = backend.name(), "list");
    let file = File::open(archive)?;
    let entries = backend.list(Box::new(buf_reader(file)), password, limits)?;

    if fmt != OutputFormat::Text {
        output::print(fmt, &entries)
            .map_err(|e| ArchiverError::invalid(format!("output serialization failed: {e}")))?;
        return Ok(());
    }

    // Text table: column widths / alignment / rule come from
    // `design/cli-table.json` (mirror of `design/cli-table.tera`).
    // Colour only when stdout is a TTY; json|yaml stay uncoloured above.
    print!(
        "{}",
        table::render_list_table(&entries, table::tty_styles().as_ref())
    );
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
    let file = File::open(archive)?;
    let entry_refs: Vec<&str> = entries.iter().map(String::as_str).collect();
    backend.extract(
        Box::new(buf_reader(file)),
        out,
        &entry_refs,
        password,
        &StderrProgress::new(),
        limits,
    )?;
    println!("extracted to {}", out.display());
    Ok(())
}

fn cmd_create(
    backend: &dyn ArchiveFormat,
    archive: &Path,
    files: &[PathBuf],
    password: Option<&str>,
    compression: &str,
    limits: &Limits,
) -> Result<(), ArchiverError> {
    tracing::info!(archive = %archive.display(), backend = backend.name(), entries = files.len(), "create");
    // Atomic create: write to a sibling tempfile in the same directory,
    // fsync-equivalent (BufWriter on drop), then rename into place. This
    // ensures a partially-written archive can never replace a previous good
    // copy at the target path.
    let parent = archive.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(ArchiverError::Io)?;
    // Atomic create: drive the backend into a `Vec<u8>`-backed sink, then
    // write the resulting bytes through a `NamedTempFile` in the same
    // directory and `persist` (rename) over the target. The trait method
    // consumes a `Box<dyn WriteSeek + 'static>`, so the sink must own its
    // state; we box a `SharedVecSink` (which holds an `Arc<Mutex<Cursor>>`)
    // and recover the bytes via a side channel.
    let parent = archive.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(ArchiverError::Io)?;
    let options = CreateOptions {
        compression_method: compression.to_string(),
        compression_level: None,
        compression: supazip_core::CompressionMethod::from_legacy_str(compression),
    };
    let shared = std::sync::Arc::new(std::sync::Mutex::new(
        std::io::Cursor::new(Vec::<u8>::new()),
    ));
    {
        let writer: Box<dyn supazip_core::traits::WriteSeek> =
            Box::new(SharedVecSink::new(shared.clone()));
        backend.create(
            writer,
            files,
            &options,
            password,
            &StderrProgress::new(),
            limits,
        )?;
    }
    let bytes = std::sync::Arc::try_unwrap(shared)
        .map_err(|_| ArchiverError::invalid("atomic create: leaked SharedVecSink clones"))?
        .into_inner()
        .map_err(|_| ArchiverError::invalid("atomic create: poisoned SharedVecSink"))?
        .into_inner();
    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(ArchiverError::Io)?;
    std::io::Write::write_all(tmp.as_file_mut(), &bytes).map_err(ArchiverError::Io)?;
    tmp.as_file().sync_all().map_err(ArchiverError::Io)?;
    tmp.persist(archive).map_err(|e| {
        ArchiverError::invalid_with_source("atomic create: persist failed", e.error)
    })?;
    println!("created {} ({} entries)", archive.display(), files.len());
    Ok(())
}

fn cmd_test(
    backend: &dyn ArchiveFormat,
    archive: &Path,
    password: Option<&str>,
    limits: &Limits,
    fmt: OutputFormat,
) -> Result<(), ArchiverError> {
    tracing::info!(archive = %archive.display(), backend = backend.name(), "test");
    let file = File::open(archive)?;
    let ok = backend.test(
        Box::new(buf_reader(file)),
        password,
        &StderrProgress::new(),
        limits,
    )?;

    if fmt != OutputFormat::Text {
        #[derive(serde::Serialize)]
        struct TestResult {
            ok: bool,
        }
        output::print(fmt, &TestResult { ok })
            .map_err(|e| ArchiverError::invalid(format!("output serialization failed: {e}")))?;
        if !ok {
            return Err(ArchiverError::invalid(format!(
                "integrity check failed for {}",
                archive.display()
            )));
        }
        return Ok(());
    }

    if ok {
        println!("OK: {}", archive.display());
        Ok(())
    } else {
        Err(ArchiverError::invalid(format!(
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
fn buf_reader<R: Read>(r: R) -> std::io::BufReader<R> {
    std::io::BufReader::with_capacity(64 * 1024, r)
}

/// `Write + Seek` implementation that forwards every operation to a shared
/// `Cursor<Vec<u8>>`. Used by `cmd_create` to capture the bytes the backend
/// produced so we can write them atomically through a temp file. The 7z
/// writer seeks back to patch the header after streaming the payload, so the
/// sink must honour `Seek` properly; that is why the inner type is
/// `Cursor<Vec<u8>>` and not just a `Vec`.
struct SharedVecSink {
    shared: std::sync::Arc<std::sync::Mutex<std::io::Cursor<Vec<u8>>>>,
}

impl SharedVecSink {
    fn new(shared: std::sync::Arc<std::sync::Mutex<std::io::Cursor<Vec<u8>>>>) -> Self {
        Self { shared }
    }
}

impl std::io::Write for SharedVecSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut c = self
            .shared
            .lock()
            .map_err(|e| std::io::Error::other(format!("mutex poisoned: {e}")))?;
        c.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl std::io::Seek for SharedVecSink {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let mut c = self
            .shared
            .lock()
            .map_err(|e| std::io::Error::other(format!("mutex poisoned: {e}")))?;
        c.seek(pos)
    }
}

/// Progress sink that prints "name: current/total" on stderr and exposes a
/// cancellation flag. CLI callers (the GUI uses `ChannelProgress`/`ProgressState`)
/// do not flip the flag today, so the underlying atomic stays `false` and
/// `is_cancelled` never returns `true`; a future revision can wire a
/// `tokio::signal::ctrl_c` listener to the flag.
struct StderrProgress {
    cancelled: Arc<AtomicBool>,
}

impl StderrProgress {
    fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl ProgressCallback for StderrProgress {
    fn set_progress(&self, _current: u64, _total: u64) {
        // intentionally quiet — high-frequency updates would spam the terminal
    }
    fn set_message(&self, message: &str) {
        eprintln!("  {message}");
    }
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// Best-effort tracing-subscriber init. Honours `RUST_LOG` (or
/// `SUPAZIP_LOG`) via `tracing_subscriber::EnvFilter` so the user can
/// crank verbosity without recompiling. A subscriber is installed at
/// most once per process, even when called from tests.
fn tracing_subscriber_init() -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    let mut ok = false;
    ONCE.call_once(|| {
        let env_filter = tracing_subscriber::EnvFilter::try_from_env("SUPAZIP_LOG")
            .or_else(|_| tracing_subscriber::EnvFilter::try_from_default_env())
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
        let subscriber = tracing_subscriber::fmt()
            .with_env_filter(env_filter)
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
