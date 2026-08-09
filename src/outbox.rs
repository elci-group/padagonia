//! Compatibility outbox and shadow-read comparison helpers.

use crate::identity::NamespaceId;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OutboxEvent {
    pub sequence: u64,
    pub namespace: NamespaceId,
    pub event_type: String,
    pub payload: Vec<u8>,
}
#[derive(Clone, Debug, Default)]
pub struct Outbox {
    next_sequence: u64,
    events: VecDeque<OutboxEvent>,
}
impl Outbox {
    pub fn append(
        &mut self,
        namespace: NamespaceId,
        event_type: impl Into<String>,
        payload: Vec<u8>,
    ) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        self.events.push_back(OutboxEvent {
            sequence,
            namespace,
            event_type: event_type.into(),
            payload,
        });
        sequence
    }
    pub fn pending(&self, namespace: &NamespaceId, limit: usize) -> Vec<&OutboxEvent> {
        self.events
            .iter()
            .filter(|event| &event.namespace == namespace)
            .take(limit.clamp(1, 10_000))
            .collect()
    }
    pub fn acknowledge_through(&mut self, sequence: u64) {
        while self
            .events
            .front()
            .is_some_and(|event| event.sequence <= sequence)
        {
            self.events.pop_front();
        }
    }
    pub fn len(&self) -> usize {
        self.events.len()
    }
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct ShadowReadDiff<T> {
    pub canonical: T,
    pub candidate: T,
    pub equivalent: bool,
}
pub fn compare_shadow_reads<T: PartialEq>(canonical: T, candidate: T) -> ShadowReadDiff<T> {
    let equivalent = canonical == candidate;
    ShadowReadDiff {
        canonical,
        candidate,
        equivalent,
    }
}

#[derive(Debug)]
pub enum OutboxError {
    Io(io::Error),
    Encode(rmp_serde::encode::Error),
    Decode(rmp_serde::decode::Error),
    CorruptRecord,
    RecordTooLarge(u64),
}
impl From<io::Error> for OutboxError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<rmp_serde::encode::Error> for OutboxError {
    fn from(error: rmp_serde::encode::Error) -> Self {
        Self::Encode(error)
    }
}
impl From<rmp_serde::decode::Error> for OutboxError {
    fn from(error: rmp_serde::decode::Error) -> Self {
        Self::Decode(error)
    }
}
impl std::fmt::Display for OutboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "outbox I/O error: {error}"),
            Self::Encode(error) => write!(f, "outbox encode error: {error}"),
            Self::Decode(error) => write!(f, "outbox decode error: {error}"),
            Self::CorruptRecord => f.write_str("outbox checksum mismatch"),
            Self::RecordTooLarge(size) => write!(f, "outbox record too large: {size}"),
        }
    }
}
impl std::error::Error for OutboxError {}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum OutboxRecord {
    Event(OutboxEvent),
    Acknowledge(u64),
}

/// Durable outbox with replay-safe acknowledgement records.
pub struct PersistentOutbox {
    file: File,
    events: VecDeque<OutboxEvent>,
    next_sequence: u64,
}
impl std::fmt::Debug for PersistentOutbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistentOutbox")
            .field("events", &self.events.len())
            .field("next_sequence", &self.next_sequence)
            .finish()
    }
}
impl PersistentOutbox {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, OutboxError> {
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(path)?;
        let mut events = VecDeque::new();
        let mut next_sequence = 0;
        while let Some(record) = read_record(&mut file)? {
            match record {
                OutboxRecord::Event(event) => {
                    next_sequence = next_sequence.max(event.sequence + 1);
                    events.push_back(event);
                }
                OutboxRecord::Acknowledge(sequence) => {
                    while events
                        .front()
                        .is_some_and(|event| event.sequence <= sequence)
                    {
                        events.pop_front();
                    }
                }
            }
        }
        Ok(Self {
            file,
            events,
            next_sequence,
        })
    }
    pub fn append(
        &mut self,
        namespace: NamespaceId,
        event_type: impl Into<String>,
        payload: Vec<u8>,
    ) -> Result<u64, OutboxError> {
        let event = OutboxEvent {
            sequence: self.next_sequence,
            namespace,
            event_type: event_type.into(),
            payload,
        };
        self.next_sequence += 1;
        append_record(&mut self.file, &OutboxRecord::Event(event.clone()))?;
        self.events.push_back(event);
        Ok(self.next_sequence - 1)
    }
    pub fn pending(&self, namespace: &NamespaceId, limit: usize) -> Vec<&OutboxEvent> {
        self.events
            .iter()
            .filter(|event| &event.namespace == namespace)
            .take(limit.clamp(1, 10_000))
            .collect()
    }
    pub fn acknowledge_through(&mut self, sequence: u64) -> Result<(), OutboxError> {
        append_record(&mut self.file, &OutboxRecord::Acknowledge(sequence))?;
        while self
            .events
            .front()
            .is_some_and(|event| event.sequence <= sequence)
        {
            self.events.pop_front();
        }
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.events.len()
    }
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

fn append_record(file: &mut File, record: &OutboxRecord) -> Result<(), OutboxError> {
    let payload = rmp_serde::to_vec(record)?;
    file.write_all(&(payload.len() as u64).to_le_bytes())?;
    file.write_all(&crc32fast::hash(&payload).to_le_bytes())?;
    file.write_all(&payload)?;
    file.sync_data()?;
    Ok(())
}
fn read_record(file: &mut File) -> Result<Option<OutboxRecord>, OutboxError> {
    let mut length = [0; 8];
    match file.read_exact(&mut length) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let length = u64::from_le_bytes(length);
    if length > 64 * 1024 * 1024 {
        return Err(OutboxError::RecordTooLarge(length));
    }
    let mut checksum = [0; 4];
    file.read_exact(&mut checksum)?;
    let mut payload = vec![0; length as usize];
    file.read_exact(&mut payload)?;
    if crc32fast::hash(&payload) != u32::from_le_bytes(checksum) {
        return Err(OutboxError::CorruptRecord);
    }
    Ok(Some(rmp_serde::from_slice(&payload)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    #[test]
    fn outbox_is_tenant_scoped_and_acknowledgeable() {
        let first = NamespaceId::new("first").unwrap();
        let second = NamespaceId::new("second").unwrap();
        let mut outbox = Outbox::default();
        outbox.append(first.clone(), "run.created", vec![1]);
        let second_sequence = outbox.append(second, "run.created", vec![2]);
        assert_eq!(outbox.pending(&first, 10).len(), 1);
        outbox.acknowledge_through(second_sequence);
        assert!(outbox.is_empty());
    }
    #[test]
    fn shadow_diff_reports_equivalence() {
        assert!(compare_shadow_reads(vec![1, 2], vec![1, 2]).equivalent);
        assert!(!compare_shadow_reads(vec![1], vec![2]).equivalent);
    }

    #[test]
    fn persistent_outbox_replays_events_and_acknowledgements() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("outbox.log");
        let namespace = NamespaceId::new("workspace").unwrap();
        let mut outbox = PersistentOutbox::open(&path).unwrap();
        outbox
            .append(namespace.clone(), "run.created", vec![1])
            .unwrap();
        drop(outbox);
        let mut reopened = PersistentOutbox::open(&path).unwrap();
        assert_eq!(reopened.pending(&namespace, 10).len(), 1);
        reopened.acknowledge_through(0).unwrap();
        drop(reopened);
        assert_eq!(PersistentOutbox::open(&path).unwrap().len(), 0);
    }
}
