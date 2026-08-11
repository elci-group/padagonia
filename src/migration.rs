//! Storage migration layer for handling data format evolution.

use crate::block::{FileHeader, VERSION};
use crate::storage::{Result, StoreError};
use std::io::{BufReader, Read};

/// Migration strategy for different storage versions.
pub trait Migration {
    /// Returns the target version this migration can handle.
    fn target_version() -> u8;

    /// Migrates data from an older version to the current version.
    fn migrate<R: Read>(reader: &mut BufReader<R>, header: FileHeader) -> Result<bool>;
}

/// Compatibility migration for the v1 frame layout. v1 used the same framed
/// MessagePack block payloads; only the header version was missing the
/// explicit namespace/index contract. Loading it into the current in-memory
/// model is therefore a safe header upgrade.
struct V1ToCurrent;

impl Migration for V1ToCurrent {
    fn target_version() -> u8 {
        VERSION
    }

    fn migrate<R: Read>(_reader: &mut BufReader<R>, _header: FileHeader) -> Result<bool> {
        Ok(true)
    }
}

/// No-op migration for current version.
struct CurrentVersion;

impl Migration for CurrentVersion {
    fn target_version() -> u8 {
        VERSION
    }

    fn migrate<R: Read>(_reader: &mut BufReader<R>, _header: FileHeader) -> Result<bool> {
        // For current version, no migration needed
        Ok(false)
    }
}

/// Migration manager that applies the appropriate migration strategy.
pub struct MigrationManager;

impl MigrationManager {
    /// Check if migration is needed for the given header version.
    pub fn needs_migration(header: &FileHeader) -> bool {
        header.version < VERSION
    }

    /// Apply the appropriate migration strategy.
    /// Returns true if migration was applied, false if not needed.
    /// Supports the v1 framed layout and the current version.
    pub fn migrate<R: Read>(reader: &mut BufReader<R>, header: FileHeader) -> Result<bool> {
        // For MVP, we reject old versions. Future migrations will be added here.
        match header.version {
            0 => Err(StoreError::BadHeader),
            1 => V1ToCurrent::migrate(reader, header),
            _ => CurrentVersion::migrate(reader, header),
        }
    }

    /// Get supported version range.
    pub fn supported_versions() -> (u8, u8) {
        (1, VERSION)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::FileHeader;
    use crate::ontology::StringTable;

    #[test]
    fn needs_migration_detects_older_versions() {
        let mut header = FileHeader::new(StringTable::new(), 0);
        header.version = 1;
        assert!(MigrationManager::needs_migration(&header));

        header.version = VERSION;
        assert!(!MigrationManager::needs_migration(&header));
    }

    #[test]
    fn current_version_migration_is_noop() {
        assert_eq!(CurrentVersion::target_version(), VERSION);
    }

    #[test]
    fn supported_versions_includes_v1_forward_migration() {
        let (min, max) = MigrationManager::supported_versions();
        assert_eq!(min, 1);
        assert_eq!(max, VERSION);
    }
}
