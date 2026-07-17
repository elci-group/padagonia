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
