use crate::line_index::{LineIndex, Range};
use crate::model::edge::{Edge, EdgeClaim};
use crate::model::identity::{EdgeId, NodeId};
use crate::model::node::{Attributes, Node, NodeClaim};
use crate::model::provenance::{Confidence, Derivation, FactOwner, Provenance, Revision};
use crate::model::registries::{ArtifactClass, EdgeKind, NodeKind};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct DiscriminatorTracker {
    counts: HashMap<(String, String, String), u32>,
}

impl DiscriminatorTracker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn next_discriminator(&mut self, kind: NodeKind, container_key: &str, name: &str) -> u32 {
        let key = (
            kind.as_str().to_string(),
            container_key.to_string(),
            name.to_string(),
        );
        let count = self.counts.entry(key).or_insert(0);
        let val = *count;
        *count += 1;
        val
    }
}

pub struct FactBuilder<'a> {
    pub root: &'a str,
    pub path: &'a str,
    pub language: &'a str,
    pub artifact_class: ArtifactClass,
    pub extractor_name: &'static str,
    pub extractor_version: &'static str,
    pub owner: FactOwner,
    pub line_index: &'a LineIndex,
    pub source: &'a [u8],
    pub tracker: DiscriminatorTracker,
}

impl<'a> FactBuilder<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        root: &'a str,
        path: &'a str,
        language: &'a str,
        artifact_class: ArtifactClass,
        extractor_name: &'static str,
        extractor_version: &'static str,
        line_index: &'a LineIndex,
        source: &'a [u8],
    ) -> Self {
        let owner = FactOwner::new(root, path, extractor_name, extractor_version);
        Self {
            root,
            path,
            language,
            artifact_class,
            extractor_name,
            extractor_version,
            owner,
            line_index,
            source,
            tracker: DiscriminatorTracker::new(),
        }
    }

    pub fn make_file_node(&self) -> NodeClaim {
        let id = NodeId::new(NodeKind::File, self.root, self.path, &[], self.path, 0);
        let node = Node {
            id,
            kind: NodeKind::File,
            name: self.path.to_string(),
            qualified_name: Some(self.path.to_string()),
            root: self.root.to_string(),
            path: self.path.to_string(),
            range: None,
            language: Some(self.language.to_string()),
            artifact_class: Some(self.artifact_class),
            provenance: Provenance {
                root: self.root.to_string(),
                path: self.path.to_string(),
                range: None,
                extractor: self.extractor_name.to_string(),
                extractor_version: self.extractor_version.to_string(),
                derivation: Derivation::Extracted,
                confidence: Confidence::EXACT,
                revision: Revision::INITIAL,
            },
            attributes: Attributes::default(),
        };

        NodeClaim {
            node,
            owner: self.owner.clone(),
        }
    }

    pub fn make_node(
        &mut self,
        kind: NodeKind,
        name: &str,
        qualified_name: Option<String>,
        container_chain: &[String],
        range: Option<Range>,
        attributes: Attributes,
    ) -> NodeClaim {
        let container_key = container_chain.join("::");
        let discriminator = self.tracker.next_discriminator(kind, &container_key, name);

        let id = NodeId::new(
            kind,
            self.root,
            self.path,
            container_chain,
            name,
            discriminator,
        );

        let node = Node {
            id,
            kind,
            name: name.to_string(),
            qualified_name,
            root: self.root.to_string(),
            path: self.path.to_string(),
            range,
            language: Some(self.language.to_string()),
            artifact_class: Some(self.artifact_class),
            provenance: Provenance {
                root: self.root.to_string(),
                path: self.path.to_string(),
                range,
                extractor: self.extractor_name.to_string(),
                extractor_version: self.extractor_version.to_string(),
                derivation: Derivation::Extracted,
                confidence: Confidence::EXACT,
                revision: Revision::INITIAL,
            },
            attributes,
        };

        NodeClaim {
            node,
            owner: self.owner.clone(),
        }
    }

    pub fn make_edge(
        &self,
        from: NodeId,
        to: NodeId,
        kind: EdgeKind,
        range: Option<Range>,
        attributes: Attributes,
    ) -> EdgeClaim {
        let id = EdgeId::new(from, to, kind, self.extractor_name);
        let edge = Edge {
            id,
            from,
            to,
            kind,
            provenance: Provenance {
                root: self.root.to_string(),
                path: self.path.to_string(),
                range,
                extractor: self.extractor_name.to_string(),
                extractor_version: self.extractor_version.to_string(),
                derivation: Derivation::Extracted,
                confidence: Confidence::EXACT,
                revision: Revision::INITIAL,
            },
            attributes,
        };

        EdgeClaim {
            edge,
            owner: self.owner.clone(),
        }
    }
}
