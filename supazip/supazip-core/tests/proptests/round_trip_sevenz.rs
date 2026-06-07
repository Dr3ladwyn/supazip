//! Property-based round-trip test for the 7z backend.
//!
//! Invariant: for any list of `(name, bytes)` pairs, the 7z backend must
//! `create → list → extract` and the extracted file bodies must match the
//! originals byte-for-byte.

use proptest::prelude::*;

use supazip_core::formats::SevenZBackend;
use supazip_core::{ArchiveFormat, CreateOptions, Limits, NoOpProgress, WriteSeek};

use crate::common::{arb_entries, cases, materialize_entries};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(32)))]
    #[test]
    fn round_trip_sevenz(entries in arb_entries()) {
        let (_src_dir, paths) = materialize_entries(&entries);
        let backend = SevenZBackend::new();

        // The trait hands `create` a `Box<dyn WriteSeek + 'static>`, so
        // the writer cannot borrow from a local `Vec<u8>`; build the
        // archive into a temp file and rewind / re-read it from disk
        // for the `list` and `extract` passes. This matches the unit
        // tests for the 7z backend.
        let archive_tmp = tempfile::tempdir().expect("archive tempdir");
        let archive_path = archive_tmp.path().join("round.7z");

        // The 7z backend's extract path applies a per-entry
        // compression-ratio check (`compressed_size * max_ratio >=
        // extracted_size`). A proptest with up to 8 small entries (≤ 4
        // KiB each, some near-empty) routinely blows that ratio even
        // though nothing is bomb-shaped. We disable the check here by
        // setting `max_compression_ratio = 0`; the bomb guard is
        // exercised by a dedicated unit test in `formats/sevenz.rs`.
        let limits = Limits {
            max_compression_ratio: 0,
            ..Limits::default()
        };

        {
            let file = std::fs::File::create(&archive_path).expect("create archive");
            let writer: Box<dyn WriteSeek> = Box::new(file);
            backend
                .create(
                    writer,
                    &paths,
                    &CreateOptions::default(),
                    None,
                    &NoOpProgress,
                    &limits,
                )
                .expect("sevenz create");
        }

        // Phase 2: list.
        let listed = backend
            .list(
                Box::new(std::fs::File::open(&archive_path).expect("reopen")),
                None,
                &Limits::default(),
            )
            .expect("sevenz list");
        prop_assert_eq!(listed.len(), entries.len());

        // Phase 3: extract and assert the bytes round-tripped.
        let out_dir = tempfile::tempdir().expect("out tempdir");
        backend
            .extract(
                Box::new(std::fs::File::open(&archive_path).expect("reopen")),
                out_dir.path(),
                &[],
                None,
                &NoOpProgress,
                &limits,
            )
            .expect("sevenz extract");

        for (name, expected) in &entries {
            let out_path = out_dir.path().join(name);
            let actual = std::fs::read(&out_path)
                .unwrap_or_else(|e| panic!("read {}: {e}", out_path.display()));
            prop_assert_eq!(&actual, expected);
        }
    }
}
