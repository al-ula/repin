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

pub const CSHARP_PACK_VERSION: &str = "0.2.0";

#[derive(Debug, Default)]
pub struct CSharpLanguagePack;

impl CSharpLanguagePack {
    pub fn new() -> Self {
        Self
    }
}

impl LanguagePack for CSharpLanguagePack {
    fn name(&self) -> &'static str {
        "csharp_pack"
    }

    fn version(&self) -> &'static str {
        CSHARP_PACK_VERSION
    }

    fn can_handle(&self, path: &str, _sample_content: &[u8]) -> bool {
        path.ends_with(".cs")
    }

    fn extract(&self, snapshot: &FileSnapshot) -> Result<ExtractedFacts, ExtractionError> {
        let mut parser = Parser::new();
        let language = tree_sitter_c_sharp::LANGUAGE.into();

        parser
            .set_language(&language)
            .map_err(|e| ExtractionError::ParseFailure(e.to_string()))?;

        let tree = parser.parse(&snapshot.content, None).ok_or_else(|| {
            ExtractionError::ParseFailure("failed to parse csharp source".to_string())
        })?;

        let line_index = LineIndex::build(&snapshot.content);
        let mut builder = FactBuilder::new(
            &snapshot.root,
            &snapshot.path,
            "csharp",
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

impl CSharpLanguagePack {
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
            "file_scoped_namespace_declaration" | "namespace_declaration" => {
                Self::process_namespace(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                );
            }
            "using_directive" => {
                Self::process_using_directive(ts_node, source, builder, current_parent_id, facts);
            }
            "class_declaration" => {
                Self::process_type_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                    NodeKind::Class,
                );
            }
            "interface_declaration" => {
                Self::process_type_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                    NodeKind::Interface,
                );
            }
            "struct_declaration" | "record_declaration" | "record_struct_declaration" => {
                Self::process_type_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                    NodeKind::Struct,
                );
            }
            "enum_declaration" => {
                Self::process_enum_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    current_parent_id,
                    facts,
                );
            }
            "delegate_declaration" => {
                Self::process_delegate_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    current_parent_id,
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
            "destructor_declaration" => {
                Self::process_destructor_declaration(
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
            "property_declaration" | "indexer_declaration" => {
                Self::process_property_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    current_parent_id,
                    facts,
                );
            }
            "field_declaration" | "event_field_declaration" | "event_declaration" => {
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

    fn process_namespace(
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
        let ns_name = node_text(&name_node, source).trim().to_string();
        if ns_name.is_empty() {
            return;
        }

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        let qualified = if container_chain.is_empty() {
            Some(ns_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), ns_name))
        };

        let ns_claim = builder.make_node(
            NodeKind::Namespace,
            &ns_name,
            qualified,
            container_chain,
            ts_node,
            attrs,
        );
        let ns_id = ns_claim.node.id;
        let ns_range = ns_claim.node.range;
        facts.nodes.push(ns_claim);

        facts.edges.push(builder.make_edge(
            current_parent_id,
            ns_id,
            EdgeKind::Contains,
            ns_range,
            Attributes::default(),
        ));

        let is_file_scoped = ts_node.kind() == "file_scoped_namespace_declaration";

        if is_file_scoped {
            container_chain.push(ns_name);
            parent_id_stack.push(ns_id);

            let mut cursor = ts_node.walk();
            for child in ts_node.children(&mut cursor) {
                if child.kind() != "name" && child.kind() != ";" {
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
        } else if let Some(body_node) = ts_node.child_by_field_name("body") {
            container_chain.push(ns_name);
            parent_id_stack.push(ns_id);

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

    fn process_using_directive(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &FactBuilder<'_>,
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let text = node_text(ts_node, source).trim();
        let cleaned = text
            .trim_start_matches("using static ")
            .trim_start_matches("using ")
            .trim_end_matches(';')
            .trim();

        let seeking = if let Some((_, right)) = cleaned.split_once('=') {
            right
                .trim()
                .rsplit('.')
                .next()
                .unwrap_or(right.trim())
                .to_string()
        } else {
            cleaned.rsplit('.').next().unwrap_or(cleaned).to_string()
        };

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

    #[allow(clippy::too_many_arguments)]
    fn process_type_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
        node_kind: NodeKind,
    ) {
        let current_parent_id = *parent_id_stack.last().unwrap();
        let name_node = match ts_node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let type_name = node_text(&name_node, source).trim().to_string();
        if type_name.is_empty() {
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
            Some(type_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), type_name))
        };

        let claim = builder.make_node(
            node_kind,
            &type_name,
            qualified,
            container_chain,
            ts_node,
            attrs,
        );
        let type_id = claim.node.id;
        let type_range = claim.node.range;
        facts.nodes.push(claim);

        facts.edges.push(builder.make_edge(
            current_parent_id,
            type_id,
            EdgeKind::Contains,
            type_range,
            Attributes::default(),
        ));

        // Base types (extends/implements)
        let mut base_list = ts_node.child_by_field_name("base_list");
        if base_list.is_none() {
            let mut c = ts_node.walk();
            for child in ts_node.children(&mut c) {
                if child.kind() == "base_list" {
                    base_list = Some(child);
                    break;
                }
            }
        }

        if let Some(base_list_node) = base_list {
            let mut cursor = base_list_node.walk();
            for child in base_list_node.children(&mut cursor) {
                if child.kind() == "identifier"
                    || child.kind() == "generic_name"
                    || child.kind() == "qualified_name"
                {
                    let text = node_text(&child, source).trim();
                    let clean = text
                        .split('<')
                        .next()
                        .unwrap_or(text)
                        .rsplit('.')
                        .next()
                        .unwrap_or(text)
                        .trim();

                    if !clean.is_empty() {
                        facts.unresolved.push(UnresolvedRef {
                            from: type_id,
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

        // Primary constructor / record parameter list
        let mut params_node = ts_node.child_by_field_name("parameters");
        if params_node.is_none() {
            let mut c = ts_node.walk();
            for child in ts_node.children(&mut c) {
                if child.kind() == "parameter_list" {
                    params_node = Some(child);
                    break;
                }
            }
        }

        if let Some(p_node) = params_node {
            let mut cursor = p_node.walk();
            for param in p_node.children(&mut cursor) {
                if param.kind() == "parameter"
                    && let Some(p_name_node) = param.child_by_field_name("name")
                {
                    let p_name = node_text(&p_name_node, source).trim();
                    let type_text = param
                        .child_by_field_name("type")
                        .map(|t| node_text(&t, source).trim())
                        .unwrap_or("");

                    let mut p_attrs = Attributes::default();
                    if !type_text.is_empty() {
                        p_attrs.insert("type".to_string(), serde_json::json!(type_text));
                    }

                    let p_qualified = Some(format!("{type_name}::{p_name}"));
                    let p_claim = builder.make_node(
                        NodeKind::Field,
                        p_name,
                        p_qualified,
                        std::slice::from_ref(&type_name),
                        &param,
                        p_attrs,
                    );
                    let p_id = p_claim.node.id;
                    let p_range = p_claim.node.range;
                    facts.nodes.push(p_claim);

                    facts.edges.push(builder.make_edge(
                        type_id,
                        p_id,
                        EdgeKind::Contains,
                        p_range,
                        Attributes::default(),
                    ));
                }
            }
        }

        let mut body_node = ts_node.child_by_field_name("body");
        if body_node.is_none() {
            let mut c = ts_node.walk();
            for child in ts_node.children(&mut c) {
                if child.kind() == "declaration_list" {
                    body_node = Some(child);
                    break;
                }
            }
        }

        if let Some(body) = body_node {
            container_chain.push(type_name);
            parent_id_stack.push(type_id);

            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
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
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
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

        let mut body_node = ts_node.child_by_field_name("body");
        if body_node.is_none() {
            let mut c = ts_node.walk();
            for child in ts_node.children(&mut c) {
                if child.kind() == "enum_member_declaration_list" {
                    body_node = Some(child);
                    break;
                }
            }
        }

        if let Some(body) = body_node {
            let mut cursor = body.walk();
            for child in body.children(&mut cursor) {
                if child.kind() == "enum_member_declaration"
                    && let Some(m_name_node) = child.child_by_field_name("name")
                {
                    let m_name = node_text(&m_name_node, source).trim();
                    let mut m_attrs = Attributes::default();
                    if let Some(doc) = Self::extract_doc_comment(&child, source) {
                        m_attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }

                    let val_qualified = Some(format!("{enum_name}::{m_name}"));
                    let val_claim = builder.make_node(
                        NodeKind::Constant,
                        m_name,
                        val_qualified,
                        std::slice::from_ref(&enum_name),
                        &child,
                        m_attrs,
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
            }
        }
    }

    fn process_delegate_declaration(
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
        let del_name = node_text(&name_node, source).trim().to_string();
        if del_name.is_empty() {
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
            Some(del_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), del_name))
        };

        let claim = builder.make_node(
            NodeKind::Type,
            &del_name,
            qualified,
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

    fn process_destructor_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let name_node = ts_node.child_by_field_name("name");
        let name_text = name_node
            .map(|n| node_text(&n, source).trim().to_string())
            .unwrap_or_else(|| "Destructor".to_string());

        let dtor_name = if name_text.starts_with('~') {
            name_text
        } else {
            format!("~{name_text}")
        };

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        let qualified = if container_chain.is_empty() {
            Some(dtor_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), dtor_name))
        };

        let dtor_claim = builder.make_node(
            NodeKind::Method,
            &dtor_name,
            qualified,
            container_chain,
            ts_node,
            attrs,
        );
        let dtor_id = dtor_claim.node.id;
        let dtor_range = dtor_claim.node.range;
        facts.nodes.push(dtor_claim);

        facts.edges.push(builder.make_edge(
            current_parent_id,
            dtor_id,
            EdgeKind::Contains,
            dtor_range,
            Attributes::default(),
        ));

        if let Some(body_node) = ts_node.child_by_field_name("body") {
            Self::traverse_body_references(&body_node, source, builder, dtor_id, facts);
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

        if let Some(type_node) = ts_node
            .child_by_field_name("returns")
            .or_else(|| ts_node.child_by_field_name("type"))
        {
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

    fn process_property_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let name_node = ts_node.child_by_field_name("name");
        let prop_name = name_node
            .map(|n| node_text(&n, source).trim().to_string())
            .unwrap_or_else(|| "this[]".to_string());

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
                "type".to_string(),
                serde_json::json!(node_text(&type_node, source).trim()),
            );
        }

        let qualified = if container_chain.is_empty() {
            Some(prop_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), prop_name))
        };

        let prop_claim = builder.make_node(
            NodeKind::Property,
            &prop_name,
            qualified,
            container_chain,
            ts_node,
            attrs,
        );
        let prop_id = prop_claim.node.id;
        let prop_range = prop_claim.node.range;
        facts.nodes.push(prop_claim);

        facts.edges.push(builder.make_edge(
            current_parent_id,
            prop_id,
            EdgeKind::Contains,
            prop_range,
            Attributes::default(),
        ));
    }

    fn process_field_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let modifiers = Self::extract_modifiers(ts_node, source);
        let is_constant = if let Some(m) = &modifiers {
            m.contains("const")
        } else {
            false
        };

        let visibility = modifiers.as_deref().and_then(Self::extract_visibility);

        let mut type_text = "";
        let mut field_names = Vec::new();

        let mut cursor = ts_node.walk();
        for child in ts_node.children(&mut cursor) {
            if child.kind() == "variable_declaration" {
                if let Some(t_node) = child.child_by_field_name("type") {
                    type_text = node_text(&t_node, source).trim();
                }
                let mut v_cursor = child.walk();
                for sub in child.children(&mut v_cursor) {
                    if sub.kind() == "variable_declarator" {
                        let name_node = sub.child_by_field_name("name").or_else(|| {
                            let mut sc = sub.walk();
                            sub.children(&mut sc).find(|c| c.kind() == "identifier")
                        });
                        if let Some(name_node) = name_node {
                            field_names
                                .push((node_text(&name_node, source).trim().to_string(), sub));
                        }
                    }
                }
            } else if child.kind() == "variable_declarator" {
                let name_node = child.child_by_field_name("name").or_else(|| {
                    let mut sc = child.walk();
                    child.children(&mut sc).find(|c| c.kind() == "identifier")
                });
                if let Some(name_node) = name_node {
                    field_names.push((node_text(&name_node, source).trim().to_string(), child));
                }
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

            let claim =
                builder.make_node(kind, &name, qualified, container_chain, &item_node, attrs);
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
        let mut mods = Vec::new();
        let mut cursor = ts_node.walk();
        for child in ts_node.children(&mut cursor) {
            if child.kind() == "modifier" || child.kind() == "modifiers" {
                let text = node_text(&child, source).trim();
                if !text.is_empty() {
                    mods.push(text.to_string());
                }
            }
        }
        if mods.is_empty() {
            None
        } else {
            Some(mods.join(" "))
        }
    }

    fn extract_visibility(modifiers: &str) -> Option<&'static str> {
        if modifiers.contains("public") {
            Some("public")
        } else if modifiers.contains("protected") {
            Some("protected")
        } else if modifiers.contains("internal") {
            Some("internal")
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
            if kind == "comment" || kind == "single_line_doc_comment" {
                let text = node_text(&sibling, source).trim();
                if let Some(stripped) = text.strip_prefix("///") {
                    let cleaned = Self::clean_xml_doc(stripped.trim());
                    if !cleaned.is_empty() {
                        doc_lines.push(cleaned);
                    }
                } else if let Some(stripped) = text.strip_prefix("//") {
                    doc_lines.push(stripped.trim().to_string());
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
            } else if kind == "modifier" || kind == "modifiers" || kind == "attribute_list" {
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

    fn clean_xml_doc(text: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;

        for ch in text.chars() {
            if ch == '<' {
                in_tag = true;
            } else if ch == '>' {
                in_tag = false;
            } else if !in_tag {
                result.push(ch);
            }
        }

        result.trim().to_string()
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
                "invocation_expression" => {
                    let func_node = child
                        .child_by_field_name("function")
                        .or_else(|| child.child_by_field_name("expression"))
                        .or_else(|| child.child(0));

                    if let Some(f_node) = func_node {
                        let f_text = node_text(&f_node, source).trim();
                        let seeking = if f_node.kind() == "member_access_expression" {
                            if let Some(name_node) = f_node.child_by_field_name("name") {
                                node_text(&name_node, source).trim().to_string()
                            } else {
                                f_text.rsplit('.').next().unwrap_or(f_text).to_string()
                            }
                        } else {
                            f_text.rsplit('.').next().unwrap_or(f_text).to_string()
                        };

                        if !seeking.is_empty() {
                            facts.unresolved.push(UnresolvedRef {
                                from: from_id,
                                seeking,
                                scope_hint: Some(f_text.to_string()),
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

                        if !simple.is_empty() && !is_builtin_csharp_type(simple) {
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
                "type_identifier" | "generic_name" => {
                    let type_name = node_text(&child, source).trim();
                    let clean = type_name.split('<').next().unwrap_or(type_name).trim();
                    if !clean.is_empty() && !is_builtin_csharp_type(clean) {
                        facts.unresolved.push(UnresolvedRef {
                            from: from_id,
                            seeking: clean.to_string(),
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

fn is_builtin_csharp_type(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "byte"
            | "sbyte"
            | "char"
            | "decimal"
            | "double"
            | "float"
            | "int"
            | "uint"
            | "nint"
            | "nuint"
            | "long"
            | "ulong"
            | "short"
            | "ushort"
            | "object"
            | "string"
            | "void"
            | "var"
            | "dynamic"
            | "Boolean"
            | "Byte"
            | "Int16"
            | "Int32"
            | "Int64"
            | "UInt16"
            | "UInt32"
            | "UInt64"
            | "Single"
            | "Double"
            | "Decimal"
            | "String"
            | "Object"
            | "Task"
            | "ValueTask"
    )
}
