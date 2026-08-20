use crate::extractor_util::{FactBuilder, node_text};
use repin_core::line_index::LineIndex;
use repin_core::model::identity::NodeId;
use repin_core::model::node::Attributes;
use repin_core::model::provenance::{Confidence, Derivation, Provenance, Revision};
use repin_core::model::registries::{EdgeKind, NodeKind};
use repin_core::model::unresolved::UnresolvedRef;
use repin_core::ports::fs::FileSnapshot;
use repin_core::ports::pack::{ExtractedFacts, ExtractionError, LanguagePack};
use tree_sitter::{Node as TsNode, Parser};

pub const RUST_PACK_VERSION: &str = "0.2.0";
pub struct RustLanguagePack;

impl Default for RustLanguagePack {
    fn default() -> Self {
        Self
    }
}

impl RustLanguagePack {
    pub fn new() -> Self {
        Self
    }
}

impl LanguagePack for RustLanguagePack {
    fn name(&self) -> &'static str {
        "rust_pack"
    }

    fn version(&self) -> &'static str {
        RUST_PACK_VERSION
    }

    fn can_handle(&self, path: &str, _sample_content: &[u8]) -> bool {
        path.ends_with(".rs")
    }

    fn extract(&self, snapshot: &FileSnapshot) -> Result<ExtractedFacts, ExtractionError> {
        let mut parser = Parser::new();
        let language = tree_sitter_rust::LANGUAGE.into();
        parser
            .set_language(&language)
            .map_err(|e| ExtractionError::ParseFailure(e.to_string()))?;

        let tree = parser.parse(&snapshot.content, None).ok_or_else(|| {
            ExtractionError::ParseFailure("failed to parse rust source".to_string())
        })?;

        let line_index = LineIndex::build(&snapshot.content);
        let mut builder = FactBuilder::new(
            &snapshot.root,
            &snapshot.path,
            "rust",
            snapshot.artifact_class,
            self.name(),
            self.version(),
            &line_index,
            &snapshot.content,
        );

        let file_node_claim = builder.make_file_node();
        let file_node_id = file_node_claim.node.id;

        let mut facts = ExtractedFacts::default();
        facts.nodes.push(file_node_claim);

        let mut container_chain = Vec::new();
        let mut parent_id_stack = vec![file_node_id];

        let root_node = tree.root_node();
        Self::traverse_node(
            &root_node,
            &snapshot.content,
            &mut builder,
            &mut container_chain,
            &mut parent_id_stack,
            &mut facts,
        );

        Ok(facts)
    }
}

impl RustLanguagePack {
    fn traverse_node(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
    ) {
        let kind = ts_node.kind();
        let current_parent_id = *parent_id_stack.last().unwrap();

        match kind {
            "function_item" => {
                if let Some(name_node) = ts_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let mut attrs = Attributes::default();

                    let is_method = !container_chain.is_empty()
                        && (container_chain.last().unwrap().starts_with("impl")
                            || container_chain.last().unwrap().starts_with("trait"));

                    let node_kind = if is_method {
                        NodeKind::Method
                    } else {
                        NodeKind::Function
                    };

                    // Check visibility
                    if let Some(vis_node) = ts_node.child_by_field_name("visibility") {
                        let vis_text = node_text(&vis_node, source);
                        attrs.insert("visibility".to_string(), serde_json::json!(vis_text));
                    }

                    // Extract doc comments
                    if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
                        attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }

                    let qualified = if container_chain.is_empty() {
                        Some(name.to_string())
                    } else {
                        Some(format!("{}::{}", container_chain.join("::"), name))
                    };

                    let fn_claim = builder.make_node(
                        node_kind,
                        name,
                        qualified,
                        container_chain,
                        ts_node,
                        attrs,
                    );
                    let fn_id = fn_claim.node.id;
                    let fn_range = fn_claim.node.range;
                    facts.nodes.push(fn_claim);

                    facts.edges.push(builder.make_edge(
                        current_parent_id,
                        fn_id,
                        EdgeKind::Contains,
                        fn_range,
                        Attributes::default(),
                    ));
                }
            }
            "struct_item" => {
                if let Some(name_node) = ts_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let mut attrs = Attributes::default();
                    if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
                        attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }

                    let qualified = if container_chain.is_empty() {
                        Some(name.to_string())
                    } else {
                        Some(format!("{}::{}", container_chain.join("::"), name))
                    };

                    let struct_claim = builder.make_node(
                        NodeKind::Struct,
                        name,
                        qualified,
                        container_chain,
                        ts_node,
                        attrs,
                    );
                    let struct_id = struct_claim.node.id;
                    let struct_range = struct_claim.node.range;
                    facts.nodes.push(struct_claim);

                    facts.edges.push(builder.make_edge(
                        current_parent_id,
                        struct_id,
                        EdgeKind::Contains,
                        struct_range,
                        Attributes::default(),
                    ));
                }
            }
            "enum_item" => {
                if let Some(name_node) = ts_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let mut attrs = Attributes::default();
                    if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
                        attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }

                    let enum_claim = builder.make_node(
                        NodeKind::Enum,
                        name,
                        Some(name.to_string()),
                        container_chain,
                        ts_node,
                        attrs,
                    );
                    let enum_id = enum_claim.node.id;
                    let enum_range = enum_claim.node.range;
                    facts.nodes.push(enum_claim);

                    facts.edges.push(builder.make_edge(
                        current_parent_id,
                        enum_id,
                        EdgeKind::Contains,
                        enum_range,
                        Attributes::default(),
                    ));
                }
            }
            "trait_item" => {
                if let Some(name_node) = ts_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let mut attrs = Attributes::default();
                    if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
                        attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }

                    let trait_claim = builder.make_node(
                        NodeKind::Trait,
                        name,
                        Some(name.to_string()),
                        container_chain,
                        ts_node,
                        attrs,
                    );
                    let trait_id = trait_claim.node.id;
                    let trait_range = trait_claim.node.range;
                    facts.nodes.push(trait_claim);

                    facts.edges.push(builder.make_edge(
                        current_parent_id,
                        trait_id,
                        EdgeKind::Contains,
                        trait_range,
                        Attributes::default(),
                    ));

                    // Recurse into trait methods
                    container_chain.push(format!("trait {name}"));
                    parent_id_stack.push(trait_id);

                    let mut cursor = ts_node.walk();
                    for child in ts_node.children(&mut cursor) {
                        Self::traverse_node(
                            &child,
                            source,
                            builder,
                            container_chain,
                            parent_id_stack,
                            facts,
                        );
                    }

                    parent_id_stack.pop();
                    container_chain.pop();
                }
            }
            "impl_item" => {
                let type_name = ts_node
                    .child_by_field_name("type")
                    .map(|n| node_text(&n, source).to_string())
                    .unwrap_or_else(|| "AnonymousType".to_string());

                let trait_name = ts_node
                    .child_by_field_name("trait")
                    .map(|n| node_text(&n, source).to_string());

                let container_label = if let Some(ref tr) = trait_name {
                    format!("impl {tr} for {type_name}")
                } else {
                    format!("impl {type_name}")
                };

                container_chain.push(container_label);

                let mut cursor = ts_node.walk();
                for child in ts_node.children(&mut cursor) {
                    Self::traverse_node(
                        &child,
                        source,
                        builder,
                        container_chain,
                        parent_id_stack,
                        facts,
                    );
                }

                container_chain.pop();
            }
            "mod_item" => {
                if let Some(name_node) = ts_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let mod_claim = builder.make_node(
                        NodeKind::Module,
                        name,
                        Some(name.to_string()),
                        container_chain,
                        ts_node,
                        Attributes::default(),
                    );
                    let mod_id = mod_claim.node.id;
                    let mod_range = mod_claim.node.range;
                    facts.nodes.push(mod_claim);

                    facts.edges.push(builder.make_edge(
                        current_parent_id,
                        mod_id,
                        EdgeKind::Contains,
                        mod_range,
                        Attributes::default(),
                    ));

                    container_chain.push(name.to_string());
                    parent_id_stack.push(mod_id);

                    let mut cursor = ts_node.walk();
                    for child in ts_node.children(&mut cursor) {
                        Self::traverse_node(
                            &child,
                            source,
                            builder,
                            container_chain,
                            parent_id_stack,
                            facts,
                        );
                    }

                    parent_id_stack.pop();
                    container_chain.pop();
                }
            }
            "use_declaration" => {
                let use_text = node_text(ts_node, source);
                let cleaned = use_text
                    .trim_start_matches("pub ")
                    .trim_start_matches("use ")
                    .trim_end_matches(';')
                    .trim();

                let seeking = cleaned.split("::").last().unwrap_or(cleaned).to_string();

                facts.unresolved.push(UnresolvedRef {
                    from: current_parent_id,
                    seeking,
                    scope_hint: Some(cleaned.to_string()),
                    edge_kind: EdgeKind::Imports,
                    provenance: Provenance {
                        root: builder.root.to_string(),
                        path: builder.path.to_string(),
                        range: None,
                        extractor: builder.extractor_name.to_string(),
                        extractor_version: builder.extractor_version.to_string(),
                        derivation: Derivation::Extracted,
                        confidence: Confidence::EXACT,
                        revision: Revision::INITIAL,
                    },
                });
            }
            _ => {
                let mut cursor = ts_node.walk();
                for child in ts_node.children(&mut cursor) {
                    Self::traverse_node(
                        &child,
                        source,
                        builder,
                        container_chain,
                        parent_id_stack,
                        facts,
                    );
                }
            }
        }
    }

    fn extract_doc_comment(ts_node: &TsNode<'_>, source: &[u8]) -> Option<String> {
        let mut prev = ts_node.prev_sibling();
        let mut doc_lines = Vec::new();

        while let Some(sibling) = prev {
            if sibling.kind() == "line_comment" {
                let text = node_text(&sibling, source).trim();
                if let Some(stripped) = text.strip_prefix("///") {
                    doc_lines.push(stripped.trim().to_string());
                }
                prev = sibling.prev_sibling();
            } else {
                break;
            }
        }

        if doc_lines.is_empty() {
            None
        } else {
            doc_lines.reverse();
            Some(doc_lines.join(" "))
        }
    }
}
