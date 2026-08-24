use std::io::{Cursor, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use supazip_core::formats::{SevenZBackend, TarBackend, TarGzBackend, TarXzBackend, ZipBackend};
use supazip_core::traits::{ArchiveFormat, CreateOptions, Limits, NoOpProgress};

#[derive(Clone, Default)]
struct SharedCursor(Arc<Mutex<Cursor<Vec<u8>>>>);

impl Write for SharedCursor {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.lock().unwrap().flush()
    }
}

impl Seek for SharedCursor {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.0.lock().unwrap().seek(pos)
    }
}

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

fn build_archive_bytes(backend: &dyn ArchiveFormat, paths: &[PathBuf]) -> Vec<u8> {
    let buf = SharedCursor::default();
    backend
        .create(
            Box::new(buf.clone()),
            paths,
            &CreateOptions::default(),
            None,
            &NoOpProgress,
            &Limits::default(),
        )
        .expect("create fixture");
    let inner = buf.0.lock().unwrap().get_ref().clone();
    inner
}

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
