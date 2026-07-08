use crate::provenance::Provenance;
use crate::store::Store;
use crate::value::Scalar;

/// SplitMix64 deterministic RNG.
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    }

    pub fn next_f32(&mut self) -> f32 {
        ((self.next() >> 32) as u32) as f32 / u32::MAX as f32
    }

    pub fn next_usize(&mut self, max: usize) -> usize {
        if max == 0 {
            0
        } else {
            (self.next() as usize) % max
        }
    }
}

const NODE_LABELS: &[&str] = &["Person", "Company", "Topic"];
const RELATIONS: &[&str] = &["works_for", "knows", "located_in", "related_to"];

/// Generate a deterministic power-law-ish graph for benchmarking.
pub fn generate_powerlaw(store: &mut Store, nodes: usize, edges: usize, seed: u64) {
    let mut rng = Rng::new(seed);
    let provenance = Provenance::new(
        "bench_agent",
        "bench_model",
        0.85,
        0.01,
        1,
        vec!["synthetic".to_string()],
    );

    let mut node_ids = Vec::with_capacity(nodes);
    for i in 0..nodes {
        let label = NODE_LABELS[rng.next_usize(NODE_LABELS.len())];
        let name = format!("node_{}", i);
        let score = rng.next() as i64 % 1000;
        let age = Scalar::I64((rng.next() % 80 + 18) as i64);
        let embedding: Vec<f32> = (0..16).map(|_| rng.next_f32()).collect();
        let id = store.add_node(
            label,
            vec![("name", Scalar::String(name)), ("score", Scalar::I64(score)), ("age", age)],
            Some(embedding),
            provenance.clone(),
        );
        node_ids.push(id);
    }

    // Preferential-ish attachment: sample source from power-law tail.
    for _ in 0..edges {
        let (src, dst) = loop {
            let src_idx = powerlaw_index(&mut rng, nodes, 2.0);
            let dst_idx = powerlaw_index(&mut rng, nodes, 2.0);
            let src = node_ids[src_idx.min(nodes - 1)];
            let dst = node_ids[dst_idx.min(nodes - 1)];
            if src != dst {
                break (src, dst);
            }
        };
        let rel = RELATIONS[rng.next_usize(RELATIONS.len())];
        let since = rng.next() % 30 + 1990;
        let confidence = 0.5 + rng.next_f32() * 0.5;
        let mut edge_prov = provenance.clone();
        edge_prov.confidence = confidence;
        let embedding: Vec<f32> = (0..16).map(|_| rng.next_f32()).collect();
        store.add_edge(
            src,
            dst,
            rel,
            vec![("since", Scalar::I64(since as i64))],
            Some(embedding),
            edge_prov,
        );
    }
}

fn powerlaw_index(rng: &mut Rng, n: usize, alpha: f64) -> usize {
    if n == 0 {
        return 0;
    }
    let u = rng.next_f32() as f64;
    let x = (1.0 - u).powf(-1.0 / (alpha - 1.0));
    let idx = (x as usize) % n;
    idx
}

/// Generate a small deterministic graph for tests.
pub fn generate_test_graph(store: &mut Store, nodes: usize, edges: usize, seed: u64) {
    generate_powerlaw(store, nodes, edges, seed);
}

/// Generate `count` nodes with `dim`-dimensional random embeddings.
pub fn generate_vectors(store: &mut Store, count: usize, dim: usize, seed: u64) {
    let mut rng = Rng::new(seed);
    let provenance = Provenance::new(
        "bench_agent",
        "bench_model",
        0.85,
        0.01,
        1,
        vec!["synthetic".to_string()],
    );

    for i in 0..count {
        let name = format!("vector_{}", i);
        let embedding: Vec<f32> = (0..dim).map(|_| rng.next_f32()).collect();
        store.add_node(
            "Vector",
            vec![("name", Scalar::String(name))],
            Some(embedding),
            provenance.clone(),
        );
    }
}
