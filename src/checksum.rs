//! Checksum validation utilities for storage blocks.

use crc32fast::Hasher;

pub fn compute_checksum(data: &[u8]) -> u32 {
    crc32fast::hash(data)
}

pub fn validate_checksum(data: &[u8], expected: u32) -> bool {
    compute_checksum(data) == expected
}

pub struct ChecksumHasher {
    hasher: Hasher,
}

impl ChecksumHasher {
    pub fn new() -> Self {
        Self {
            hasher: Hasher::new(),
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
    }

    pub fn finalize(self) -> u32 {
        self.hasher.finalize()
    }
}

impl Default for ChecksumHasher {
    fn default() -> Self {
        Self::new()
    }
}
