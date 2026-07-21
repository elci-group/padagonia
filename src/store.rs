use crate::edge::Edge;
use crate::fact::FactSubject;
use crate::hnsw::{Distance, HnswIndex};
use crate::id::{EdgeId, LabelId, NodeId, RelationId};
use crate::node::Node;
use crate::ontology::{StringTable, StringTableExt};
use crate::provenance::Provenance;
use crate::value::Scalar;
use ahash::AHashMap;
use std::sync::{Arc, RwLock};

/// HNSW index cached on the store so repeated vector searches do not rebuild
/// it from scratch. Nodes are append-only, so the cache stays valid until the
/// next `add_node` (or a change in construction parameters).
struct CachedHnsw {
    distance: Distance,
    m: usize,
    ef_construction: usize,
    index: Arc<HnswIndex>,
}

impl std::fmt::Debug for CachedHnsw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CachedHnsw")
            .field("distance", &self.distance)
            .field("m", &self.m)
            .field("ef_construction", &self.ef_construction)
            .field("indexed_points", &self.index.len())
            .finish()
    }
}

/// In-memory append-only graph store with ontology interning and indexes.
///
/// The maps are crate-private: external consumers get read-only views through
/// the accessor methods, so immutability of stored nodes/edges is enforced by
/// the type system rather than by convention.
#[derive(Clone, Debug, Default)]
pub struct Store {
    pub(crate) string_table: StringTable,
    pub(crate) nodes: AHashMap<NodeId, Node>,
    pub(crate) edges: AHashMap<EdgeId, Edge>,
    pub(crate) facts: AHashMap<FactSubject, Vec<Provenance>>,
    pub(crate) node_label_index: AHashMap<LabelId, Vec<NodeId>>,
    pub(crate) edge_label_index: AHashMap<RelationId, Vec<EdgeId>>,
    pub(crate) outgoing: AHashMap<NodeId, Vec<EdgeId>>,
    pub(crate) incoming: AHashMap<NodeId, Vec<EdgeId>>,
    pub(crate) next_node_id: u64,
    pub(crate) next_edge_id: u64,
    hnsw_cache: Arc<RwLock<Option<CachedHnsw>>>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read-only view of the interned ontology strings.
    pub fn string_table(&self) -> &StringTable {
        &self.string_table
    }

    /// Read-only view of all nodes by id.
    pub fn nodes(&self) -> &AHashMap<NodeId, Node> {
        &self.nodes
    }

    /// Read-only view of all edges by id.
    pub fn edges(&self) -> &AHashMap<EdgeId, Edge> {
        &self.edges
    }

    /// Read-only view of the competing facts recorded for each subject.
    pub fn facts(&self) -> &AHashMap<FactSubject, Vec<Provenance>> {
        &self.facts
    }

    /// Read-only view of the node-label index.
    pub fn node_label_index(&self) -> &AHashMap<LabelId, Vec<NodeId>> {
        &self.node_label_index
    }

    /// Read-only view of the edge-relation index.
    pub fn edge_label_index(&self) -> &AHashMap<RelationId, Vec<EdgeId>> {
        &self.edge_label_index
    }

    /// Read-only view of the outgoing-adjacency index.
    pub fn outgoing(&self) -> &AHashMap<NodeId, Vec<EdgeId>> {
        &self.outgoing
    }

    /// Read-only view of the incoming-adjacency index.
    pub fn incoming(&self) -> &AHashMap<NodeId, Vec<EdgeId>> {
        &self.incoming
    }

    /// Id that will be assigned to the next added node.
    pub fn next_node_id(&self) -> u64 {
        self.next_node_id
    }

    /// Id that will be assigned to the next added edge.
    pub fn next_edge_id(&self) -> u64 {
        self.next_edge_id
    }

    pub fn intern_label(&mut self, s: &str) -> LabelId {
        self.string_table.intern_label(s)
    }

    pub fn intern_relation(&mut self, s: &str) -> RelationId {
        self.string_table.intern_relation(s)
    }

    pub fn intern_key(&mut self, s: &str) -> crate::id::KeyId {
        self.string_table.intern_key(s)
    }

    pub fn add_node(
        &mut self,
        label: &str,
        props: Vec<(&str, Scalar)>,
        embedding: Option<Vec<f32>>,
        provenance: Provenance,
    ) -> NodeId {
        let label_id = self.intern_label(label);
        let properties: Vec<_> = props
            .into_iter()
            .map(|(k, v)| (self.intern_key(k), v))
            .collect();
        let id = NodeId(self.next_node_id);
        self.next_node_id += 1;
        let node = Node {
            id,
            label: label_id,
            properties,
            embedding,
            provenance,
        };
        self.node_label_index.entry(label_id).or_default().push(id);
        let prov = node.provenance.clone();
        self.nodes.insert(id, node);
        self.add_fact(FactSubject::Node(id), prov);
        self.invalidate_hnsw_cache();
        id
    }

    pub fn add_edge(
        &mut self,
        src: NodeId,
        dst: NodeId,
        label: &str,
        props: Vec<(&str, Scalar)>,
        embedding: Option<Vec<f32>>,
        provenance: Provenance,
    ) -> EdgeId {
        let label_id = self.intern_relation(label);
        let properties: Vec<_> = props
            .into_iter()
            .map(|(k, v)| (self.intern_key(k), v))
            .collect();
        let id = EdgeId(self.next_edge_id);
        self.next_edge_id += 1;
        let edge = Edge {
            id,
            src,
            dst,
            label: label_id,
            properties,
            embedding,
            provenance,
        };
        self.edge_label_index.entry(label_id).or_default().push(id);
        self.outgoing.entry(src).or_default().push(id);
        self.incoming.entry(dst).or_default().push(id);
        let prov = edge.provenance.clone();
        self.edges.insert(id, edge);
        self.add_fact(FactSubject::Edge(id), prov);
        id
    }

    pub fn add_fact(&mut self, subject: FactSubject, provenance: Provenance) {
        self.facts.entry(subject).or_default().push(provenance);
    }

    pub fn stats(&self) -> (usize, usize, usize, usize, usize) {
        (
            self.nodes.len(),
            self.edges.len(),
            self.facts.values().map(|v| v.len()).sum(),
            self.node_label_index.len(),
            self.edge_label_index.len(),
        )
    }

    /// Build an approximate nearest-neighbor index over all node embeddings.
    pub fn build_hnsw_index(
        &self,
        distance: Distance,
        m: usize,
        ef_construction: usize,
        ef_search: usize,
    ) -> HnswIndex {
        let mut idx = HnswIndex::new(distance, m, ef_construction, ef_search);
        for node in self.nodes.values() {
            if let Some(emb) = &node.embedding {
                idx.insert(node.id, emb.clone());
            }
        }
        idx
    }

    /// Build an approximate nearest-neighbor index with a deterministic seed.
    pub fn build_hnsw_index_with_seed(
        &self,
        distance: Distance,
        m: usize,
        ef_construction: usize,
        ef_search: usize,
        seed: u64,
    ) -> HnswIndex {
        let mut idx = HnswIndex::with_seed(distance, m, ef_construction, ef_search, seed);
        for node in self.nodes.values() {
            if let Some(emb) = &node.embedding {
                idx.insert(node.id, emb.clone());
            }
        }
        idx
    }

    /// Return a shared HNSW index over all node embeddings, rebuilding it only
    /// when the construction parameters changed or nodes were added since the
    /// cached index was built.
    pub fn cached_hnsw_index(
        &self,
        distance: Distance,
        m: usize,
        ef_construction: usize,
        ef_search: usize,
    ) -> Arc<HnswIndex> {
        let matches = |cached: &CachedHnsw| {
            cached.distance == distance
                && cached.m == m
                && cached.ef_construction == ef_construction
        };
        if let Ok(guard) = self.hnsw_cache.read() {
            if let Some(cached) = guard.as_ref() {
                if matches(cached) {
                    return cached.index.clone();
                }
            }
        }
        let mut guard = self.hnsw_cache.write().unwrap_or_else(|e| e.into_inner());
        if let Some(cached) = guard.as_ref() {
            if matches(cached) {
                return cached.index.clone();
            }
        }
        let index = Arc::new(self.build_hnsw_index(distance, m, ef_construction, ef_search));
        *guard = Some(CachedHnsw {
            distance,
            m,
            ef_construction,
            index: index.clone(),
        });
        index
    }

    fn invalidate_hnsw_cache(&self) {
        if let Ok(mut guard) = self.hnsw_cache.write() {
            guard.take();
        }
    }
}
