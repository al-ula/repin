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

pub const GO_PACK_VERSION: &str = "0.2.0";

#[derive(Debug, Default)]
pub struct GoLanguagePack;

impl GoLanguagePack {
    pub fn new() -> Self {
        Self
    }
}

impl LanguagePack for GoLanguagePack {
    fn name(&self) -> &'static str {
        "go_pack"
    }

    fn version(&self) -> &'static str {
        GO_PACK_VERSION
    }

    fn can_handle(&self, path: &str, _sample_content: &[u8]) -> bool {
        path.ends_with(".go")
    }

    fn extract(&self, snapshot: &FileSnapshot) -> Result<ExtractedFacts, ExtractionError> {
        let mut parser = Parser::new();
        let language = tree_sitter_go::LANGUAGE.into();

        parser
            .set_language(&language)
            .map_err(|e| ExtractionError::ParseFailure(e.to_string()))?;

        let tree = parser.parse(&snapshot.content, None).ok_or_else(|| {
            ExtractionError::ParseFailure("failed to parse go source".to_string())
        })?;

        let line_index = LineIndex::build(&snapshot.content);
        let mut builder = FactBuilder::new(
            &snapshot.root,
            &snapshot.path,
            "go",
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

impl GoLanguagePack {
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
            "package_clause" => {
                let pkg_name = ts_node
                    .children(&mut ts_node.walk())
                    .find(|c| c.kind() == "package_identifier")
                    .map(|c| node_text(&c, source).trim())
                    .unwrap_or("");

                if !pkg_name.is_empty() {
                    let mut attrs = Attributes::default();
                    if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
                        attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }

                    let pkg_claim = builder.make_node(
                        NodeKind::Package,
                        pkg_name,
                        Some(pkg_name.to_string()),
                        container_chain,
                        ts_node,
                        attrs,
                    );
                    let pkg_id = pkg_claim.node.id;
                    let pkg_range = pkg_claim.node.range;
                    facts.nodes.push(pkg_claim);

                    facts.edges.push(builder.make_edge(
                        current_parent_id,
                        pkg_id,
                        EdgeKind::Contains,
                        pkg_range,
                        Attributes::default(),
                    ));
                }
            }
            "import_declaration" => {
                let mut cursor = ts_node.walk();
                for child in ts_node.children(&mut cursor) {
                    if child.kind() == "import_spec" {
                        Self::process_import_spec(&child, source, builder, current_parent_id, facts);
                    } else if child.kind() == "import_spec_list" {
                        let mut list_cursor = child.walk();
                        for item in child.children(&mut list_cursor) {
                            if item.kind() == "import_spec" {
                                Self::process_import_spec(
                                    &item,
                                    source,
                                    builder,
                                    current_parent_id,
                                    facts,
                                );
                            }
                        }
                    }
                }
            }
            "function_declaration" => {
                if let Some(name_node) = ts_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let mut attrs = Attributes::default();

                    if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
                        attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }

                    // Check signature / parameters
                    if let Some(params_node) = ts_node.child_by_field_name("parameters") {
                        attrs.insert(
                            "parameters".to_string(),
                            serde_json::json!(node_text(&params_node, source).trim()),
                        );
                    }
                    if let Some(res_node) = ts_node.child_by_field_name("result") {
                        attrs.insert(
                            "return_type".to_string(),
                            serde_json::json!(node_text(&res_node, source).trim()),
                        );
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

                    parent_id_stack.push(fn_id);
                    Self::traverse_body_references(
                        ts_node,
                        source,
                        builder,
                        fn_id,
                        facts,
                    );
                    parent_id_stack.pop();
                }
            }
            "method_declaration" => {
                if let Some(name_node) = ts_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let mut attrs = Attributes::default();

                    if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
                        attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }

                    // Extract receiver
                    let mut receiver_type = String::new();
                    if let Some(receiver_node) = ts_node.child_by_field_name("receiver") {
                        let rec_text = node_text(&receiver_node, source).trim();
                        attrs.insert("receiver".to_string(), serde_json::json!(rec_text));

                        // Find the type inside receiver, e.g. (s *Server) -> Server or *Server
                        let mut rec_cursor = receiver_node.walk();
                        for child in receiver_node.children(&mut rec_cursor) {
                            if child.kind() == "parameter_declaration" {
                                if let Some(type_node) = child.child_by_field_name("type") {
                                    receiver_type = node_text(&type_node, source)
                                        .trim()
                                        .trim_start_matches('*')
                                        .to_string();
                                }
                            }
                        }
                    }

                    if let Some(params_node) = ts_node.child_by_field_name("parameters") {
                        attrs.insert(
                            "parameters".to_string(),
                            serde_json::json!(node_text(&params_node, source).trim()),
                        );
                    }
                    if let Some(res_node) = ts_node.child_by_field_name("result") {
                        attrs.insert(
                            "return_type".to_string(),
                            serde_json::json!(node_text(&res_node, source).trim()),
                        );
                    }

                    let qualified = if !receiver_type.is_empty() {
                        Some(format!("{receiver_type}::{name}"))
                    } else if !container_chain.is_empty() {
                        Some(format!("{}::{}", container_chain.join("::"), name))
                    } else {
                        Some(name.to_string())
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

                    parent_id_stack.push(method_id);
                    Self::traverse_body_references(
                        ts_node,
                        source,
                        builder,
                        method_id,
                        facts,
                    );
                    parent_id_stack.pop();
                }
            }
            "type_declaration" => {
                let mut cursor = ts_node.walk();
                for child in ts_node.children(&mut cursor) {
                    if child.kind() == "type_spec" {
                        Self::process_type_spec(
                            ts_node,
                            &child,
                            source,
                            builder,
                            container_chain,
                            parent_id_stack,
                            facts,
                        );
                    } else if child.kind() == "type_alias" {
                        Self::process_type_alias(
                            ts_node,
                            &child,
                            source,
                            builder,
                            container_chain,
                            current_parent_id,
                            facts,
                        );
                    }
                }
            }
            "const_declaration" => {
                let mut cursor = ts_node.walk();
                for child in ts_node.children(&mut cursor) {
                    if child.kind() == "const_spec" {
                        Self::process_const_or_var_spec(
                            ts_node,
                            &child,
                            source,
                            builder,
                            container_chain,
                            current_parent_id,
                            NodeKind::Constant,
                            facts,
                        );
                    }
                }
            }
            "var_declaration" => {
                let mut cursor = ts_node.walk();
                for child in ts_node.children(&mut cursor) {
                    if child.kind() == "var_spec" {
                        Self::process_const_or_var_spec(
                            ts_node,
                            &child,
                            source,
                            builder,
                            container_chain,
                            current_parent_id,
                            NodeKind::Variable,
                            facts,
                        );
                    }
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

    fn process_import_spec(
        spec_node: &TsNode<'_>,
        source: &[u8],
        builder: &FactBuilder<'_>,
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let path_node = spec_node.child_by_field_name("path");
        let raw_path = if let Some(p) = path_node {
            node_text(&p, source).trim().trim_matches('"')
        } else {
            ""
        };

        if raw_path.is_empty() {
            return;
        }

        let alias_node = spec_node.child_by_field_name("name");
        let alias = alias_node.map(|a| node_text(&a, source).trim());

        let seeking = if let Some(a) = alias {
            if a == "." || a == "_" {
                raw_path.rsplit('/').next().unwrap_or(raw_path).to_string()
            } else {
                a.to_string()
            }
        } else {
            raw_path.rsplit('/').next().unwrap_or(raw_path).to_string()
        };

        let scope_hint = if let Some(a) = alias {
            format!("{a} \"{raw_path}\"")
        } else {
            format!("\"{raw_path}\"")
        };

        facts.unresolved.push(UnresolvedRef {
            from: current_parent_id,
            seeking,
            scope_hint: Some(scope_hint),
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

    fn process_type_spec(
        parent_decl: &TsNode<'_>,
        type_spec: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
    ) {
        let current_parent_id = *parent_id_stack.last().unwrap();
        let name_node = match type_spec.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let name = node_text(&name_node, source);
        let mut attrs = Attributes::default();

        // Doc comment on type_spec or parent_decl
        if let Some(doc) = Self::extract_doc_comment(type_spec, source)
            .or_else(|| Self::extract_doc_comment(parent_decl, source))
        {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        let type_body = type_spec.child_by_field_name("type");
        let body_kind = type_body.map(|b| b.kind()).unwrap_or("");

        let (node_kind, is_container) = match body_kind {
            "struct_type" => (NodeKind::Struct, true),
            "interface_type" => (NodeKind::Interface, true),
            _ => (NodeKind::Type, false),
        };

        let qualified = if container_chain.is_empty() {
            Some(name.to_string())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), name))
        };

        let type_claim = builder.make_node(
            node_kind,
            name,
            qualified,
            container_chain,
            type_spec,
            attrs,
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

        if is_container && let Some(body_node) = type_body {
            container_chain.push(name.to_string());
            parent_id_stack.push(type_id);

            if body_kind == "struct_type" {
                Self::process_struct_fields(&body_node, source, builder, container_chain, type_id, facts);
            } else if body_kind == "interface_type" {
                Self::process_interface_methods(&body_node, source, builder, container_chain, type_id, facts);
            }

            parent_id_stack.pop();
            container_chain.pop();
        }
    }

    fn process_type_alias(
        parent_decl: &TsNode<'_>,
        alias_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let name_node = match alias_node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let name = node_text(&name_node, source);
        let mut attrs = Attributes::default();

        if let Some(doc) = Self::extract_doc_comment(alias_node, source)
            .or_else(|| Self::extract_doc_comment(parent_decl, source))
        {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        if let Some(target) = alias_node.child_by_field_name("type") {
            attrs.insert(
                "alias_target".to_string(),
                serde_json::json!(node_text(&target, source).trim()),
            );
        }

        let qualified = if container_chain.is_empty() {
            Some(name.to_string())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), name))
        };

        let claim = builder.make_node(
            NodeKind::Type,
            name,
            qualified,
            container_chain,
            alias_node,
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

    fn process_struct_fields(
        struct_body: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        struct_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let mut cursor = struct_body.walk();
        for child in struct_body.children(&mut cursor) {
            if child.kind() == "field_declaration_list" {
                let mut field_cursor = child.walk();
                for field in child.children(&mut field_cursor) {
                    if field.kind() == "field_declaration" {
                        let mut attrs = Attributes::default();
                        if let Some(doc) = Self::extract_doc_comment(&field, source) {
                            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                        }
                        if let Some(tag_node) = field.child_by_field_name("tag") {
                            attrs.insert(
                                "tag".to_string(),
                                serde_json::json!(node_text(&tag_node, source).trim()),
                            );
                        }
                        if let Some(type_node) = field.child_by_field_name("type") {
                            attrs.insert(
                                "type".to_string(),
                                serde_json::json!(node_text(&type_node, source).trim()),
                            );
                        }

                        // Check if there are named fields (field_identifier) or embedded type
                        let mut names = Vec::new();
                        let mut sub_cursor = field.walk();
                        for sub in field.children(&mut sub_cursor) {
                            if sub.kind() == "field_identifier" {
                                names.push((node_text(&sub, source), sub));
                            }
                        }

                        // Embedded field (e.g. `sync.Mutex` or `MyEmbedded`)
                        if names.is_empty() && let Some(type_node) = field.child_by_field_name("type") {
                            let type_name = node_text(&type_node, source).trim();
                            let clean_name = type_name.rsplit('.').next().unwrap_or(type_name).trim_start_matches('*');
                            let qualified = Some(format!("{}::{}", container_chain.join("::"), clean_name));
                            let field_claim = builder.make_node(
                                NodeKind::Field,
                                clean_name,
                                qualified,
                                container_chain,
                                &field,
                                attrs.clone(),
                            );
                            let field_id = field_claim.node.id;
                            let field_range = field_claim.node.range;
                            facts.nodes.push(field_claim);

                            facts.edges.push(builder.make_edge(
                                struct_id,
                                field_id,
                                EdgeKind::Contains,
                                field_range,
                                Attributes::default(),
                            ));

                            if !clean_name.is_empty() && !is_builtin_type(clean_name) {
                                add_unresolved_ref(
                                    builder,
                                    struct_id,
                                    clean_name.to_string(),
                                    Some(format!("embed {type_name}")),
                                    EdgeKind::Extends,
                                    facts,
                                );
                            }
                        }

                        for (name, sub_node) in names {
                            let qualified = Some(format!("{}::{}", container_chain.join("::"), name));
                            let field_claim = builder.make_node(
                                NodeKind::Field,
                                name,
                                qualified,
                                container_chain,
                                &sub_node,
                                attrs.clone(),
                            );
                            let field_id = field_claim.node.id;
                            let field_range = field_claim.node.range;
                            facts.nodes.push(field_claim);

                            facts.edges.push(builder.make_edge(
                                struct_id,
                                field_id,
                                EdgeKind::Contains,
                                field_range,
                                Attributes::default(),
                            ));
                        }

                        // Record field type reference
                        if let Some(type_node) = field.child_by_field_name("type") {
                            let type_name = node_text(&type_node, source).trim();
                            let clean_type = type_name
                                .rsplit('.')
                                .next()
                                .unwrap_or(type_name)
                                .trim_start_matches('*')
                                .trim_start_matches("[]");
                            if !clean_type.is_empty() && !is_builtin_type(clean_type) {
                                add_unresolved_ref(
                                    builder,
                                    struct_id,
                                    clean_type.to_string(),
                                    None,
                                    EdgeKind::HasType,
                                    facts,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn process_interface_methods(
        iface_body: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        iface_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let mut cursor = iface_body.walk();
        for child in iface_body.children(&mut cursor) {
            if child.kind() == "method_spec" || child.kind() == "method_elem" {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let mut attrs = Attributes::default();

                    if let Some(doc) = Self::extract_doc_comment(&child, source) {
                        attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }
                    if let Some(params_node) = child.child_by_field_name("parameters") {
                        attrs.insert(
                            "parameters".to_string(),
                            serde_json::json!(node_text(&params_node, source).trim()),
                        );
                    }
                    if let Some(res_node) = child.child_by_field_name("result") {
                        attrs.insert(
                            "return_type".to_string(),
                            serde_json::json!(node_text(&res_node, source).trim()),
                        );
                    }

                    let qualified = Some(format!("{}::{}", container_chain.join("::"), name));
                    let method_claim = builder.make_node(
                        NodeKind::Method,
                        name,
                        qualified,
                        container_chain,
                        &child,
                        attrs,
                    );
                    let method_id = method_claim.node.id;
                    let method_range = method_claim.node.range;
                    facts.nodes.push(method_claim);

                    facts.edges.push(builder.make_edge(
                        iface_id,
                        method_id,
                        EdgeKind::Contains,
                        method_range,
                        Attributes::default(),
                    ));

                    Self::traverse_body_references(&child, source, builder, method_id, facts);
                }
            } else if child.kind() == "type_identifier" || child.kind() == "type_elem" {
                let iface_name = node_text(&child, source).trim();
                let clean_name = iface_name.rsplit('.').next().unwrap_or(iface_name);
                if !clean_name.is_empty() && !is_builtin_type(clean_name) {
                    add_unresolved_ref(
                        builder,
                        iface_id,
                        clean_name.to_string(),
                        Some(format!("embed {iface_name}")),
                        EdgeKind::Extends,
                        facts,
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_const_or_var_spec(
        parent_decl: &TsNode<'_>,
        spec_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        node_kind: NodeKind,
        facts: &mut ExtractedFacts,
    ) {
        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(spec_node, source)
            .or_else(|| Self::extract_doc_comment(parent_decl, source))
        {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        if let Some(type_node) = spec_node.child_by_field_name("type") {
            attrs.insert(
                "type".to_string(),
                serde_json::json!(node_text(&type_node, source).trim()),
            );
        }

        let mut cursor = spec_node.walk();
        for child in spec_node.children(&mut cursor) {
            if child.kind() == "identifier" {
                let name = node_text(&child, source);
                let qualified = if container_chain.is_empty() {
                    Some(name.to_string())
                } else {
                    Some(format!("{}::{}", container_chain.join("::"), name))
                };

                let claim = builder.make_node(
                    node_kind,
                    name,
                    qualified,
                    container_chain,
                    &child,
                    attrs.clone(),
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
    }

    fn extract_doc_comment(ts_node: &TsNode<'_>, source: &[u8]) -> Option<String> {
        let mut prev = ts_node.prev_sibling();
        let mut doc_lines = Vec::new();

        while let Some(sibling) = prev {
            if sibling.kind() == "comment" {
                let text = node_text(&sibling, source).trim();
                if let Some(stripped) = text.strip_prefix("//") {
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
                "call_expression" => {
                    if let Some(func_node) = child.child_by_field_name("function") {
                        match func_node.kind() {
                            "identifier" => {
                                let fn_name = node_text(&func_node, source).trim();
                                if !fn_name.is_empty() && !is_builtin_func(fn_name) {
                                    add_unresolved_ref(
                                        builder,
                                        from_id,
                                        fn_name.to_string(),
                                        Some(fn_name.to_string()),
                                        EdgeKind::Calls,
                                        facts,
                                    );
                                }
                            }
                            "selector_expression" => {
                                if let Some(field_node) = func_node.child_by_field_name("field") {
                                    let field_name = node_text(&field_node, source).trim();
                                    let operand_text = func_node
                                        .child_by_field_name("operand")
                                        .map(|op| node_text(&op, source).trim())
                                        .unwrap_or("");
                                    if !field_name.is_empty() {
                                        let scope_hint = if operand_text.is_empty() {
                                            Some(field_name.to_string())
                                        } else {
                                            Some(format!("{operand_text}.{field_name}"))
                                        };
                                        add_unresolved_ref(
                                            builder,
                                            from_id,
                                            field_name.to_string(),
                                            scope_hint,
                                            EdgeKind::Calls,
                                            facts,
                                        );
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Self::traverse_body_references(&child, source, builder, from_id, facts);
                }
                "composite_literal" => {
                    if let Some(type_node) = child.child_by_field_name("type") {
                        let type_text = node_text(&type_node, source).trim();
                        let clean_name = type_text
                            .rsplit('.')
                            .next()
                            .unwrap_or(type_text)
                            .trim_start_matches('&')
                            .trim_start_matches('*');
                        if !clean_name.is_empty() && !is_builtin_type(clean_name) {
                            add_unresolved_ref(
                                builder,
                                from_id,
                                clean_name.to_string(),
                                Some(format!("&{type_text}{{...}}")),
                                EdgeKind::Instantiates,
                                facts,
                            );
                        }
                    }
                    Self::traverse_body_references(&child, source, builder, from_id, facts);
                }
                "type_identifier" => {
                    let type_name = node_text(&child, source).trim();
                    if !type_name.is_empty() && !is_builtin_type(type_name) {
                        add_unresolved_ref(
                            builder,
                            from_id,
                            type_name.to_string(),
                            None,
                            EdgeKind::HasType,
                            facts,
                        );
                    }
                }
                _ => {
                    Self::traverse_body_references(&child, source, builder, from_id, facts);
                }
            }
        }
    }
}

fn is_builtin_type(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "byte"
            | "complex64"
            | "complex128"
            | "error"
            | "float32"
            | "float64"
            | "int"
            | "int8"
            | "int16"
            | "int32"
            | "int64"
            | "rune"
            | "string"
            | "uint"
            | "uint8"
            | "uint16"
            | "uint32"
            | "uint64"
            | "uintptr"
            | "any"
            | "comparable"
            | "nil"
            | "true"
            | "false"
            | "iota"
    )
}

fn is_builtin_func(name: &str) -> bool {
    matches!(
        name,
        "append"
            | "cap"
            | "clear"
            | "close"
            | "complex"
            | "copy"
            | "delete"
            | "imag"
            | "len"
            | "make"
            | "max"
            | "min"
            | "new"
            | "panic"
            | "print"
            | "println"
            | "real"
            | "recover"
    )
}

fn add_unresolved_ref(
    builder: &FactBuilder<'_>,
    from: NodeId,
    seeking: String,
    scope_hint: Option<String>,
    edge_kind: EdgeKind,
    facts: &mut ExtractedFacts,
) {
    if facts
        .unresolved
        .iter()
        .any(|u| u.from == from && u.seeking == seeking && u.edge_kind == edge_kind)
    {
        return;
    }

    facts.unresolved.push(UnresolvedRef {
        from,
        seeking,
        scope_hint,
        edge_kind,
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
