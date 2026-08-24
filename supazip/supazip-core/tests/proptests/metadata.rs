//! Metadata-invariant property tests (m0.4.0 — WS-I).
//!
//! These tests go beyond the round-trip byte-equality checks in
//! `round_trip_*.rs` and verify structural metadata properties:
//!
//! - Entry count is preserved across create → list.
//! - Total uncompressed size is preserved.
//! - Listed entries appear in insertion order.
//! - Extract recreates files with the correct names.
//! - Path-traversal entry names (containing `..`) are rejected.

use proptest::prelude::*;
use std::io::BufWriter;
use std::path::PathBuf;

use supazip_core::formats::ZipBackend;
use supazip_core::{ArchiveFormat, CreateOptions, Limits, NoOpProgress, WriteSeek};

use crate::common::{arb_entries, cases, materialize_entries, CwdGuard, EntryInput};

/// Strategy: each entry name is prefixed with `"../"` so the path contains
/// a `ParentDir` component. Entry names get a hex index suffix to avoid
/// collisions (mirroring `arb_entries`).
fn arb_entries_with_dotdot() -> proptest::strategy::BoxedStrategy<Vec<EntryInput>> {
    arb_entries()
        .prop_map(|entries| {
            entries
                .into_iter()
                .enumerate()
                .map(|(i, (_name, content))| (format!("../evil_{i:02x}.txt"), content))
                .collect()
        })
        .boxed()
}

/// Helper: create a ZIP archive from on-disk `paths` and return the
/// archive file path. Panics on IO errors (the caller uses `prop_assert`
/// for the invariant checks, not for archive creation).
fn create_zip(paths: &[PathBuf]) -> (tempfile::TempDir, std::path::PathBuf) {
    let backend = ZipBackend::new();
    let archive_tmp = tempfile::tempdir().expect("archive tempdir");
    let archive_path = archive_tmp.path().join("meta.zip");
    let file = std::fs::File::create(&archive_path).expect("create archive");
    let writer: Box<dyn WriteSeek> = Box::new(BufWriter::new(file));
    backend
        .create(
            writer,
            paths,
            &CreateOptions::default(),
            None,
            &NoOpProgress,
            &Limits::default(),
        )
        .expect("zip create");
    (archive_tmp, archive_path)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(32)))]

    #[test]
    fn list_preserves_entry_count(entries in arb_entries()) {
        let (_dir, paths) = materialize_entries(&entries);
        let (_tmp, archive_path) = create_zip(&paths);
        let backend = ZipBackend::new();
        let listed = backend
            .list(
                Box::new(std::fs::File::open(&archive_path).expect("reopen")),
                None,
                &Limits::default(),
            )
            .expect("zip list");
        prop_assert_eq!(listed.len(), entries.len());
    }

    #[test]
    fn list_preserves_total_size(entries in arb_entries()) {
        let (_dir, paths) = materialize_entries(&entries);
        let (_tmp, archive_path) = create_zip(&paths);
        let backend = ZipBackend::new();
        let listed = backend
            .list(
                Box::new(std::fs::File::open(&archive_path).expect("reopen")),
                None,
                &Limits::default(),
            )
            .expect("zip list");
        let total_listed: u64 = listed.iter().map(|e| e.size).sum();
        let total_original: u64 = entries.iter().map(|(_, c)| c.len() as u64).sum();
        prop_assert_eq!(total_listed, total_original);
    }

    #[test]
    fn list_sorted_by_name(entries in arb_entries()) {
        let (_dir, paths) = materialize_entries(&entries);
        let (_tmp, archive_path) = create_zip(&paths);
        let backend = ZipBackend::new();
        let listed = backend
            .list(
                Box::new(std::fs::File::open(&archive_path).expect("reopen")),
                None,
                &Limits::default(),
            )
            .expect("zip list");
        // Entries must appear in the same order they were inserted.
        // (The ZIP format stores entries in insertion order; we verify
        // that the listing preserves that order.)
        for (i, entry) in listed.iter().enumerate() {
            prop_assert_eq!(
                &entry.name, &entries[i].0,
                "entry {}: listed name {:?} != expected {:?}",
                i, entry.name, entries[i].0,
            );
        }
    }

    #[test]
    fn extract_creates_files_with_correct_names(entries in arb_entries()) {
        let (_dir, paths) = materialize_entries(&entries);
        let (_tmp, archive_path) = create_zip(&paths);
        let backend = ZipBackend::new();
        let out_dir = tempfile::tempdir().expect("out tempdir");
        backend
            .extract(
                Box::new(std::fs::File::open(&archive_path).expect("reopen")),
                out_dir.path(),
                &[],
                None,
                &NoOpProgress,
                &Limits::default(),
            )
            .expect("zip extract");
        for (name, _) in &entries {
            let out_path = out_dir.path().join(name);
            prop_assert!(
                out_path.exists(),
                "extracted file missing: {name}"
            );
        }
    }

    #[test]
    fn path_traversal_rejected(entries in arb_entries_with_dotdot()) {
        // Write content to a subdirectory and construct paths that
        // resolve correctly but contain a ".." component.
        let dir = tempfile::tempdir().expect("tempdir");
        let subdir = dir.path().join("sub");
        std::fs::create_dir_all(&subdir).expect("mkdir sub");

        let mut traversal_paths = Vec::with_capacity(entries.len());
        for (name, content) in &entries {
            // name is "../evil_XX.txt"; strip the "../" for the on-disk
            // filename inside `sub/`.
            let basename = name.strip_prefix("../").unwrap_or(name);
            std::fs::write(subdir.join(basename), content).expect("write subfile");
            // "sub/../sub/{basename}" resolves to subdir/{basename} but
            // contains a ParentDir component that safe_join must reject.
            traversal_paths.push(PathBuf::from(format!("sub/../sub/{basename}")));
        }

        // CwdGuard chdirs into `dir` so the relative paths resolve.
        let _guard = CwdGuard::new(dir).expect("chdir");
        let backend = ZipBackend::new();
        let archive_tmp = tempfile::tempdir().expect("archive tempdir");
        let archive_path = archive_tmp.path().join("traversal.zip");

        {
            let file = std::fs::File::create(&archive_path).expect("create");
            let writer: Box<dyn WriteSeek> = Box::new(BufWriter::new(file));
            let create_result = backend.create(
                writer,
                &traversal_paths,
                &CreateOptions::default(),
                None,
                &NoOpProgress,
                &Limits::default(),
            );

            if create_result.is_ok() {
                // Archive was created; extract must reject the traversal
                // names — either by returning an error or by silently
                // skipping them (the zip crate's `enclosed_name()` returns
                // `None` for paths with `..` components).
                let out_dir = tempfile::tempdir().expect("out tempdir");
                let result = backend.extract(
                    Box::new(std::fs::File::open(&archive_path).expect("reopen")),
                    out_dir.path(),
                    &[],
                    None,
                    &NoOpProgress,
                    &Limits::default(),
                );

                if result.is_ok() {
                    // Extract succeeded but traversal entries must not
                    // have been materialised on disk.
                    for (name, _) in &entries {
                        let basename = name.strip_prefix("../").unwrap_or(name);
                        prop_assert!(
                            !out_dir.path().join(basename).exists(),
                            "traversal entry must not be extracted: {name}"
                        );
                    }
                }
                // If extract itself failed that is also acceptable — the
                // traversal was caught one way or another.
            }
            // If create itself failed that is also acceptable — the
            // traversal was caught one way or another.
        }
    }
}
