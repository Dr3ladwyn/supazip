# Solid 7z — research note

> **Status:** research only. No code change in milestone 0.2.0.
> **Decision (provisional):** defer to 0.3.0; revisit if a user-visible
> use case appears.

## What "solid" means in 7z

A 7z archive is a sequence of **folders** (compressed blocks), and each
folder holds one or more entries. A **solid** archive uses a single folder
for every entry, so the LZMA2 dictionary spans the concatenation of all
files. The 7z format supports two layouts:

- **Non-solid (one block per entry).** Each file is compressed in its own
  folder. Reads can start at any entry, but compression ratio is the same
  as a per-file `gzip`/`xz` would produce.
- **Solid (one block for the whole archive).** All files share one
  dictionary. Compression ratio improves by a few percent on small files
  and dramatically on homogeneous corpora (logs, JSON dumps, source trees).
  The cost: random access is impossible — to extract entry N you have to
  decompress entries `0..N` first.

The 7z CLI's `-ms=on` flag turns solid on; the default for the 7z CLI is
**solid on** with a 4 GiB dictionary.

## `sevenz-rust` 0.6.x surface

`sevenz-rust` (the crate SupaZip depends on for read + write) does
**not** expose a `set_solid(true)` toggle. The behaviour is selected by
which `push_*` method you call on `SevenZWriter`:

| Method                              | Layout                                         |
|-------------------------------------|------------------------------------------------|
| `push_archive_entry(entry, reader)` | one folder per call ⇒ **non-solid**            |
| `push_archive_entries(vec, reader)` | one folder for the whole `vec` ⇒ **solid**     |
| `push_source_path(path, filter)`    | one folder for the path's contents ⇒ **solid** |
| `compress(src, dest)`               | one folder ⇒ **solid**                         |

The crate's own README confirms this in the "Solid compression" section
on docs.rs / crates.io: the example is

```rust
let mut sz = SevenZWriter::create("dest.7z").expect("create writer ok");
sz.push_source_path("path/to/compress", |_| true).expect("pack ok");
sz.finish().expect("compress ok");
```

with no `set_solid` call in sight. The "Compression methods" section
demonstrates the only configuration knob, `set_content_methods(...)`,
which selects the LZMA2 chain (and optionally AES-256 in front) but does
not control solid / non-solid.

### Why no explicit toggle

Looking at the crate's public API and the upstream 7z C library it
wraps, the writer is structured as "build one folder, then start a new
folder whenever the caller asks for a new entry". The call boundary
*is* the folder boundary. There is no buffer that lets you push several
"logical entries" into the same folder the way the 7z CLI does when
`-mcs=on` is passed.

That means choosing solid vs. non-solid in `sevenz-rust` is a matter of
which write entry point you call — not a parameter on a single entry
point. The 7z CLI's `-ms=on` would, in `sevenz-rust` terms, look like
"replace `push_archive_entry` with `push_archive_entries`" and accept
the same `Vec<SevenZArchiveEntry>` we already produce.

## `CreateOptions::solid` — what the milestone plan asked for

WS-A in `docs/milestones/m0.2.0-engine-solid.md` asks for a
`CreateOptions::solid: bool` field that `SevenZBackend` honours.
Mapping that to `sevenz-rust 0.6.x` is straightforward:

- `solid = false` ⇒ keep `push_archive_entry` in a loop (current code).
- `solid = true` ⇒ collect `entries` into a single `Vec`, then call
  `push_archive_entries` once. Caveat: every entry must already be
  openable as a `File`; `push_archive_entries` takes a single shared
  `SeqReader`, so files would have to be buffered or read in two passes.

### Why defer to 0.3.0

1. **No user-visible request.** No issue, no forum thread, no internal
   user story asks for solid 7z in SupaZip. The roadmap goal "7z solid
   archive API is exposed and tested" was written before
   `sevenz-rust 0.6` shipped, when the writer API still had open
   questions; the landscape has changed and the pressure is gone.
2. **The API surface is one method swap, not a feature.** Adding
   `CreateOptions::solid` plus a CLI flag is roughly a day's work, but
   it also needs integration tests for both layouts, a GUI string for
   the new checkbox, and a new entry in `en.toml` / `ru.toml` — a
   multiplier that doesn't pay back without a real consumer.
3. **Solid is the wrong default for a GUI archiver.** A user opening a
   large archive expects "click on `photos/img_42.jpg`, get that file
   only" to feel snappy. Solid forces a full prefix decompression, so
   turning it on silently would degrade the most common interaction.
   The right time to revisit is when (and if) the GUI ships a "create
   archive for long-term storage" mode that explains the trade-off.
4. **`sevenz-rust 0.6` is the wrong time.** The crate's 0.5 → 0.6
   transition shuffled the writer; the 0.7 line is on the horizon and
   may re-shuffle it. Pinning a solid layout to 0.6 means re-validating
   when we bump. WS-F (crates.io publish) wants a stable surface, not
   a freshly-added optional branch.

## What "defer" commits us to

- `CreateOptions` stays as it is (no `solid` field).
- The CLI / GUI do not gain a `--solid` flag.
- The 7z backend keeps the per-entry `push_archive_entry` loop.
- If a user files an issue asking for solid support, this document is
  the starting point: read it, then implement the `Vec` swap behind
  the same `CreateOptions::solid` knob, run the existing round-trip
  tests with `solid = true`, and add one cross-tool check (extract
  the produced archive with the upstream 7z CLI to confirm
  compatibility).

## References

- `sevenz-rust` 0.6.1 README, "Solid compression" + "Compression methods"
  sections — <https://docs.rs/sevenz-rust/0.6.1> and
  <https://crates.io/crates/sevenz-rust/0.6.1>.
- `SevenZWriter` API reference — <https://docs.rs/sevenz-rust/latest/sevenz_rust/struct.SevenZWriter.html>.
- Milestone plan: `docs/milestones/m0.2.0-engine-solid.md`, WS-A task 3.
