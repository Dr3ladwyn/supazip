//! Property-based round-trip test for the xz-compressed TAR backend.
//!
//! Invariant: for any list of `(name, bytes)` pairs, the `tar.xz` backend
//! must `create → list → extract` and the extracted file bodies must
//! match the originals byte-for-byte.
//!
//! The xz backend decompresses the whole stream into memory before
//! walking the tar; the default `Limits` allow the resulting resident
//! bytes. The proptest case count of 32 keeps wall time well under 5
//! seconds on the developer machine.

use proptest::prelude::*;

use supazip_core::formats::TarXzBackend;
use supazip_core::{ArchiveFormat, CreateOptions, Limits, NoOpProgress, WriteSeek};

use crate::common::{arb_entries, cases, materialize_entries};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(cases(32)))]
    #[test]
    fn round_trip_tar_xz(entries in arb_entries()) {
        let (_src_dir, paths) = materialize_entries(&entries);
        let backend = TarXzBackend::new();

        // Build the archive into a temp file; the writer needs to be
        // `'static`, which forbids a borrowed `&mut Vec<u8>`.
        let archive_tmp = tempfile::tempdir().expect("archive tempdir");
        let archive_path = archive_tmp.path().join("round.tar.xz");
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
                    &Limits::default(),
                )
                .expect("tar.xz create");
        }

        // Phase 2: list.
        let listed = backend
            .list(
                Box::new(std::fs::File::open(&archive_path).expect("reopen")),
                None,
                &Limits::default(),
            )
            .expect("tar.xz list");
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
            .expect("tar.xz extract");

        for (name, expected) in &entries {
            let out_path = out_dir.path().join(name);
            let actual = std::fs::read(&out_path)
                .unwrap_or_else(|e| panic!("read {}: {e}", out_path.display()));
            prop_assert_eq!(&actual, expected);
        }
    }
}
