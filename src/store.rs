use crate::edge::Edge;
use crate::fact::FactSubject;
use crate::hnsw::{Distance, HnswIndex};
use crate::id::{EdgeId, LabelId, NodeId, RelationId};
use crate::node::Node;
use crate::ontology::{StringTable, StringTableExt};
use crate::provenance::Provenance;
use crate::value::Scalar;
use ahash::AHashMap;

/// In-memory append-only graph store with ontology interning and indexes.
#[derive(Clone, Debug, Default)]
pub struct Store {
    pub string_table: StringTable,
    pub nodes: AHashMap<NodeId, Node>,
    pub edges: AHashMap<EdgeId, Edge>,
    pub facts: AHashMap<FactSubject, Vec<Provenance>>,
    pub node_label_index: AHashMap<LabelId, Vec<NodeId>>,
    pub edge_label_index: AHashMap<RelationId, Vec<EdgeId>>,
    pub outgoing: AHashMap<NodeId, Vec<EdgeId>>,
    pub incoming: AHashMap<NodeId, Vec<EdgeId>>,
    pub next_node_id: u64,
    pub next_edge_id: u64,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
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
                idx.insert(NodeId(node.id.0), emb.clone());
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
                idx.insert(NodeId(node.id.0), emb.clone());
            }
        }
        idx
    }
}
