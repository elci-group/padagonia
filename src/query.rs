use crate::edge::Edge;
use crate::fact::FactSubject;
use crate::hnsw::{Distance, HnswParams};
use crate::id::{LabelId, NodeId, RelationId};
use crate::identity::NamespaceId;
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

#[derive(Clone, Debug, Default)]
pub struct NodeQuery {
    pub namespace: Option<NamespaceId>,
    pub label: Option<LabelId>,
    pub created_after: Option<u64>,
    pub created_before: Option<u64>,
    pub limit: usize,
    pub cursor: Option<String>,
}

pub struct NodePage<'a> {
    pub entries: Vec<&'a Node>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NumericAggregate {
    pub count: usize,
    pub sum: f64,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl<'a> QueryEngine<'a> {
    pub fn new(store: &'a Store) -> Self {
        Self { store }
    }

    pub fn exact_node(&self, namespace: &NamespaceId, external_id: &str) -> Option<&'a Node> {
        self.store
            .node_by_external_id(namespace, external_id)
            .and_then(|id| self.store.nodes.get(&id))
    }

    /// Bounded, stable pagination over node records.
    pub fn nodes_page(&self, query: &NodeQuery) -> NodePage<'a> {
        let limit = query.limit.clamp(1, 10_000);
        let after = query.cursor.as_deref().and_then(parse_cursor);
        let mut nodes: Vec<_> = query
            .label
            .and_then(|label| self.store.node_label_index.get(&label))
            .map_or_else(
                || self.store.nodes.values().collect(),
                |ids| {
                    ids.iter()
                        .filter_map(|id| self.store.nodes.get(id))
                        .collect()
                },
            );
        nodes.sort_by_key(|node| (node.created_at, node.id));
        let mut entries: Vec<&'a Node> = Vec::with_capacity(limit);
        let mut next_cursor = None;
        for node in nodes {
            if query
                .namespace
                .as_ref()
                .is_some_and(|ns| &node.namespace != ns)
                || query.created_after.is_some_and(|at| node.created_at <= at)
                || query.created_before.is_some_and(|at| node.created_at >= at)
                || after.is_some_and(|key| (node.created_at, node.id.0) <= key)
            {
                continue;
            }
            if entries.len() == limit {
                next_cursor = entries
                    .last()
                    .map(|last| format_cursor(last.created_at, last.id.0));
                break;
            }
            entries.push(node);
        }
        NodePage {
            entries,
            next_cursor,
        }
    }

    pub fn count_nodes(&self, namespace: Option<&NamespaceId>, label: Option<LabelId>) -> usize {
        label
            .and_then(|label| self.store.node_label_index.get(&label))
            .map_or_else(
                || {
                    self.store
                        .nodes
                        .values()
                        .filter(|node| namespace.is_none_or(|ns| &node.namespace == ns))
                        .count()
                },
                |ids| {
                    ids.iter()
                        .filter_map(|id| self.store.nodes.get(id))
                        .filter(|node| namespace.is_none_or(|ns| &node.namespace == ns))
                        .count()
                },
            )
    }

    pub fn aggregate_numeric_property(
        &self,
        namespace: Option<&NamespaceId>,
        label: Option<LabelId>,
        key: crate::KeyId,
    ) -> NumericAggregate {
        let nodes: Vec<&'a Node> = label
            .and_then(|label| self.store.node_label_index.get(&label))
            .map_or_else(
                || self.store.nodes.values().collect(),
                |ids| {
                    ids.iter()
                        .filter_map(|id| self.store.nodes.get(id))
                        .collect()
                },
            );
        let mut aggregate = NumericAggregate::default();
        for node in nodes {
            if namespace.is_some_and(|ns| &node.namespace != ns) {
                continue;
            }
            let Some(value) = node
                .properties
                .iter()
                .find_map(|(property_key, value)| {
                    (*property_key == key).then(|| match value {
                        crate::Scalar::I64(value) => Some(*value as f64),
                        crate::Scalar::F64(value) if value.is_finite() => Some(*value),
                        _ => None,
                    })
                })
                .flatten()
            else {
                continue;
            };
            aggregate.count += 1;
            aggregate.sum += value;
            aggregate.min = Some(aggregate.min.map_or(value, |current| current.min(value)));
            aggregate.max = Some(aggregate.max.map_or(value, |current| current.max(value)));
        }
        aggregate
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
            facts.iter().max_by(|a, b| {
                a.confidence
                    .partial_cmp(&b.confidence)
                    .unwrap_or(Ordering::Equal)
            })
        })
    }

    /// Approximate vector search over node embeddings using Euclidean distance.
    pub fn vector_search(
        &self,
        query: &[f32],
        k: usize,
        label_filter: Option<LabelId>,
        ef: usize,
    ) -> Vec<(NodeId, f32)> {
        self.vector_search_with_distance(Distance::Euclidean, query, k, label_filter, ef)
    }

    /// Approximate vector search over node embeddings with a chosen metric.
    ///
    /// The HNSW index is cached on the store and shared across calls; it is
    /// only rebuilt when nodes are added or the construction parameters
    /// change, so repeated queries do not pay the build cost.
    pub fn vector_search_with_distance(
        &self,
        distance: Distance,
        query: &[f32],
        k: usize,
        label_filter: Option<LabelId>,
        ef: usize,
    ) -> Vec<(NodeId, f32)> {
        let params = HnswParams {
            ef_search: ef,
            ..HnswParams::default()
        };
        self.vector_search_with_params(distance, params, query, k, label_filter)
    }

    /// Approximate vector search with explicit HNSW parameters.
    pub fn vector_search_with_params(
        &self,
        distance: Distance,
        params: HnswParams,
        query: &[f32],
        k: usize,
        label_filter: Option<LabelId>,
    ) -> Vec<(NodeId, f32)> {
        let index = self.store.cached_hnsw_index(
            distance,
            params.m,
            params.ef_construction,
            params.ef_search.max(k),
        );
        let mut ef_cur = params.ef_search.max(k);
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

fn format_cursor(created_at: u64, id: u64) -> String {
    format!("{created_at:016x}{id:016x}")
}

fn parse_cursor(cursor: &str) -> Option<(u64, u64)> {
    if cursor.len() != 32 {
        return None;
    }
    Some((
        u64::from_str_radix(&cursor[..16], 16).ok()?,
        u64::from_str_radix(&cursor[16..], 16).ok()?,
    ))
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
