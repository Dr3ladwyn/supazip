//! Property-based round-trip tests for every backend.
//!
//! WS-G (m0.3.0) — invariant: `create → list → extract → bytes-equal`.
//! WS-I (m0.4.0) — metadata invariant tests (count, size, names, traversal).
//!
//! The five round-trip modules below share a single helper module
//! (`common`) that defines the entry strategy and the on-disk materialiser.
//! The `metadata` module adds structural metadata-invariant checks on top.
//! We pull each backend's test file in via `#[path]` because Cargo's
//! integration-test target only picks up `*.rs` directly under `tests/`;
//! `tests/proptests/*.rs` would be invisible otherwise.

// `proptest` is a dev-dependency; the test-target crate sees it through
// its `[dev-dependencies]` table. We re-export it as an explicit item so
// submodules loaded via `#[path]` can resolve `use proptest::...`.
extern crate proptest;

#[path = "proptests/common.rs"]
mod common;

#[path = "proptests/round_trip_zip.rs"]
mod round_trip_zip;

#[path = "proptests/round_trip_sevenz.rs"]
mod round_trip_sevenz;

#[path = "proptests/round_trip_tar.rs"]
mod round_trip_tar;

#[path = "proptests/round_trip_tar_gz.rs"]
mod round_trip_tar_gz;

#[path = "proptests/round_trip_tar_xz.rs"]
mod round_trip_tar_xz;

#[path = "proptests/metadata.rs"]
mod metadata;
