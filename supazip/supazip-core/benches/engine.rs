//! Criterion benchmarks for the SupaZip engine.
//!
//! WS-H (m0.3.0): exercise the five backends (`zip`, `7z`, `tar`, `tar.gz`,
//! `tar.xz`) over the three hot operations (`list`, `extract`, `create`)
//! plus a `test` smoke pass per backend. Sizes: 10 / 100 / 1 000 entries
//! at 256 B per entry for `list` / `extract` / `test`, and a single
//! 100-entry × 1 KiB fixture for `create`. The fixture is built once per
//! group and re-used; the `b.iter` closure is the only thing that runs
//! inside the timed loop.
//!
//! Run a baseline:
//!
//! ```text
//! cargo bench -p supazip-core --bench engine -- --save-baseline m0.3.0
//! ```
//!
//! Compare against it:
//!
//! ```text
//! cargo bench -p supazip-core --bench engine -- --baseline m0.3.0
//! ```
//!
//! See `docs/benchmarks.md` for the full workflow and the CI job
//! definition.

use std::io::Cursor;
use std::path::PathBuf;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use supazip_core::{
    formats::{SevenZBackend, TarBackend, TarGzBackend, TarXzBackend, ZipBackend},
    ArchiveFormat, CreateOptions, Limits, NoOpProgress,
};

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

/// Build a scratch directory populated with `n_entries` files of
/// `entry_size` bytes each. Returns the owning `TempDir` (so it stays
/// alive for the duration of the bench) plus the file paths in stable
/// order.
fn make_test_dir(n_entries: usize, entry_size: usize) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut paths = Vec::with_capacity(n_entries);
    let payload = vec![0u8; entry_size];
    for i in 0..n_entries {
        let p = dir.path().join(format!("file_{i:04}.bin"));
        std::fs::write(&p, &payload).expect("write fixture");
        paths.push(p);
    }
    (dir, paths)
}

/// Build an in-memory archive using `backend` and `paths`, returning the
/// raw bytes. Used by the list / extract / test benches which need a
/// reader.
#[derive(Clone)]
struct SharedWriter(std::sync::Arc<std::sync::Mutex<Cursor<Vec<u8>>>>);
impl std::io::Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}
impl std::io::Seek for SharedWriter {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.0.lock().unwrap().seek(pos)
    }
}

fn build_archive_bytes(backend: &dyn ArchiveFormat, paths: &[PathBuf]) -> Vec<u8> {
    let cur = std::sync::Arc::new(std::sync::Mutex::new(Cursor::new(Vec::<u8>::new())));
    backend
        .create(
            Box::new(SharedWriter(cur.clone())),
            paths,
            &CreateOptions::default(),
            None,
            &NoOpProgress,
            &Limits::default(),
        )
        .expect("create fixture");
    std::sync::Arc::try_unwrap(cur)
        .unwrap()
        .into_inner()
        .unwrap()
        .into_inner()
}

// ---------------------------------------------------------------------------
// create — one bench per backend at a single 100 × 1 KiB fixture
// ---------------------------------------------------------------------------

fn bench_create(c: &mut Criterion) {
    let mut group = c.benchmark_group("create");
    // 100 entries × 1 KiB hits the sweet spot: < 1 s per backend,
    // exercises the per-entry plumbing, the compression loop, and the
    // central directory / folder write.
    let (_d, paths) = make_test_dir(100, 1024);
    let n = paths.len();
    group.throughput(Throughput::Elements(n as u64));

    group.bench_function(BenchmarkId::new("zip", n), |b| {
        b.iter(|| {
            let backend = ZipBackend::new();
            let cur = Cursor::new(Vec::<u8>::new());
            backend
                .create(
                    Box::new(cur),
                    &paths,
                    &CreateOptions::default(),
                    None,
                    &NoOpProgress,
                    &Limits::default(),
                )
                .unwrap();
        });
    });

    group.bench_function(BenchmarkId::new("7z", n), |b| {
        b.iter(|| {
            let backend = SevenZBackend::new();
            let cur = Cursor::new(Vec::<u8>::new());
            backend
                .create(
                    Box::new(cur),
                    &paths,
                    &CreateOptions::default(),
                    None,
                    &NoOpProgress,
                    &Limits::default(),
                )
                .unwrap();
        });
    });

    group.bench_function(BenchmarkId::new("tar", n), |b| {
        b.iter(|| {
            let backend = TarBackend::new();
            let cur = Cursor::new(Vec::<u8>::new());
            backend
                .create(
                    Box::new(cur),
                    &paths,
                    &CreateOptions::default(),
                    None,
                    &NoOpProgress,
                    &Limits::default(),
                )
                .unwrap();
        });
    });

    group.bench_function(BenchmarkId::new("tar.gz", n), |b| {
        b.iter(|| {
            let backend = TarGzBackend::new();
            let cur = Cursor::new(Vec::<u8>::new());
            backend
                .create(
                    Box::new(cur),
                    &paths,
                    &CreateOptions::default(),
                    None,
                    &NoOpProgress,
                    &Limits::default(),
                )
                .unwrap();
        });
    });

    group.bench_function(BenchmarkId::new("tar.xz", n), |b| {
        b.iter(|| {
            let backend = TarXzBackend::new();
            let cur = Cursor::new(Vec::<u8>::new());
            backend
                .create(
                    Box::new(cur),
                    &paths,
                    &CreateOptions::default(),
                    None,
                    &NoOpProgress,
                    &Limits::default(),
                )
                .unwrap();
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// list — one bench per backend, parameterised on entry count
// ---------------------------------------------------------------------------

fn bench_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("list");
    for &n in &[10usize, 100, 1000] {
        let (_d, paths) = make_test_dir(n, 256);
        let bytes_zip = build_archive_bytes(&ZipBackend::new(), &paths);
        let bytes_7z = build_archive_bytes(&SevenZBackend::new(), &paths);
        let bytes_tar = build_archive_bytes(&TarBackend::new(), &paths);
        let bytes_targz = build_archive_bytes(&TarGzBackend::new(), &paths);
        let bytes_tarxz = build_archive_bytes(&TarXzBackend::new(), &paths);

        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::new("zip", n), &bytes_zip, |b, bytes| {
            b.iter(|| {
                let cur = Cursor::new(bytes.clone());
                ZipBackend::new()
                    .list(Box::new(cur), None, &Limits::default())
                    .unwrap();
            });
        });

        group.bench_with_input(BenchmarkId::new("7z", n), &bytes_7z, |b, bytes| {
            b.iter(|| {
                let cur = Cursor::new(bytes.clone());
                SevenZBackend::new()
                    .list(Box::new(cur), None, &Limits::default())
                    .unwrap();
            });
        });

        group.bench_with_input(BenchmarkId::new("tar", n), &bytes_tar, |b, bytes| {
            b.iter(|| {
                let cur = Cursor::new(bytes.clone());
                TarBackend::new()
                    .list(Box::new(cur), None, &Limits::default())
                    .unwrap();
            });
        });

        group.bench_with_input(BenchmarkId::new("tar.gz", n), &bytes_targz, |b, bytes| {
            b.iter(|| {
                let cur = Cursor::new(bytes.clone());
                TarGzBackend::new()
                    .list(Box::new(cur), None, &Limits::default())
                    .unwrap();
            });
        });

        group.bench_with_input(BenchmarkId::new("tar.xz", n), &bytes_tarxz, |b, bytes| {
            b.iter(|| {
                let cur = Cursor::new(bytes.clone());
                TarXzBackend::new()
                    .list(Box::new(cur), None, &Limits::default())
                    .unwrap();
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// extract — one bench per backend, parameterised on entry count
// ---------------------------------------------------------------------------

fn bench_extract(c: &mut Criterion) {
    let mut group = c.benchmark_group("extract");
    for &n in &[10usize, 100, 1000] {
        let (_d, paths) = make_test_dir(n, 256);
        let bytes_zip = build_archive_bytes(&ZipBackend::new(), &paths);
        let bytes_7z = build_archive_bytes(&SevenZBackend::new(), &paths);
        let bytes_tar = build_archive_bytes(&TarBackend::new(), &paths);
        let bytes_targz = build_archive_bytes(&TarGzBackend::new(), &paths);
        let bytes_tarxz = build_archive_bytes(&TarXzBackend::new(), &paths);

        group.throughput(Throughput::Elements(n as u64));

        // Each iteration extracts into a fresh temp dir so leftover
        // entries from a previous iteration cannot shadow the next one.
        group.bench_with_input(BenchmarkId::new("zip", n), &bytes_zip, |b, bytes| {
            b.iter_with_setup(
                || tempfile::tempdir().expect("tempdir"),
                |dest| {
                    let cur = Cursor::new(bytes.clone());
                    ZipBackend::new()
                        .extract(
                            Box::new(cur),
                            dest.path(),
                            &[],
                            None,
                            &NoOpProgress,
                            &Limits::default(),
                        )
                        .unwrap();
                },
            );
        });

        group.bench_with_input(BenchmarkId::new("7z", n), &bytes_7z, |b, bytes| {
            b.iter_with_setup(
                || tempfile::tempdir().expect("tempdir"),
                |dest| {
                    let cur = Cursor::new(bytes.clone());
                    SevenZBackend::new()
                        .extract(
                            Box::new(cur),
                            dest.path(),
                            &[],
                            None,
                            &NoOpProgress,
                            &Limits::default(),
                        )
                        .unwrap();
                },
            );
        });

        group.bench_with_input(BenchmarkId::new("tar", n), &bytes_tar, |b, bytes| {
            b.iter_with_setup(
                || tempfile::tempdir().expect("tempdir"),
                |dest| {
                    let cur = Cursor::new(bytes.clone());
                    TarBackend::new()
                        .extract(
                            Box::new(cur),
                            dest.path(),
                            &[],
                            None,
                            &NoOpProgress,
                            &Limits::default(),
                        )
                        .unwrap();
                },
            );
        });

        group.bench_with_input(BenchmarkId::new("tar.gz", n), &bytes_targz, |b, bytes| {
            b.iter_with_setup(
                || tempfile::tempdir().expect("tempdir"),
                |dest| {
                    let cur = Cursor::new(bytes.clone());
                    TarGzBackend::new()
                        .extract(
                            Box::new(cur),
                            dest.path(),
                            &[],
                            None,
                            &NoOpProgress,
                            &Limits::default(),
                        )
                        .unwrap();
                },
            );
        });

        group.bench_with_input(BenchmarkId::new("tar.xz", n), &bytes_tarxz, |b, bytes| {
            b.iter_with_setup(
                || tempfile::tempdir().expect("tempdir"),
                |dest| {
                    let cur = Cursor::new(bytes.clone());
                    TarXzBackend::new()
                        .extract(
                            Box::new(cur),
                            dest.path(),
                            &[],
                            None,
                            &NoOpProgress,
                            &Limits::default(),
                        )
                        .unwrap();
                },
            );
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// test — one bench per backend, parameterised on entry count
// ---------------------------------------------------------------------------

fn bench_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("test");
    for &n in &[10usize, 100, 1000] {
        let (_d, paths) = make_test_dir(n, 256);
        let bytes_zip = build_archive_bytes(&ZipBackend::new(), &paths);
        let bytes_7z = build_archive_bytes(&SevenZBackend::new(), &paths);
        let bytes_tar = build_archive_bytes(&TarBackend::new(), &paths);
        let bytes_targz = build_archive_bytes(&TarGzBackend::new(), &paths);
        let bytes_tarxz = build_archive_bytes(&TarXzBackend::new(), &paths);

        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(BenchmarkId::new("zip", n), &bytes_zip, |b, bytes| {
            b.iter(|| {
                let cur = Cursor::new(bytes.clone());
                ZipBackend::new()
                    .test(Box::new(cur), None, &NoOpProgress, &Limits::default())
                    .unwrap();
            });
        });

        group.bench_with_input(BenchmarkId::new("7z", n), &bytes_7z, |b, bytes| {
            b.iter(|| {
                let cur = Cursor::new(bytes.clone());
                SevenZBackend::new()
                    .test(Box::new(cur), None, &NoOpProgress, &Limits::default())
                    .unwrap();
            });
        });

        group.bench_with_input(BenchmarkId::new("tar", n), &bytes_tar, |b, bytes| {
            b.iter(|| {
                let cur = Cursor::new(bytes.clone());
                TarBackend::new()
                    .test(Box::new(cur), None, &NoOpProgress, &Limits::default())
                    .unwrap();
            });
        });

        group.bench_with_input(BenchmarkId::new("tar.gz", n), &bytes_targz, |b, bytes| {
            b.iter(|| {
                let cur = Cursor::new(bytes.clone());
                TarGzBackend::new()
                    .test(Box::new(cur), None, &NoOpProgress, &Limits::default())
                    .unwrap();
            });
        });

        group.bench_with_input(BenchmarkId::new("tar.xz", n), &bytes_tarxz, |b, bytes| {
            b.iter(|| {
                let cur = Cursor::new(bytes.clone());
                TarXzBackend::new()
                    .test(Box::new(cur), None, &NoOpProgress, &Limits::default())
                    .unwrap();
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_create, bench_list, bench_extract, bench_test);
criterion_main!(benches);
