use padagonia::bench_support::generate_powerlaw;
use padagonia::id::{LabelId, NodeId};
use padagonia::storage::{Block, BlockKind, BlockPayload, FileHeader, StoreError};
use padagonia::store::Store;
use std::fs;

#[test]
fn saved_files_use_current_storage_version() {
    let mut store = Store::new();
    generate_powerlaw(&mut store, 10, 20, 3);

    let tmp = tempfile::NamedTempFile::new().unwrap();
    store.save(tmp.path()).unwrap();

    let bytes = fs::read(tmp.path()).unwrap();
    let (header, _) = read_frame::<FileHeader>(&bytes);
    assert_eq!(header.version, 2);
}

#[test]
fn old_storage_version_is_rejected() {
    let mut store = Store::new();
    generate_powerlaw(&mut store, 10, 20, 4);

    let tmp = tempfile::NamedTempFile::new().unwrap();
    store.save(tmp.path()).unwrap();

    let bytes = fs::read(tmp.path()).unwrap();
    let (mut header, rest_offset) = read_frame::<FileHeader>(&bytes);
    header.version = 1;

    let mut rewritten = encode_frame(&header);
    rewritten.extend_from_slice(&bytes[rest_offset..]);
    fs::write(tmp.path(), rewritten).unwrap();

    assert!(matches!(
        Store::load(tmp.path()),
        Err(StoreError::BadHeader)
    ));
}

#[test]
fn corrupted_block_checksum_is_rejected() {
    let mut store = Store::new();
    generate_powerlaw(&mut store, 10, 20, 5);

    let tmp = tempfile::NamedTempFile::new().unwrap();
    store.save(tmp.path()).unwrap();

    let bytes = fs::read(tmp.path()).unwrap();
    let (header, block_offset) = read_frame::<FileHeader>(&bytes);
    let (mut block, remaining_offset) = read_frame::<Block>(&bytes[block_offset..]);
    block.checksum ^= 1;

    let mut rewritten = encode_frame(&header);
    rewritten.extend_from_slice(&encode_frame(&block));
    rewritten.extend_from_slice(&bytes[block_offset + remaining_offset..]);
    fs::write(tmp.path(), rewritten).unwrap();

    assert!(matches!(
        Store::load(tmp.path()),
        Err(StoreError::CrcMismatch { block_index: 0 })
    ));
}

#[test]
fn trailing_bytes_after_declared_blocks_are_rejected() {
    let mut store = Store::new();
    generate_powerlaw(&mut store, 10, 20, 6);

    let tmp = tempfile::NamedTempFile::new().unwrap();
    store.save(tmp.path()).unwrap();

    let mut bytes = fs::read(tmp.path()).unwrap();
    bytes.extend_from_slice(b"trailing");
    fs::write(tmp.path(), bytes).unwrap();

    assert!(matches!(
        Store::load(tmp.path()),
        Err(StoreError::TrailingBytes { bytes: 8 })
    ));
}

#[test]
fn oversized_frame_is_rejected_before_allocation() {
    let tmp = tempfile::NamedTempFile::new().unwrap();
    fs::write(tmp.path(), (1_u64 << 40).to_le_bytes()).unwrap();

    assert!(matches!(
        Store::load(tmp.path()),
        Err(StoreError::FrameTooLarge { len }) if len == (1_u64 << 40)
    ));
}

#[test]
fn mismatched_block_kind_and_payload_are_rejected() {
    let mut store = Store::new();
    generate_powerlaw(&mut store, 10, 20, 7);

    let tmp = tempfile::NamedTempFile::new().unwrap();
    store.save(tmp.path()).unwrap();

    let bytes = fs::read(tmp.path()).unwrap();
    let (header, block_offset) = read_frame::<FileHeader>(&bytes);
    let (mut block, remaining_offset) = read_frame::<Block>(&bytes[block_offset..]);
    if let BlockKind::Nodes(label) = block.kind {
        block.kind = BlockKind::Nodes(LabelId(label.0 + 1));
    } else {
        panic!("expected first block to contain nodes");
    }
    block.checksum = crc32fast::hash(&block.payload);

    let mut rewritten = encode_frame(&header);
    rewritten.extend_from_slice(&encode_frame(&block));
    rewritten.extend_from_slice(&bytes[block_offset + remaining_offset..]);
    fs::write(tmp.path(), rewritten).unwrap();

    assert!(matches!(
        Store::load(tmp.path()),
        Err(StoreError::InconsistentBlockPayload)
    ));
}

#[test]
fn node_label_outside_string_table_is_rejected() {
    let mut store = Store::new();
    generate_powerlaw(&mut store, 10, 20, 8);

    let tmp = tempfile::NamedTempFile::new().unwrap();
    store.save(tmp.path()).unwrap();

    let bytes = fs::read(tmp.path()).unwrap();
    let (header, block_offset) = read_frame::<FileHeader>(&bytes);
    let (mut block, remaining_offset) = read_frame::<Block>(&bytes[block_offset..]);
    let mut payload = rmp_serde::from_slice::<BlockPayload>(&block.payload).unwrap();
    if let BlockPayload::Nodes { label, nodes } = &mut payload {
        let unknown = LabelId(header.string_table.len() as u32 + 100);
        *label = unknown;
        nodes[0].label = unknown;
        block.kind = BlockKind::Nodes(unknown);
    } else {
        panic!("expected first block to contain nodes");
    }
    block.payload = rmp_serde::to_vec(&payload).unwrap();
    block.checksum = crc32fast::hash(&block.payload);

    let mut rewritten = encode_frame(&header);
    rewritten.extend_from_slice(&encode_frame(&block));
    rewritten.extend_from_slice(&bytes[block_offset + remaining_offset..]);
    fs::write(tmp.path(), rewritten).unwrap();

    assert!(matches!(
        Store::load(tmp.path()),
        Err(StoreError::UnknownStringId { .. })
    ));
}

#[test]
fn dangling_edge_is_rejected() {
    let mut store = Store::new();
    generate_powerlaw(&mut store, 10, 20, 9);

    let tmp = tempfile::NamedTempFile::new().unwrap();
    store.save(tmp.path()).unwrap();

    let bytes = fs::read(tmp.path()).unwrap();
    let (header, mut offset) = read_frame::<FileHeader>(&bytes);
    let mut prefix = Vec::new();
    let mut suffix_offset = offset;
    let mut edge_block = None;
    for _ in 0..header.block_count {
        let block_start = offset;
        let (block, consumed) = read_frame::<Block>(&bytes[offset..]);
        offset += consumed;
        if matches!(block.kind, BlockKind::Edges(_)) {
            suffix_offset = offset;
            edge_block = Some(block);
            break;
        }
        prefix.extend_from_slice(&bytes[block_start..offset]);
    }
    let mut edge_block = edge_block.expect("expected an edge block");
    let mut payload = rmp_serde::from_slice::<BlockPayload>(&edge_block.payload).unwrap();
    if let BlockPayload::Edges { edges, .. } = &mut payload {
        edges[0].src = NodeId(999_999);
    } else {
        panic!("expected second block to contain edges");
    }
    edge_block.payload = rmp_serde::to_vec(&payload).unwrap();
    edge_block.checksum = crc32fast::hash(&edge_block.payload);

    let mut rewritten = encode_frame(&header);
    rewritten.extend_from_slice(&prefix);
    rewritten.extend_from_slice(&encode_frame(&edge_block));
    rewritten.extend_from_slice(&bytes[suffix_offset..]);
    fs::write(tmp.path(), rewritten).unwrap();

    assert!(matches!(
        Store::load(tmp.path()),
        Err(StoreError::DanglingEdge { .. })
    ));
}

fn read_frame<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> (T, usize) {
    let mut len_bytes = [0_u8; 8];
    len_bytes.copy_from_slice(&bytes[..8]);
    let len = u64::from_le_bytes(len_bytes) as usize;
    let start = 8;
    let end = start + len;
    (rmp_serde::from_slice(&bytes[start..end]).unwrap(), end)
}

fn encode_frame<T: serde::Serialize>(value: &T) -> Vec<u8> {
    let bytes = rmp_serde::to_vec(value).unwrap();
    let mut frame = (bytes.len() as u64).to_le_bytes().to_vec();
    frame.extend_from_slice(&bytes);
    frame
}
