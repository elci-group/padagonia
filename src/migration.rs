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
    /// Currently only supports v2 (current), rejects older versions.
    pub fn migrate<R: Read>(reader: &mut BufReader<R>, header: FileHeader) -> Result<bool> {
        // For MVP, we reject old versions. Future migrations will be added here.
        match header.version {
            0 | 1 => Err(StoreError::BadHeader),
            _ => CurrentVersion::migrate(reader, header),
        }
    }

    /// Get supported version range.
    pub fn supported_versions() -> (u8, u8) {
        (VERSION, VERSION) // Only current version is supported for MVP
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
    fn supported_versions_returns_current_only() {
        let (min, max) = MigrationManager::supported_versions();
        assert_eq!(min, VERSION);
        assert_eq!(max, VERSION);
    }
}
