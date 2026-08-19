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

pub struct TsLanguagePack;

impl Default for TsLanguagePack {
    fn default() -> Self {
        Self
    }
}

impl TsLanguagePack {
    pub fn new() -> Self {
        Self
    }
}

impl LanguagePack for TsLanguagePack {
    fn name(&self) -> &'static str {
        "ts_pack"
    }

    fn version(&self) -> &'static str {
        "0.2.0"
    }

    fn can_handle(&self, path: &str, _sample_content: &[u8]) -> bool {
        path.ends_with(".ts")
            || path.ends_with(".tsx")
            || path.ends_with(".js")
            || path.ends_with(".jsx")
            || path.ends_with(".mjs")
            || path.ends_with(".cjs")
    }

    fn extract(&self, snapshot: &FileSnapshot) -> Result<ExtractedFacts, ExtractionError> {
        let mut parser = Parser::new();
        let language = if snapshot.path.ends_with(".tsx") || snapshot.path.ends_with(".jsx") {
            tree_sitter_typescript::LANGUAGE_TSX.into()
        } else {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
        };

        parser
            .set_language(&language)
            .map_err(|e| ExtractionError::ParseFailure(e.to_string()))?;

        let tree = parser.parse(&snapshot.content, None).ok_or_else(|| {
            ExtractionError::ParseFailure("failed to parse typescript source".to_string())
        })?;

        let line_index = LineIndex::build(&snapshot.content);
        let mut builder = FactBuilder::new(
            &snapshot.root,
            &snapshot.path,
            "typescript",
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

impl TsLanguagePack {
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
            "function_declaration" => {
                if let Some(name_node) = ts_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let mut attrs = Attributes::default();

                    if let Some(doc) = Self::extract_jsdoc(ts_node, source) {
                        attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }

                    let qualified = if container_chain.is_empty() {
                        Some(name.to_string())
                    } else {
                        Some(format!("{}::{}", container_chain.join("::"), name))
                    };

                    let fn_claim = builder.make_node(
                        NodeKind::Function,
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
            "class_declaration" => {
                if let Some(name_node) = ts_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let mut attrs = Attributes::default();

                    if let Some(doc) = Self::extract_jsdoc(ts_node, source) {
                        attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }

                    let qualified = if container_chain.is_empty() {
                        Some(name.to_string())
                    } else {
                        Some(format!("{}::{}", container_chain.join("::"), name))
                    };

                    let class_claim = builder.make_node(
                        NodeKind::Class,
                        name,
                        qualified,
                        container_chain,
                        ts_node,
                        attrs,
                    );
                    let class_id = class_claim.node.id;
                    let class_range = class_claim.node.range;
                    facts.nodes.push(class_claim);

                    facts.edges.push(builder.make_edge(
                        current_parent_id,
                        class_id,
                        EdgeKind::Contains,
                        class_range,
                        Attributes::default(),
                    ));

                    // Recurse into class body
                    container_chain.push(format!("class {name}"));
                    parent_id_stack.push(class_id);

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
            "interface_declaration" => {
                if let Some(name_node) = ts_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let mut attrs = Attributes::default();

                    if let Some(doc) = Self::extract_jsdoc(ts_node, source) {
                        attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }

                    let iface_claim = builder.make_node(
                        NodeKind::Interface,
                        name,
                        Some(name.to_string()),
                        container_chain,
                        ts_node,
                        attrs,
                    );
                    let iface_id = iface_claim.node.id;
                    let iface_range = iface_claim.node.range;
                    facts.nodes.push(iface_claim);

                    facts.edges.push(builder.make_edge(
                        current_parent_id,
                        iface_id,
                        EdgeKind::Contains,
                        iface_range,
                        Attributes::default(),
                    ));
                }
            }
            "type_alias_declaration" => {
                if let Some(name_node) = ts_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let type_claim = builder.make_node(
                        NodeKind::Type,
                        name,
                        Some(name.to_string()),
                        container_chain,
                        ts_node,
                        Attributes::default(),
                    );
                    let type_id = type_claim.node.id;
                    let type_range = type_claim.node.range;
                    facts.nodes.push(type_claim);

                    facts.edges.push(builder.make_edge(
                        current_parent_id,
                        type_id,
                        EdgeKind::Contains,
                        type_range,
                        Attributes::default(),
                    ));
                }
            }
            "enum_declaration" => {
                if let Some(name_node) = ts_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let enum_claim = builder.make_node(
                        NodeKind::Enum,
                        name,
                        Some(name.to_string()),
                        container_chain,
                        ts_node,
                        Attributes::default(),
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
            "method_definition" => {
                if let Some(name_node) = ts_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let mut attrs = Attributes::default();

                    if let Some(doc) = Self::extract_jsdoc(ts_node, source) {
                        attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }

                    let qualified = if container_chain.is_empty() {
                        Some(name.to_string())
                    } else {
                        Some(format!("{}::{}", container_chain.join("::"), name))
                    };

                    let method_claim = builder.make_node(
                        NodeKind::Method,
                        name,
                        qualified,
                        container_chain,
                        ts_node,
                        attrs,
                    );
                    let method_id = method_claim.node.id;
                    let method_range = method_claim.node.range;
                    facts.nodes.push(method_claim);

                    facts.edges.push(builder.make_edge(
                        current_parent_id,
                        method_id,
                        EdgeKind::Contains,
                        method_range,
                        Attributes::default(),
                    ));
                }
            }
            "variable_declarator" => {
                if let Some(name_node) = ts_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    if let Some(value_node) = ts_node.child_by_field_name("value")
                        && (value_node.kind() == "arrow_function"
                            || value_node.kind() == "function_expression")
                    {
                        let mut attrs = Attributes::default();
                        if let Some(doc) = Self::extract_jsdoc(ts_node, source) {
                            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                        }

                        let fn_claim = builder.make_node(
                            NodeKind::Function,
                            name,
                            Some(name.to_string()),
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
            }
            "import_statement" => {
                let import_text = node_text(ts_node, source);
                let source_path = if let Some(source_node) = ts_node.child_by_field_name("source") {
                    node_text(&source_node, source)
                        .trim_matches('\'')
                        .trim_matches('"')
                        .to_string()
                } else {
                    "".to_string()
                };

                let seeking = source_path
                    .split('/')
                    .next_back()
                    .unwrap_or(&source_path)
                    .trim_end_matches(".js")
                    .trim_end_matches(".ts")
                    .to_string();

                if !seeking.is_empty() {
                    facts.unresolved.push(UnresolvedRef {
                        from: current_parent_id,
                        seeking,
                        scope_hint: Some(import_text.to_string()),
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

    fn extract_jsdoc(ts_node: &TsNode<'_>, source: &[u8]) -> Option<String> {
        let target = if let Some(parent) = ts_node.parent() {
            if parent.kind() == "export_statement" {
                parent
            } else {
                *ts_node
            }
        } else {
            *ts_node
        };

        let prev = target.prev_sibling();
        if let Some(sibling) = prev
            && sibling.kind() == "comment"
        {
            let text = node_text(&sibling, source).trim();
            if text.starts_with("/**") {
                let cleaned = text
                    .trim_start_matches("/**")
                    .trim_end_matches("*/")
                    .lines()
                    .map(|l| l.trim().trim_start_matches('*').trim())
                    .filter(|l| !l.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                return Some(cleaned);
            }
        }
        None
    }
}
