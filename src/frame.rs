//! Frame encoding/decoding utilities for storage format with bounded streaming.

use crate::storage::{Result, StoreError};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::io::{Read, Write};

/// Maximum allowed frame size for security and memory safety.
/// This prevents malicious or corrupted files from causing excessive memory allocation.
pub const MAX_FRAME_BYTES: u64 = 1 << 30; // 1 GiB

/// Write a value as a frame with length prefix.
pub fn write_frame<W: Write, T: Serialize>(writer: &mut W, value: &T) -> Result<()> {
    let bytes = rmp_serde::to_vec(value)?;
    writer.write_all(&(bytes.len() as u64).to_le_bytes())?;
    writer.write_all(&bytes)?;
    Ok(())
}

/// Read a value from a length-prefixed frame with bounded size validation.
///
/// This implements bounded streaming decode by:
/// 1. Reading the length prefix first
/// 2. Validating the length against MAX_FRAME_BYTES before allocation
/// 3. Returning FrameTooLarge error if the frame exceeds the bound
/// 4. Only allocating memory after validation passes
pub fn read_frame<R: Read, T: DeserializeOwned>(reader: &mut R) -> Result<T> {
    let mut len_bytes = [0_u8; 8];
    reader.read_exact(&mut len_bytes)?;
    let len = u64::from_le_bytes(len_bytes);

    // Bounded decode: validate size before allocation
    if len > MAX_FRAME_BYTES {
        return Err(StoreError::FrameTooLarge { len });
    }

    let len = len as usize;
    let mut bytes = vec![0_u8; len];
    reader.read_exact(&mut bytes)?;
    Ok(rmp_serde::from_slice(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::FileHeader;
    use crate::ontology::StringTable;

    #[test]
    fn frame_write_roundtrip_preserves_data() {
        let header = FileHeader::new(StringTable::new(), 5);
        let mut buffer = Vec::new();

        write_frame(&mut buffer, &header).unwrap();
        let recovered: FileHeader = read_frame(&mut buffer.as_slice()).unwrap();

        assert_eq!(recovered.version, header.version);
        assert_eq!(recovered.block_count, header.block_count);
    }

    #[test]
    fn oversized_frame_is_rejected() {
        let mut buffer = Vec::new();
        // Write a length that exceeds MAX_FRAME_BYTES
        let oversized_len = MAX_FRAME_BYTES + 1;
        buffer.write_all(&oversized_len.to_le_bytes()).unwrap();

        let result: Result<FileHeader> = read_frame(&mut buffer.as_slice());
        assert!(matches!(result, Err(StoreError::FrameTooLarge { len }) if len == oversized_len));
    }
}
