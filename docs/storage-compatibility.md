# Storage Compatibility

PADAGONIA storage files are versioned independently from the crate version.
The current storage format version is **2**.

## Version 2

Version 2 files use:

- magic header `PADAGON\n`,
- a length-prefixed MessagePack `FileHeader`,
- one length-prefixed MessagePack `Block` per declared block,
- MessagePack block payloads,
- CRC32 checksums over each encoded block payload.

Loads reject:

- bad magic or unsupported version,
- truncated frames,
- frames larger than the implementation limit,
- CRC mismatches,
- trailing bytes after the declared block count.
- files above 8 GiB or headers declaring more than 1,000,000 blocks,
- duplicate node/edge identifiers, non-finite scalar or embedding data,
  inconsistent node embedding dimensions, invalid provenance ranges, and
  dangling semantic references.

Saves validate the same semantic invariants before touching the destination.
They write and sync a same-directory temporary file, atomically replace the
destination, and sync the parent directory on Unix. An unsuccessful encode or
rename leaves the previous complete graph in place and cleans its temporary
file. Platform filesystems must provide ordinary same-filesystem rename
semantics; network filesystems require deployment-specific validation.

## Compatibility Policy

PADAGONIA only guarantees read compatibility for storage versions that are
listed in this document. Unsupported versions fail with `StoreError::BadHeader`
unless and until a migration path is implemented.

Any future storage format change must:

- bump the storage format version,
- add a golden fixture for the new version,
- keep or explicitly retire previous-version fixtures,
- document migration behavior here,
- update README and CHANGELOG.
