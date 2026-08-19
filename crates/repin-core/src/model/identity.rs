use crate::model::registries::{EdgeKind, NodeKind};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId([u8; 32]);

impl NodeId {
    pub fn new(
        kind: NodeKind,
        root: &str,
        path: &str,
        container_chain: &[String],
        name: &str,
        discriminator: u32,
    ) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"node:v1\0");
        hasher.update(kind.as_str().as_bytes());
        hasher.update(b"\0");
        hasher.update(root.as_bytes());
        hasher.update(b"\0");
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
        for container in container_chain {
            hasher.update(container.as_bytes());
            hasher.update(b"::");
        }
        hasher.update(b"\0");
        hasher.update(name.as_bytes());
        hasher.update(b"\0");
        hasher.update(&discriminator.to_le_bytes());

        Self(*hasher.finalize().as_bytes())
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for b in self.0 {
            use std::fmt::Write;
            let _ = write!(s, "{:02x}", b);
        }
        s
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "node_{}", &self.to_hex()[..12])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EdgeId([u8; 32]);

impl EdgeId {
    pub fn new(from: NodeId, to: NodeId, kind: EdgeKind, extractor: &str) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"edge:v1\0");
        hasher.update(from.as_bytes());
        hasher.update(b"\0");
        hasher.update(to.as_bytes());
        hasher.update(b"\0");
        hasher.update(kind.as_str().as_bytes());
        hasher.update(b"\0");
        hasher.update(extractor.as_bytes());

        Self(*hasher.finalize().as_bytes())
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for b in self.0 {
            use std::fmt::Write;
            let _ = write!(s, "{:02x}", b);
        }
        s
    }
}

impl fmt::Display for EdgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "edge_{}", &self.to_hex()[..12])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_stable_without_position() {
        let id1 = NodeId::new(
            NodeKind::Function,
            "root",
            "src/main.rs",
            &["crate".to_string()],
            "process",
            0,
        );
        let id2 = NodeId::new(
            NodeKind::Function,
            "root",
            "src/main.rs",
            &["crate".to_string()],
            "process",
            0,
        );
        assert_eq!(id1, id2);
    }
}
