use crate::id::NodeId;
use crate::ontology::StringTableExt;
use crate::store::Store;
use ahash::AHashMap;
use serde_json::{Map, Value};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

pub trait Projection {
    fn to_adjacency_list(&self) -> AHashMap<NodeId, Vec<NodeId>>;
    fn to_json(&self) -> Value;
    fn to_jsonl<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()>;
    fn to_csv_nodes<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()>;
    fn to_csv_edges<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()>;
}

fn scalar_to_json(scalar: &crate::value::Scalar) -> Value {
    use crate::value::Scalar;
    match scalar {
        Scalar::Null => Value::Null,
        Scalar::Bool(b) => Value::Bool(*b),
        Scalar::I64(i) => Value::Number((*i).into()),
        Scalar::F64(f) => {
            Value::Number(serde_json::Number::from_f64(*f).unwrap_or_else(|| 0.into()))
        }
        Scalar::String(s) => Value::String(s.clone()),
        Scalar::Bytes(b) => Value::Array(b.iter().map(|x| Value::Number((*x).into())).collect()),
        Scalar::Embedding(v) => Value::Array(
            v.iter()
                .map(|x| {
                    Value::Number(
                        serde_json::Number::from_f64(*x as f64).unwrap_or_else(|| 0.into()),
                    )
                })
                .collect(),
        ),
        Scalar::Timestamp(t) => Value::Number((*t).into()),
    }
}

fn props_to_json(props: &[(crate::id::KeyId, crate::value::Scalar)], store: &Store) -> Value {
    let mut map = Map::new();
    for (k, v) in props {
        let key = store
            .string_table
            .resolve_key(*k)
            .unwrap_or("?")
            .to_string();
        map.insert(key, scalar_to_json(v));
    }
    Value::Object(map)
}

impl Projection for Store {
    fn to_adjacency_list(&self) -> AHashMap<NodeId, Vec<NodeId>> {
        let mut adj: AHashMap<NodeId, Vec<NodeId>> = AHashMap::new();
        for node in self.nodes.keys() {
            adj.entry(*node).or_default();
        }
        for edge in self.edges.values() {
            adj.entry(edge.src).or_default().push(edge.dst);
        }
        adj
    }

    fn to_json(&self) -> Value {
        let nodes: Vec<Value> = self
            .nodes
            .values()
            .map(|n| {
                let mut obj = Map::new();
                obj.insert("id".to_string(), Value::Number(n.id.0.into()));
                obj.insert(
                    "label".to_string(),
                    Value::String(
                        self.string_table
                            .resolve_label(n.label)
                            .unwrap_or("?")
                            .to_string(),
                    ),
                );
                obj.insert("properties".to_string(), props_to_json(&n.properties, self));
                if let Some(emb) = &n.embedding {
                    obj.insert(
                        "embedding".to_string(),
                        Value::Array(
                            emb.iter()
                                .map(|x| {
                                    Value::Number(
                                        serde_json::Number::from_f64(*x as f64)
                                            .unwrap_or_else(|| 0.into()),
                                    )
                                })
                                .collect(),
                        ),
                    );
                }
                Value::Object(obj)
            })
            .collect();
        let edges: Vec<Value> = self
            .edges
            .values()
            .map(|e| {
                let mut obj = Map::new();
                obj.insert("id".to_string(), Value::Number(e.id.0.into()));
                obj.insert("src".to_string(), Value::Number(e.src.0.into()));
                obj.insert("dst".to_string(), Value::Number(e.dst.0.into()));
                obj.insert(
                    "label".to_string(),
                    Value::String(
                        self.string_table
                            .resolve_relation(e.label)
                            .unwrap_or("?")
                            .to_string(),
                    ),
                );
                obj.insert("properties".to_string(), props_to_json(&e.properties, self));
                Value::Object(obj)
            })
            .collect();
        let mut root = Map::new();
        root.insert("nodes".to_string(), Value::Array(nodes));
        root.insert("edges".to_string(), Value::Array(edges));
        Value::Object(root)
    }

    fn to_jsonl<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        for node in self.nodes.values() {
            let mut obj = Map::new();
            obj.insert("type".to_string(), Value::String("node".to_string()));
            obj.insert("id".to_string(), Value::Number(node.id.0.into()));
            obj.insert(
                "label".to_string(),
                Value::String(
                    self.string_table
                        .resolve_label(node.label)
                        .unwrap_or("?")
                        .to_string(),
                ),
            );
            obj.insert(
                "properties".to_string(),
                props_to_json(&node.properties, self),
            );
            writeln!(writer, "{}", Value::Object(obj))?;
        }
        for edge in self.edges.values() {
            let mut obj = Map::new();
            obj.insert("type".to_string(), Value::String("edge".to_string()));
            obj.insert("id".to_string(), Value::Number(edge.id.0.into()));
            obj.insert("src".to_string(), Value::Number(edge.src.0.into()));
            obj.insert("dst".to_string(), Value::Number(edge.dst.0.into()));
            obj.insert(
                "label".to_string(),
                Value::String(
                    self.string_table
                        .resolve_relation(edge.label)
                        .unwrap_or("?")
                        .to_string(),
                ),
            );
            obj.insert(
                "properties".to_string(),
                props_to_json(&edge.properties, self),
            );
            writeln!(writer, "{}", Value::Object(obj))?;
        }
        writer.flush()
    }

    fn to_csv_nodes<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        writeln!(writer, "id,label,properties")?;
        for node in self.nodes.values() {
            let label = self.string_table.resolve_label(node.label).unwrap_or("?");
            let props =
                serde_json::to_string(&props_to_json(&node.properties, self)).unwrap_or_default();
            writeln!(
                writer,
                "{},{},\"{}\"",
                node.id.0,
                label,
                props.replace('"', "\"\"")
            )?;
        }
        writer.flush()
    }

    fn to_csv_edges<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        writeln!(writer, "id,src,dst,label,properties")?;
        for edge in self.edges.values() {
            let label = self
                .string_table
                .resolve_relation(edge.label)
                .unwrap_or("?");
            let props =
                serde_json::to_string(&props_to_json(&edge.properties, self)).unwrap_or_default();
            writeln!(
                writer,
                "{},{},{},{},\"{}\"",
                edge.id.0,
                edge.src.0,
                edge.dst.0,
                label,
                props.replace('"', "\"\"")
            )?;
        }
        writer.flush()
    }
}
