//! Property-based round-trip test for the ZIP backend.
//!
//! Invariant: for any list of `(name, bytes)` pairs, the ZIP backend must
//! `create → list → extract` and the extracted file bodies must match the
//! originals byte-for-byte.

use proptest::prelude::*;
use std::io::BufWriter;

use supazip_core::formats::ZipBackend;
use supazip_core::{ArchiveFormat, CreateOptions, Limits, NoOpProgress, WriteSeek};

use crate::common::{arb_entries, cases, materialize_entries};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(32)))]
    #[test]
    fn round_trip_zip(entries in arb_entries()) {
        let (_src_dir, paths) = materialize_entries(&entries);
        let backend = ZipBackend::new();

        // Phase 1: create. The trait hands `create` a `Box<dyn WriteSeek>`,
        // which defaults to `Box<dyn WriteSeek + 'static>`, so the cursor
        // cannot borrow from a local `Vec<u8>`. The escape hatch is to
        // build the archive into a temp file and rewind / re-read it from
        // disk for the `list` and `extract` passes. This is the same
        // pattern the existing unit tests use.
        let archive_tmp = tempfile::tempdir().expect("archive tempdir");
        let archive_path = archive_tmp.path().join("round.zip");
        {
            let file = std::fs::File::create(&archive_path).expect("create archive");
            let writer: Box<dyn WriteSeek> = Box::new(BufWriter::new(file));
            backend
                .create(
                    writer,
                    &paths,
                    &CreateOptions::default(),
                    None,
                    &NoOpProgress,
                    &Limits::default(),
                )
                .expect("zip create");
        }

        // Phase 2: list.
        let listed = backend
            .list(
                Box::new(std::fs::File::open(&archive_path).expect("reopen")),
                None,
                &Limits::default(),
            )
            .expect("zip list");
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
                &Limits::default(),
            )
            .expect("zip extract");

        for (name, expected) in &entries {
            let out_path = out_dir.path().join(name);
            let actual = std::fs::read(&out_path)
                .unwrap_or_else(|e| panic!("read {}: {e}", out_path.display()));
            prop_assert_eq!(&actual, expected);
        }
    }
}
