use super::extractor_util::{FactBuilder, node_text};
use repin_core::line_index::LineIndex;
use repin_core::model::identity::NodeId;
use repin_core::model::node::Attributes;
use repin_core::model::provenance::{Confidence, Derivation, Provenance, Revision};
use repin_core::model::registries::{EdgeKind, NodeKind};
use repin_core::model::unresolved::UnresolvedRef;
use repin_core::ports::fs::FileSnapshot;
use repin_core::ports::pack::{ExtractedFacts, ExtractionError, LanguagePack};
use tree_sitter::{Node as TsNode, Parser};

pub const JAVA_PACK_VERSION: &str = "0.2.0";

#[derive(Debug, Default)]
pub struct JavaLanguagePack;

impl JavaLanguagePack {
    pub fn new() -> Self {
        Self
    }
}

impl LanguagePack for JavaLanguagePack {
    fn name(&self) -> &'static str {
        "java_pack"
    }

    fn version(&self) -> &'static str {
        JAVA_PACK_VERSION
    }

    fn can_handle(&self, path: &str, _sample_content: &[u8]) -> bool {
        path.ends_with(".java")
    }

    fn extract(&self, snapshot: &FileSnapshot) -> Result<ExtractedFacts, ExtractionError> {
        let mut parser = Parser::new();
        let language = tree_sitter_java::LANGUAGE.into();

        parser
            .set_language(&language)
            .map_err(|e| ExtractionError::ParseFailure(e.to_string()))?;

        let tree = parser.parse(&snapshot.content, None).ok_or_else(|| {
            ExtractionError::ParseFailure("failed to parse java source".to_string())
        })?;

        let line_index = LineIndex::build(&snapshot.content);
        let mut builder = FactBuilder::new(
            &snapshot.root,
            &snapshot.path,
            "java",
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

impl JavaLanguagePack {
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
            "package_declaration" => {
                Self::process_package_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    current_parent_id,
                    facts,
                );
            }
            "import_declaration" => {
                Self::process_import_declaration(ts_node, source, builder, current_parent_id, facts);
            }
            "class_declaration" => {
                Self::process_class_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                );
            }
            "interface_declaration" => {
                Self::process_interface_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                );
            }
            "enum_declaration" => {
                Self::process_enum_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                );
            }
            "record_declaration" => {
                Self::process_record_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                );
            }
            "annotation_type_declaration" => {
                Self::process_annotation_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                );
            }
            "constructor_declaration" => {
                Self::process_constructor_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    current_parent_id,
                    facts,
                );
            }
            "method_declaration" => {
                Self::process_method_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    current_parent_id,
                    facts,
                );
            }
            "field_declaration" | "constant_declaration" => {
                Self::process_field_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    current_parent_id,
                    facts,
                );
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

    fn process_package_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let mut pkg_name = String::new();
        let mut cursor = ts_node.walk();
        for child in ts_node.children(&mut cursor) {
            if child.kind() == "scoped_identifier" || child.kind() == "identifier" {
                pkg_name = node_text(&child, source).trim().to_string();
                break;
            }
        }

        if pkg_name.is_empty() {
            return;
        }

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        let claim = builder.make_node(
            NodeKind::Package,
            &pkg_name,
            Some(pkg_name.clone()),
            container_chain,
            ts_node,
            attrs,
        );
        let id = claim.node.id;
        let range = claim.node.range;
        facts.nodes.push(claim);

        facts.edges.push(builder.make_edge(
            current_parent_id,
            id,
            EdgeKind::Contains,
            range,
            Attributes::default(),
        ));
    }

    fn process_import_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &FactBuilder<'_>,
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let text = node_text(ts_node, source).trim();
        let cleaned = text
            .trim_start_matches("import static ")
            .trim_start_matches("import ")
            .trim_end_matches(';')
            .trim();

        let seeking = cleaned.rsplit('.').next().unwrap_or(cleaned).to_string();
        if !seeking.is_empty() {
            facts.unresolved.push(UnresolvedRef {
                from: current_parent_id,
                seeking,
                scope_hint: Some(text.to_string()),
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

    fn process_class_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
    ) {
        let current_parent_id = *parent_id_stack.last().unwrap();
        let name_node = match ts_node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let class_name = node_text(&name_node, source).trim().to_string();
        if class_name.is_empty() {
            return;
        }

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        if let Some(modifiers) = Self::extract_modifiers(ts_node, source) {
            attrs.insert("modifiers".to_string(), serde_json::json!(modifiers));
            if let Some(vis) = Self::extract_visibility(&modifiers) {
                attrs.insert("visibility".to_string(), serde_json::json!(vis));
            }
        }

        let qualified = if container_chain.is_empty() {
            Some(class_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), class_name))
        };

        let class_claim = builder.make_node(
            NodeKind::Class,
            &class_name,
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

        // Superclass (extends)
        if let Some(superclass_node) = ts_node.child_by_field_name("superclass") {
            let mut cursor = superclass_node.walk();
            for child in superclass_node.children(&mut cursor) {
                if child.kind() == "type_identifier" || child.kind() == "generic_type" {
                    let text = node_text(&child, source).trim();
                    let clean = text.split('<').next().unwrap_or(text).trim();
                    if !clean.is_empty() {
                        facts.unresolved.push(UnresolvedRef {
                            from: class_id,
                            seeking: clean.to_string(),
                            scope_hint: Some(text.to_string()),
                            edge_kind: EdgeKind::Extends,
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
            }
        }

        // Interfaces (implements)
        if let Some(interfaces_node) = ts_node.child_by_field_name("interfaces") {
            let mut cursor = interfaces_node.walk();
            for child in interfaces_node.children(&mut cursor) {
                if child.kind() == "type_list" {
                    let mut list_cursor = child.walk();
                    for iface in child.children(&mut list_cursor) {
                        if iface.kind() == "type_identifier" || iface.kind() == "generic_type" {
                            let text = node_text(&iface, source).trim();
                            let clean = text.split('<').next().unwrap_or(text).trim();
                            if !clean.is_empty() {
                                facts.unresolved.push(UnresolvedRef {
                                    from: class_id,
                                    seeking: clean.to_string(),
                                    scope_hint: Some(text.to_string()),
                                    edge_kind: EdgeKind::Implements,
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
                    }
                }
            }
        }

        if let Some(body_node) = ts_node.child_by_field_name("body") {
            container_chain.push(class_name);
            parent_id_stack.push(class_id);

            let mut cursor = body_node.walk();
            for child in body_node.children(&mut cursor) {
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

    fn process_interface_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
    ) {
        let current_parent_id = *parent_id_stack.last().unwrap();
        let name_node = match ts_node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let iface_name = node_text(&name_node, source).trim().to_string();
        if iface_name.is_empty() {
            return;
        }

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        if let Some(modifiers) = Self::extract_modifiers(ts_node, source) {
            attrs.insert("modifiers".to_string(), serde_json::json!(modifiers));
            if let Some(vis) = Self::extract_visibility(&modifiers) {
                attrs.insert("visibility".to_string(), serde_json::json!(vis));
            }
        }

        let qualified = if container_chain.is_empty() {
            Some(iface_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), iface_name))
        };

        let iface_claim = builder.make_node(
            NodeKind::Interface,
            &iface_name,
            qualified,
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

        // Extended interfaces
        let mut extends_node = ts_node.child_by_field_name("extends_interfaces")
            .or_else(|| ts_node.child_by_field_name("extends"));
        if extends_node.is_none() {
            let mut c = ts_node.walk();
            for child in ts_node.children(&mut c) {
                if child.kind() == "extends_interfaces" {
                    extends_node = Some(child);
                    break;
                }
            }
        }

        if let Some(extends_node) = extends_node {
            let mut cursor = extends_node.walk();
            for child in extends_node.children(&mut cursor) {
                if child.kind() == "type_list" {
                    let mut list_cursor = child.walk();
                    for iface in child.children(&mut list_cursor) {
                        if iface.kind() == "type_identifier" || iface.kind() == "generic_type" {
                            let text = node_text(&iface, source).trim();
                            let clean = text.split('<').next().unwrap_or(text).trim();
                            if !clean.is_empty() {
                                facts.unresolved.push(UnresolvedRef {
                                    from: iface_id,
                                    seeking: clean.to_string(),
                                    scope_hint: Some(text.to_string()),
                                    edge_kind: EdgeKind::Extends,
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
                    }
                } else if child.kind() == "type_identifier" || child.kind() == "generic_type" {
                    let text = node_text(&child, source).trim();
                    let clean = text.split('<').next().unwrap_or(text).trim();
                    if !clean.is_empty() {
                        facts.unresolved.push(UnresolvedRef {
                            from: iface_id,
                            seeking: clean.to_string(),
                            scope_hint: Some(text.to_string()),
                            edge_kind: EdgeKind::Extends,
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
            }
        }

        if let Some(body_node) = ts_node.child_by_field_name("body") {
            container_chain.push(iface_name);
            parent_id_stack.push(iface_id);

            let mut cursor = body_node.walk();
            for child in body_node.children(&mut cursor) {
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

    fn process_enum_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
    ) {
        let current_parent_id = *parent_id_stack.last().unwrap();
        let name_node = match ts_node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let enum_name = node_text(&name_node, source).trim().to_string();
        if enum_name.is_empty() {
            return;
        }

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        if let Some(modifiers) = Self::extract_modifiers(ts_node, source) {
            attrs.insert("modifiers".to_string(), serde_json::json!(modifiers));
            if let Some(vis) = Self::extract_visibility(&modifiers) {
                attrs.insert("visibility".to_string(), serde_json::json!(vis));
            }
        }

        let qualified = if container_chain.is_empty() {
            Some(enum_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), enum_name))
        };

        let enum_claim = builder.make_node(
            NodeKind::Enum,
            &enum_name,
            qualified,
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

        if let Some(body_node) = ts_node.child_by_field_name("body") {
            container_chain.push(enum_name.clone());
            parent_id_stack.push(enum_id);

            let mut cursor = body_node.walk();
            for child in body_node.children(&mut cursor) {
                if child.kind() == "enum_constant" {
                    if let Some(c_name_node) = child.child_by_field_name("name") {
                        let c_name = node_text(&c_name_node, source).trim();
                        let mut c_attrs = Attributes::default();
                        if let Some(doc) = Self::extract_doc_comment(&child, source) {
                            c_attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                        }

                        let val_qualified = Some(format!("{enum_name}::{c_name}"));
                        let val_claim = builder.make_node(
                            NodeKind::Constant,
                            c_name,
                            val_qualified,
                            std::slice::from_ref(&enum_name),
                            &child,
                            c_attrs,
                        );
                        let val_id = val_claim.node.id;
                        let val_range = val_claim.node.range;
                        facts.nodes.push(val_claim);

                        facts.edges.push(builder.make_edge(
                            enum_id,
                            val_id,
                            EdgeKind::Contains,
                            val_range,
                            Attributes::default(),
                        ));
                    }
                } else {
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

            parent_id_stack.pop();
            container_chain.pop();
        }
    }

    fn process_record_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
    ) {
        let current_parent_id = *parent_id_stack.last().unwrap();
        let name_node = match ts_node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let record_name = node_text(&name_node, source).trim().to_string();
        if record_name.is_empty() {
            return;
        }

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        if let Some(modifiers) = Self::extract_modifiers(ts_node, source) {
            attrs.insert("modifiers".to_string(), serde_json::json!(modifiers));
            if let Some(vis) = Self::extract_visibility(&modifiers) {
                attrs.insert("visibility".to_string(), serde_json::json!(vis));
            }
        }

        if let Some(parameters_node) = ts_node.child_by_field_name("parameters") {
            attrs.insert(
                "parameters".to_string(),
                serde_json::json!(node_text(&parameters_node, source).trim()),
            );
        }

        let qualified = if container_chain.is_empty() {
            Some(record_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), record_name))
        };

        let record_claim = builder.make_node(
            NodeKind::Struct,
            &record_name,
            qualified,
            container_chain,
            ts_node,
            attrs,
        );
        let record_id = record_claim.node.id;
        let record_range = record_claim.node.range;
        facts.nodes.push(record_claim);

        facts.edges.push(builder.make_edge(
            current_parent_id,
            record_id,
            EdgeKind::Contains,
            record_range,
            Attributes::default(),
        ));

        // Process record components (fields)
        let mut params_node = ts_node.child_by_field_name("parameters");
        if params_node.is_none() {
            let mut c = ts_node.walk();
            for child in ts_node.children(&mut c) {
                if child.kind() == "record_header" || child.kind() == "formal_parameters" {
                    params_node = Some(child);
                    break;
                }
            }
        }
        if let Some(params_node) = params_node {
            let mut cursor = params_node.walk();
            for comp in params_node.children(&mut cursor) {
                if comp.kind() == "record_component" || comp.kind() == "formal_parameter" {
                    let comp_name_node = comp.child_by_field_name("name").or_else(|| {
                        let mut sub_c = comp.walk();
                        comp.children(&mut sub_c).find(|c| c.kind() == "identifier")
                    });

                    if let Some(comp_name_node) = comp_name_node {
                        let comp_name = node_text(&comp_name_node, source).trim();
                        let type_text = comp
                            .child_by_field_name("type")
                            .map(|t| node_text(&t, source).trim())
                            .unwrap_or("");

                        let mut comp_attrs = Attributes::default();
                        if !type_text.is_empty() {
                            comp_attrs.insert("type".to_string(), serde_json::json!(type_text));
                        }

                        let comp_qualified = Some(format!("{record_name}::{comp_name}"));
                        let comp_claim = builder.make_node(
                            NodeKind::Field,
                            comp_name,
                            comp_qualified,
                            std::slice::from_ref(&record_name),
                            &comp,
                            comp_attrs,
                        );
                        let comp_id = comp_claim.node.id;
                        let comp_range = comp_claim.node.range;
                        facts.nodes.push(comp_claim);

                        facts.edges.push(builder.make_edge(
                            record_id,
                            comp_id,
                            EdgeKind::Contains,
                            comp_range,
                            Attributes::default(),
                        ));
                    }
                }
            }
        }

        if let Some(body_node) = ts_node.child_by_field_name("body") {
            container_chain.push(record_name);
            parent_id_stack.push(record_id);

            let mut cursor = body_node.walk();
            for child in body_node.children(&mut cursor) {
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

    fn process_annotation_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
    ) {
        let current_parent_id = *parent_id_stack.last().unwrap();
        let name_node = match ts_node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let anno_name = node_text(&name_node, source).trim().to_string();
        if anno_name.is_empty() {
            return;
        }

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        if let Some(modifiers) = Self::extract_modifiers(ts_node, source) {
            attrs.insert("modifiers".to_string(), serde_json::json!(modifiers));
            if let Some(vis) = Self::extract_visibility(&modifiers) {
                attrs.insert("visibility".to_string(), serde_json::json!(vis));
            }
        }

        let qualified = if container_chain.is_empty() {
            Some(anno_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), anno_name))
        };

        let anno_claim = builder.make_node(
            NodeKind::Interface,
            &anno_name,
            qualified,
            container_chain,
            ts_node,
            attrs,
        );
        let anno_id = anno_claim.node.id;
        let anno_range = anno_claim.node.range;
        facts.nodes.push(anno_claim);

        facts.edges.push(builder.make_edge(
            current_parent_id,
            anno_id,
            EdgeKind::Contains,
            anno_range,
            Attributes::default(),
        ));

        if let Some(body_node) = ts_node.child_by_field_name("body") {
            container_chain.push(anno_name);
            parent_id_stack.push(anno_id);

            let mut cursor = body_node.walk();
            for child in body_node.children(&mut cursor) {
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

    fn process_constructor_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let name_node = match ts_node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let ctor_name = node_text(&name_node, source).trim().to_string();
        if ctor_name.is_empty() {
            return;
        }

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        if let Some(modifiers) = Self::extract_modifiers(ts_node, source) {
            attrs.insert("modifiers".to_string(), serde_json::json!(modifiers));
            if let Some(vis) = Self::extract_visibility(&modifiers) {
                attrs.insert("visibility".to_string(), serde_json::json!(vis));
            }
        }

        if let Some(parameters_node) = ts_node.child_by_field_name("parameters") {
            attrs.insert(
                "parameters".to_string(),
                serde_json::json!(node_text(&parameters_node, source).trim()),
            );
        }

        let qualified = if container_chain.is_empty() {
            Some(ctor_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), ctor_name))
        };

        let ctor_claim = builder.make_node(
            NodeKind::Constructor,
            &ctor_name,
            qualified,
            container_chain,
            ts_node,
            attrs,
        );
        let ctor_id = ctor_claim.node.id;
        let ctor_range = ctor_claim.node.range;
        facts.nodes.push(ctor_claim);

        facts.edges.push(builder.make_edge(
            current_parent_id,
            ctor_id,
            EdgeKind::Contains,
            ctor_range,
            Attributes::default(),
        ));

        if let Some(body_node) = ts_node.child_by_field_name("body") {
            Self::traverse_body_references(&body_node, source, builder, ctor_id, facts);
        }
    }

    fn process_method_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let name_node = match ts_node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let method_name = node_text(&name_node, source).trim().to_string();
        if method_name.is_empty() {
            return;
        }

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        if let Some(modifiers) = Self::extract_modifiers(ts_node, source) {
            attrs.insert("modifiers".to_string(), serde_json::json!(modifiers));
            if let Some(vis) = Self::extract_visibility(&modifiers) {
                attrs.insert("visibility".to_string(), serde_json::json!(vis));
            }
        }

        if let Some(type_node) = ts_node.child_by_field_name("type") {
            attrs.insert(
                "return_type".to_string(),
                serde_json::json!(node_text(&type_node, source).trim()),
            );
        }

        if let Some(parameters_node) = ts_node.child_by_field_name("parameters") {
            attrs.insert(
                "parameters".to_string(),
                serde_json::json!(node_text(&parameters_node, source).trim()),
            );
        }

        let qualified = if container_chain.is_empty() {
            Some(method_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), method_name))
        };

        let method_claim = builder.make_node(
            NodeKind::Method,
            &method_name,
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

        if let Some(body_node) = ts_node.child_by_field_name("body") {
            Self::traverse_body_references(&body_node, source, builder, method_id, facts);
        }
    }

    fn process_field_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let type_text = ts_node
            .child_by_field_name("type")
            .map(|t| node_text(&t, source).trim())
            .unwrap_or("");

        let modifiers = Self::extract_modifiers(ts_node, source);
        let is_constant = if let Some(m) = &modifiers {
            m.contains("static") && m.contains("final")
        } else {
            false
        };

        let visibility = modifiers.as_deref().and_then(Self::extract_visibility);

        let mut field_names = Vec::new();
        let mut cursor = ts_node.walk();
        for child in ts_node.children(&mut cursor) {
            if child.kind() == "variable_declarator"
                && let Some(name_node) = child.child_by_field_name("name")
            {
                field_names.push((node_text(&name_node, source).trim().to_string(), child));
            }
        }

        let kind = if is_constant {
            NodeKind::Constant
        } else {
            NodeKind::Field
        };

        for (name, item_node) in field_names {
            let mut attrs = Attributes::default();
            if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
                attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
            }
            if !type_text.is_empty() {
                attrs.insert("type".to_string(), serde_json::json!(type_text));
            }
            if let Some(m) = &modifiers {
                attrs.insert("modifiers".to_string(), serde_json::json!(m));
            }
            if let Some(v) = visibility {
                attrs.insert("visibility".to_string(), serde_json::json!(v));
            }

            let qualified = if container_chain.is_empty() {
                Some(name.clone())
            } else {
                Some(format!("{}::{}", container_chain.join("::"), name))
            };

            let claim = builder.make_node(
                kind,
                &name,
                qualified,
                container_chain,
                &item_node,
                attrs,
            );
            let id = claim.node.id;
            let range = claim.node.range;
            facts.nodes.push(claim);

            facts.edges.push(builder.make_edge(
                current_parent_id,
                id,
                EdgeKind::Contains,
                range,
                Attributes::default(),
            ));
        }
    }

    fn extract_modifiers(ts_node: &TsNode<'_>, source: &[u8]) -> Option<String> {
        let mut cursor = ts_node.walk();
        for child in ts_node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let text = node_text(&child, source).trim();
                if !text.is_empty() {
                    return Some(text.to_string());
                }
            }
        }
        None
    }

    fn extract_visibility(modifiers: &str) -> Option<&'static str> {
        if modifiers.contains("public") {
            Some("public")
        } else if modifiers.contains("protected") {
            Some("protected")
        } else if modifiers.contains("private") {
            Some("private")
        } else {
            None
        }
    }

    fn extract_doc_comment(ts_node: &TsNode<'_>, source: &[u8]) -> Option<String> {
        let mut prev = ts_node.prev_sibling();
        let mut doc_lines = Vec::new();

        while let Some(sibling) = prev {
            let kind = sibling.kind();
            if kind == "block_comment" || kind == "line_comment" {
                let text = node_text(&sibling, source).trim();
                if let Some(stripped) = text.strip_prefix("//") {
                    let cleaned = stripped.trim_start_matches('/').trim();
                    doc_lines.push(cleaned.to_string());
                } else if text.starts_with("/*") && text.ends_with("*/") {
                    let inside = &text[2..text.len() - 2];
                    for line in inside.lines() {
                        let cleaned = line.trim().trim_start_matches('*').trim();
                        if !cleaned.is_empty() {
                            doc_lines.push(cleaned.to_string());
                        }
                    }
                }
                prev = sibling.prev_sibling();
            } else if kind == "modifiers" {
                // If comments are before modifiers, step before modifiers
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

    fn traverse_body_references(
        node: &TsNode<'_>,
        source: &[u8],
        builder: &FactBuilder<'_>,
        from_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let kind = child.kind();
            match kind {
                "method_invocation" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        let method_name = node_text(&name_node, source).trim().to_string();
                        let object_text = child
                            .child_by_field_name("object")
                            .map(|o| node_text(&o, source).trim().to_string())
                            .unwrap_or_default();

                        if !method_name.is_empty() {
                            let scope_hint = if object_text.is_empty() {
                                Some(method_name.clone())
                            } else {
                                Some(format!("{object_text}.{method_name}"))
                            };

                            facts.unresolved.push(UnresolvedRef {
                                from: from_id,
                                seeking: method_name,
                                scope_hint,
                                edge_kind: EdgeKind::Calls,
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
                    Self::traverse_body_references(&child, source, builder, from_id, facts);
                }
                "object_creation_expression" => {
                    if let Some(type_node) = child.child_by_field_name("type") {
                        let type_text = node_text(&type_node, source).trim();
                        let clean = type_text.split('<').next().unwrap_or(type_text).trim();
                        let simple = clean.rsplit('.').next().unwrap_or(clean).trim();

                        if !simple.is_empty() && !is_builtin_java_type(simple) {
                            facts.unresolved.push(UnresolvedRef {
                                from: from_id,
                                seeking: simple.to_string(),
                                scope_hint: Some(format!("new {type_text}(...)")),
                                edge_kind: EdgeKind::Instantiates,
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
                    Self::traverse_body_references(&child, source, builder, from_id, facts);
                }
                "type_identifier" => {
                    let type_name = node_text(&child, source).trim();
                    if !type_name.is_empty() && !is_builtin_java_type(type_name) {
                        facts.unresolved.push(UnresolvedRef {
                            from: from_id,
                            seeking: type_name.to_string(),
                            scope_hint: None,
                            edge_kind: EdgeKind::HasType,
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
                    Self::traverse_body_references(&child, source, builder, from_id, facts);
                }
            }
        }
    }
}

fn is_builtin_java_type(name: &str) -> bool {
    matches!(
        name,
        "byte"
            | "short"
            | "int"
            | "long"
            | "float"
            | "double"
            | "boolean"
            | "char"
            | "void"
            | "String"
            | "Integer"
            | "Long"
            | "Double"
            | "Float"
            | "Boolean"
            | "Byte"
            | "Short"
            | "Character"
            | "Object"
            | "Class"
            | "Void"
            | "var"
    )
}
