use crate::id::NodeId;
use fast_hnsw::distance::Distance as FastDistance;
use fast_hnsw::{Builder, Hnsw};
use rand::{Rng, SeedableRng};

/// Distance metric used by the HNSW index.
///
/// Note: `Euclidean` is implemented with squared L2 distance internally
/// because it preserves nearest-neighbour ordering and is significantly
/// faster. Returned distances are therefore squared L2 values.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Distance {
    Euclidean,
    Cosine,
}

/// Reuse the graph `NodeId` to identify a point in the vector index.
pub type PointId = NodeId;

enum InnerMetric {
    SquaredEuclidean,
    Cosine,
}

impl From<Distance> for InnerMetric {
    fn from(d: Distance) -> Self {
        match d {
            Distance::Euclidean => InnerMetric::SquaredEuclidean,
            Distance::Cosine => InnerMetric::Cosine,
        }
    }
}

impl FastDistance for InnerMetric {
    #[inline]
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        match self {
            InnerMetric::SquaredEuclidean => {
                a.iter().zip(b.iter()).map(|(x, y)| {
                    let d = x - y;
                    d * d
                }).sum()
            }
            InnerMetric::Cosine => cosine_distance(a, b),
        }
    }
}

/// PADAGONIA's approximate nearest-neighbour index.
///
/// This is a thin wrapper around the `fast-hnsw` crate so that embeddings
/// are a first-class query primitive inside PADAGONIA. It maps the
/// graph's `NodeId`s to the index's internal sequential ids, which means
/// nodes do not need to be inserted in id order and gaps are allowed.
pub struct HnswIndex {
    inner: Hnsw<InnerMetric>,
    id_map: Vec<PointId>,
    #[allow(dead_code)]
    ef_search: usize,
}

impl HnswIndex {
    /// Create an index with a random seed.
    pub fn new(distance: Distance, m: usize, ef_construction: usize, ef_search: usize) -> Self {
        let seed = rand::rngs::StdRng::from_entropy().gen();
        Self::with_seed(distance, m, ef_construction, ef_search, seed)
    }

    /// Create an index with an explicit seed for deterministic behaviour.
    pub fn with_seed(
        distance: Distance,
        m: usize,
        ef_construction: usize,
        ef_search: usize,
        seed: u64,
    ) -> Self {
        let metric = InnerMetric::from(distance);
        let inner = Builder::new()
            .m(m)
            .ef_construction(ef_construction)
            .seed(seed)
            .build(metric);
        Self {
            inner,
            id_map: Vec::new(),
            ef_search,
        }
    }

    /// Number of indexed points.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Insert a new point with its embedding vector.
    ///
    /// `id` is the graph's `NodeId`; the index keeps an internal mapping so
    /// search results can return the original graph id.
    pub fn insert(&mut self, id: PointId, vector: Vec<f32>) {
        let internal_id = self.inner.insert(vector);
        if internal_id == self.id_map.len() {
            self.id_map.push(id);
        } else {
            self.id_map[internal_id] = id;
        }
    }

    /// Approximate k-NN search.
    ///
    /// `ef` controls the size of the dynamic candidate list. The effective
    /// search ef is at least `k`.
    pub fn search(&self, query: &[f32], k: usize, ef: usize) -> Vec<(PointId, f32)> {
        if self.is_empty() {
            return Vec::new();
        }
        let k = k.min(self.len());
        if k == 0 {
            return Vec::new();
        }
        self.inner
            .search(query, k, ef)
            .into_iter()
            .map(|r| (self.id_map[r.id], r.distance))
            .collect()
    }
}

fn cosine_distance(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a == 0.0 || norm_b == 0.0 {
        return 1.0;
    }
    let cos = (dot / (norm_a.sqrt() * norm_b.sqrt())).clamp(-1.0, 1.0);
    1.0 - cos
}
