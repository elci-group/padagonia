//! Durable, replayable mutation batches.
//!
//! The journal is deliberately separate from snapshot storage. A caller can
//! fsync committed batches first and ship snapshots independently, which gives
//! recovery a clear acknowledged-write boundary.

use crate::identity::{IdentityError, NamespaceId};
use crate::{NodeId, Provenance, Scalar, Store};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

const MAX_RECORD_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Mutation {
    AddNode {
        namespace: NamespaceId,
        external_id: String,
        label: String,
        properties: Vec<(String, Scalar)>,
        embedding: Option<Vec<f32>>,
        provenance: Provenance,
    },
    AddEdge {
        namespace: NamespaceId,
        external_id: String,
        src: NodeId,
        dst: NodeId,
        label: String,
        properties: Vec<(String, Scalar)>,
        embedding: Option<Vec<f32>>,
        provenance: Provenance,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
    pub idempotency_key: String,
    pub mutations: Vec<Mutation>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CommitResult {
    pub sequence: u64,
    pub node_ids: Vec<NodeId>,
    pub edge_ids: Vec<crate::EdgeId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum JournalRecord {
    Prepare(Transaction),
    Commit {
        transaction: Transaction,
        result: CommitResult,
    },
}

#[derive(Debug)]
pub enum JournalError {
    Io(io::Error),
    Encode(rmp_serde::encode::Error),
    Decode(rmp_serde::decode::Error),
    CorruptRecord,
    RecordTooLarge(u64),
    EmptyIdempotencyKey,
    EmptyBatch,
    Identity(IdentityError),
    MissingPreparedTransaction,
}

impl fmt::Display for JournalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "journal I/O error: {error}"),
            Self::Encode(error) => write!(f, "journal encode error: {error}"),
            Self::Decode(error) => write!(f, "journal decode error: {error}"),
            Self::CorruptRecord => f.write_str("journal record checksum mismatch"),
            Self::RecordTooLarge(bytes) => write!(f, "journal record is too large: {bytes} bytes"),
            Self::EmptyIdempotencyKey => f.write_str("idempotency key must not be empty"),
            Self::EmptyBatch => f.write_str("transaction batch must not be empty"),
            Self::Identity(error) => write!(f, "invalid identity in transaction: {error}"),
            Self::MissingPreparedTransaction => {
                f.write_str("commit has no matching prepare record")
            }
        }
    }
}

impl std::error::Error for JournalError {}
impl From<io::Error> for JournalError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<rmp_serde::encode::Error> for JournalError {
    fn from(error: rmp_serde::encode::Error) -> Self {
        Self::Encode(error)
    }
}
impl From<rmp_serde::decode::Error> for JournalError {
    fn from(error: rmp_serde::decode::Error) -> Self {
        Self::Decode(error)
    }
}
impl From<IdentityError> for JournalError {
    fn from(error: IdentityError) -> Self {
        Self::Identity(error)
    }
}

/// Append-only journal. Loading validates every record before exposing it.
pub struct TransactionJournal {
    file: File,
    path: PathBuf,
    next_sequence: u64,
    committed: HashMap<String, (Transaction, CommitResult)>,
    prepared: HashMap<String, Transaction>,
}

impl fmt::Debug for TransactionJournal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransactionJournal")
            .field("next_sequence", &self.next_sequence)
            .field("committed", &self.committed.len())
            .field("prepared", &self.prepared.len())
            .finish()
    }
}

impl TransactionJournal {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, JournalError> {
        let path = path.as_ref().to_owned();
        let mut reader = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)?;
        let mut committed = HashMap::new();
        let mut prepared = HashMap::new();
        let mut sequence = 0;
        loop {
            match read_record(&mut reader)? {
                Some(JournalRecord::Prepare(transaction)) => {
                    validate_transaction(&transaction)?;
                    prepared.insert(transaction.idempotency_key.clone(), transaction);
                }
                Some(JournalRecord::Commit {
                    transaction,
                    result,
                }) => {
                    validate_transaction(&transaction)?;
                    if !prepared.contains_key(&transaction.idempotency_key) {
                        return Err(JournalError::MissingPreparedTransaction);
                    }
                    sequence = sequence.max(result.sequence + 1);
                    prepared.remove(&transaction.idempotency_key);
                    committed.insert(transaction.idempotency_key.clone(), (transaction, result));
                }
                None => break,
            }
        }
        Ok(Self {
            file: reader,
            path,
            next_sequence: sequence,
            committed,
            prepared,
        })
    }

    pub fn committed_result(&self, key: &str) -> Option<&CommitResult> {
        self.committed.get(key).map(|(_, result)| result)
    }

    /// Apply a batch atomically to a working clone and acknowledge it durably.
    pub fn commit(
        &mut self,
        store: &mut Store,
        transaction: Transaction,
    ) -> Result<CommitResult, JournalError> {
        validate_transaction(&transaction)?;
        if let Some((_, result)) = self.committed.get(&transaction.idempotency_key) {
            return Ok(result.clone());
        }

        append_record(&mut self.file, &JournalRecord::Prepare(transaction.clone()))?;
        self.prepared
            .insert(transaction.idempotency_key.clone(), transaction.clone());
        let mut working = store.clone();
        let (node_ids, edge_ids) = apply_mutations(&mut working, &transaction.mutations)?;
        let result = CommitResult {
            sequence: self.next_sequence,
            node_ids,
            edge_ids,
        };
        append_record(
            &mut self.file,
            &JournalRecord::Commit {
                transaction: transaction.clone(),
                result: result.clone(),
            },
        )?;
        self.next_sequence += 1;
        self.prepared.remove(&transaction.idempotency_key);
        self.committed.insert(
            transaction.idempotency_key.clone(),
            (transaction, result.clone()),
        );
        *store = working;
        Ok(result)
    }

    /// Replays all committed records into a fresh or restored store.
    pub fn replay(&self, store: &mut Store) -> Result<Vec<CommitResult>, JournalError> {
        let mut transactions: Vec<_> = self.committed.values().cloned().collect();
        transactions.sort_by_key(|(_, result)| result.sequence);
        let mut results = Vec::with_capacity(transactions.len());
        for (transaction, result) in transactions {
            let _ = apply_mutations(store, &transaction.mutations)?;
            results.push(result);
        }
        Ok(results)
    }

    /// Replays only records that are not already represented by the loaded
    /// snapshot. This makes startup safe when a process crashed after a
    /// journal commit but before the follow-up snapshot replacement.
    pub fn replay_missing(&self, store: &mut Store) -> Result<Vec<CommitResult>, JournalError> {
        let mut transactions: Vec<_> = self.committed.values().cloned().collect();
        transactions.sort_by_key(|(_, result)| result.sequence);
        let mut results = Vec::new();
        for (transaction, result) in transactions {
            let mut working = store.clone();
            let (nodes, edges) = apply_missing_mutations(&mut working, &transaction.mutations)?;
            if nodes.is_empty() && edges.is_empty() {
                continue;
            }
            *store = working;
            results.push(result);
        }
        Ok(results)
    }

    /// Discards journal history after a verified snapshot has become the
    /// checkpoint. The replacement is atomic and the parent directory is
    /// synced on Unix so a crash cannot leave a partial journal.
    pub fn checkpoint(&mut self) -> Result<(), JournalError> {
        let temporary = self
            .path
            .with_extension(format!("journal.tmp.{}", std::process::id()));
        let replacement = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        replacement.sync_all()?;
        std::fs::rename(&temporary, &self.path)?;
        if let Some(parent) = self.path.parent() {
            sync_directory(parent)?;
        }
        self.file = OpenOptions::new()
            .read(true)
            .append(true)
            .open(&self.path)?;
        self.next_sequence = 0;
        self.committed.clear();
        self.prepared.clear();
        Ok(())
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn validate_transaction(transaction: &Transaction) -> Result<(), JournalError> {
    if transaction.idempotency_key.trim().is_empty() {
        return Err(JournalError::EmptyIdempotencyKey);
    }
    if transaction.mutations.is_empty() {
        return Err(JournalError::EmptyBatch);
    }
    Ok(())
}

fn apply_mutations(
    store: &mut Store,
    mutations: &[Mutation],
) -> Result<(Vec<NodeId>, Vec<crate::EdgeId>), JournalError> {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for mutation in mutations {
        match mutation {
            Mutation::AddNode {
                namespace,
                external_id,
                label,
                properties,
                embedding,
                provenance,
            } => {
                let props = properties
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.clone()))
                    .collect();
                nodes.push(store.add_node_in_namespace(
                    namespace.clone(),
                    external_id.clone(),
                    label,
                    props,
                    embedding.clone(),
                    provenance.clone(),
                )?);
            }
            Mutation::AddEdge {
                namespace,
                external_id,
                src,
                dst,
                label,
                properties,
                embedding,
                provenance,
            } => {
                let props = properties
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.clone()))
                    .collect();
                edges.push(store.add_edge_in_namespace(
                    namespace.clone(),
                    external_id.clone(),
                    *src,
                    *dst,
                    label,
                    props,
                    embedding.clone(),
                    provenance.clone(),
                )?);
            }
        }
    }
    Ok((nodes, edges))
}

fn apply_missing_mutations(
    store: &mut Store,
    mutations: &[Mutation],
) -> Result<(Vec<NodeId>, Vec<crate::EdgeId>), JournalError> {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for mutation in mutations {
        match mutation {
            Mutation::AddNode {
                namespace,
                external_id,
                label,
                properties,
                embedding,
                provenance,
            } => {
                if let Some(id) = store.node_by_external_id(namespace, external_id) {
                    nodes.push(id);
                    continue;
                }
                let props = properties
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.clone()))
                    .collect();
                nodes.push(store.add_node_in_namespace(
                    namespace.clone(),
                    external_id.clone(),
                    label,
                    props,
                    embedding.clone(),
                    provenance.clone(),
                )?);
            }
            Mutation::AddEdge {
                namespace,
                external_id,
                src,
                dst,
                label,
                properties,
                embedding,
                provenance,
            } => {
                if let Some(id) = store.edge_by_external_id(namespace, external_id) {
                    edges.push(id);
                    continue;
                }
                let props = properties
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.clone()))
                    .collect();
                edges.push(store.add_edge_in_namespace(
                    namespace.clone(),
                    external_id.clone(),
                    *src,
                    *dst,
                    label,
                    props,
                    embedding.clone(),
                    provenance.clone(),
                )?);
            }
        }
    }
    Ok((nodes, edges))
}

fn append_record(file: &mut File, record: &JournalRecord) -> Result<(), JournalError> {
    let payload = rmp_serde::to_vec(record)?;
    let checksum = crc32fast::hash(&payload);
    file.write_all(&(payload.len() as u64).to_le_bytes())?;
    file.write_all(&checksum.to_le_bytes())?;
    file.write_all(&payload)?;
    file.sync_data()?;
    Ok(())
}

fn read_record(file: &mut File) -> Result<Option<JournalRecord>, JournalError> {
    let mut length = [0; 8];
    match file.read_exact(&mut length) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let length = u64::from_le_bytes(length);
    if length > MAX_RECORD_BYTES {
        return Err(JournalError::RecordTooLarge(length));
    }
    let mut checksum = [0; 4];
    file.read_exact(&mut checksum)?;
    let mut payload = vec![0; length as usize];
    file.read_exact(&mut payload)?;
    if crc32fast::hash(&payload) != u32::from_le_bytes(checksum) {
        return Err(JournalError::CorruptRecord);
    }
    Ok(Some(rmp_serde::from_slice(&payload)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn node(namespace: NamespaceId, id: &str) -> Mutation {
        Mutation::AddNode {
            namespace,
            external_id: id.into(),
            label: "Run".into(),
            properties: vec![],
            embedding: None,
            provenance: Provenance::new("test", "model", 1.0, 0.0, 1, vec![]),
        }
    }

    #[test]
    fn commit_is_idempotent_and_survives_reopen() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("events.padj");
        let namespace = NamespaceId::new("workspace").unwrap();
        let transaction = Transaction {
            idempotency_key: "ingest-1".into(),
            mutations: vec![node(namespace, "run-1")],
        };
        let mut store = Store::new();
        let mut journal = TransactionJournal::open(&path).unwrap();
        let first = journal.commit(&mut store, transaction.clone()).unwrap();
        let second = journal.commit(&mut store, transaction).unwrap();
        assert_eq!(first, second);
        assert_eq!(store.nodes().len(), 1);
        drop(journal);
        let reopened = TransactionJournal::open(&path).unwrap();
        assert_eq!(reopened.committed_result("ingest-1"), Some(&first));
    }

    #[test]
    fn truncated_tail_is_treated_as_uncommitted() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("events.padj");
        let namespace = NamespaceId::new("workspace").unwrap();
        let transaction = Transaction {
            idempotency_key: "ingest-1".into(),
            mutations: vec![node(namespace, "run-1")],
        };
        let mut store = Store::new();
        let mut journal = TransactionJournal::open(&path).unwrap();
        journal.commit(&mut store, transaction).unwrap();
        drop(journal);
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&[1, 2, 3]).unwrap();
        drop(file);
        assert!(TransactionJournal::open(&path).is_ok());
    }
}
