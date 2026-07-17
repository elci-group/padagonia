use crate::edge::Edge;
use crate::fact::FactSubject;
use crate::hnsw::{Distance, DEFAULT_EF_CONSTRUCTION, DEFAULT_M};
use crate::id::{LabelId, NodeId, RelationId};
use crate::node::Node;
use crate::provenance::Provenance;
use crate::store::Store;
use ahash::AHashSet;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Total-order wrapper for `f32` so it can live in a `BinaryHeap`.
#[derive(Copy, Clone, PartialEq)]
struct Score(f32);

impl Eq for Score {}

impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Score {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(Ordering::Equal)
    }
}

pub struct QueryEngine<'a> {
    store: &'a Store,
}

impl<'a> QueryEngine<'a> {
    pub fn new(store: &'a Store) -> Self {
        Self { store }
    }

    pub fn outgoing(&self, node: NodeId, relation: Option<RelationId>) -> Vec<&'a Edge> {
        self.store
            .outgoing
            .get(&node)
            .map(|ids| {
                ids.iter()
                    .filter_map(|&id| self.store.edges.get(&id))
                    .filter(|e| relation.is_none_or(|r| e.label == r))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn incoming(&self, node: NodeId, relation: Option<RelationId>) -> Vec<&'a Edge> {
        self.store
            .incoming
            .get(&node)
            .map(|ids| {
                ids.iter()
                    .filter_map(|&id| self.store.edges.get(&id))
                    .filter(|e| relation.is_none_or(|r| e.label == r))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn neighbors(&self, node: NodeId) -> Vec<NodeId> {
        let mut nbrs = Vec::new();
        if let Some(ids) = self.store.outgoing.get(&node) {
            nbrs.extend(
                ids.iter()
                    .filter_map(|&id| self.store.edges.get(&id).map(|e| e.dst)),
            );
        }
        if let Some(ids) = self.store.incoming.get(&node) {
            nbrs.extend(
                ids.iter()
                    .filter_map(|&id| self.store.edges.get(&id).map(|e| e.src)),
            );
        }
        nbrs.sort_by_key(|n| n.0);
        nbrs.dedup();
        nbrs
    }

    pub fn bfs(
        &self,
        start: NodeId,
        max_depth: usize,
        relation_filter: Option<RelationId>,
        min_confidence: Option<f32>,
    ) -> Vec<(NodeId, usize)> {
        if !self.store.nodes.contains_key(&start) {
            return Vec::new();
        }

        let mut visited = AHashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back((start, 0));
        visited.insert(start);
        let mut result = vec![(start, 0)];

        while let Some((current, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }
            let edges = self.outgoing(current, relation_filter);
            for edge in edges {
                let conf = edge.provenance.confidence;
                if let Some(min) = min_confidence {
                    if conf < min {
                        continue;
                    }
                }
                let next = edge.dst;
                if visited.insert(next) {
                    result.push((next, depth + 1));
                    queue.push_back((next, depth + 1));
                }
            }
        }
        result
    }

    pub fn facts_about(&self, subject: FactSubject) -> Vec<&'a Provenance> {
        self.store
            .facts
            .get(&subject)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    pub fn by_label(&self, label: LabelId) -> Vec<&'a Node> {
        self.store
            .node_label_index
            .get(&label)
            .map(|ids| {
                ids.iter()
                    .filter_map(|&id| self.store.nodes.get(&id))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn by_relation(&self, relation: RelationId) -> Vec<&'a Edge> {
        self.store
            .edge_label_index
            .get(&relation)
            .map(|ids| {
                ids.iter()
                    .filter_map(|&id| self.store.edges.get(&id))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn highest_confidence_fact(&self, subject: FactSubject) -> Option<&'a Provenance> {
        self.store.facts.get(&subject).and_then(|facts| {
            facts
                .iter()
                .max_by(|a, b| a.confidence.partial_cmp(&b.confidence).unwrap())
        })
    }

    /// Approximate vector search over node embeddings.
    pub fn vector_search(
        &self,
        query: &[f32],
        k: usize,
        label_filter: Option<LabelId>,
        ef: usize,
    ) -> Vec<(NodeId, f32)> {
        let index = self.store.build_hnsw_index(
            Distance::Euclidean,
            DEFAULT_M,
            DEFAULT_EF_CONSTRUCTION,
            ef.max(k),
        );
        let mut ef_cur = ef.max(k);
        loop {
            let results = index.search(query, k, ef_cur);
            let mut filtered: Vec<_> = results
                .into_iter()
                .filter_map(|(pid, dist)| {
                    let nid = NodeId(pid.0);
                    self.store.nodes.get(&nid).and_then(|n| {
                        if label_filter.is_none_or(|l| n.label == l) {
                            Some((nid, dist))
                        } else {
                            None
                        }
                    })
                })
                .collect();
            if filtered.len() >= k || ef_cur >= index.len() || label_filter.is_none() {
                filtered.truncate(k);
                return filtered;
            }
            ef_cur = (ef_cur * 2).min(index.len());
        }
    }

    /// Exact top-k vector search by full scan (for tests and benchmarks).
    pub fn brute_force_vector_search(
        &self,
        query: &[f32],
        k: usize,
        label_filter: Option<LabelId>,
    ) -> Vec<(NodeId, f32)> {
        let mut heap = BinaryHeap::<(Score, NodeId)>::new();
        for node in self.store.nodes.values() {
            if let Some(emb) = &node.embedding {
                if label_filter.is_none_or(|l| node.label == l) {
                    let d = Score(euclidean_distance(query, emb));
                    heap.push((d, node.id));
                    if heap.len() > k {
                        heap.pop();
                    }
                }
            }
        }
        let mut results: Vec<_> = heap.into_iter().map(|(Score(d), id)| (id, d)).collect();
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        results
    }
}

fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let d = x - y;
            d * d
        })
        .sum()
}
