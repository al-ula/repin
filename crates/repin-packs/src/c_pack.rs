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

pub const C_PACK_VERSION: &str = "0.2.0";

#[derive(Debug, Default)]
pub struct CLanguagePack;

impl CLanguagePack {
    pub fn new() -> Self {
        Self
    }
}

impl LanguagePack for CLanguagePack {
    fn name(&self) -> &'static str {
        "c_pack"
    }

    fn version(&self) -> &'static str {
        C_PACK_VERSION
    }

    fn can_handle(&self, path: &str, _sample_content: &[u8]) -> bool {
        path.ends_with(".c") || path.ends_with(".h")
    }

    fn extract(&self, snapshot: &FileSnapshot) -> Result<ExtractedFacts, ExtractionError> {
        let mut parser = Parser::new();
        let language = tree_sitter_c::LANGUAGE.into();

        parser
            .set_language(&language)
            .map_err(|e| ExtractionError::ParseFailure(e.to_string()))?;

        let tree = parser.parse(&snapshot.content, None).ok_or_else(|| {
            ExtractionError::ParseFailure("failed to parse c source".to_string())
        })?;

        let line_index = LineIndex::build(&snapshot.content);
        let mut builder = FactBuilder::new(
            &snapshot.root,
            &snapshot.path,
            "c",
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

impl CLanguagePack {
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
            "preproc_include" => {
                Self::process_include(ts_node, source, builder, current_parent_id, facts);
            }
            "preproc_def" => {
                Self::process_preproc_def(ts_node, source, builder, container_chain, current_parent_id, facts);
            }
            "preproc_function_def" => {
                Self::process_preproc_function_def(ts_node, source, builder, container_chain, current_parent_id, facts);
            }
            "function_definition" => {
                Self::process_function_definition(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    current_parent_id,
                    facts,
                );
            }
            "declaration" => {
                Self::process_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                );
            }
            "type_definition" => {
                Self::process_type_definition(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                );
            }
            "struct_specifier" => {
                Self::process_struct_specifier(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                );
            }
            "union_specifier" => {
                Self::process_union_specifier(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                );
            }
            "enum_specifier" => {
                Self::process_enum_specifier(
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

    fn process_include(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &FactBuilder<'_>,
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let include_text = node_text(ts_node, source).trim();
        let path_node = ts_node.child_by_field_name("path");
        let path_text = path_node
            .map(|p| node_text(&p, source).trim())
            .unwrap_or("");

        let clean_path = path_text
            .trim_start_matches('<')
            .trim_end_matches('>')
            .trim_start_matches('"')
            .trim_end_matches('"')
            .trim();

        let seeking = clean_path
            .rsplit('/')
            .next()
            .unwrap_or(clean_path)
            .trim_end_matches(".h")
            .to_string();

        if !seeking.is_empty() {
            facts.unresolved.push(UnresolvedRef {
                from: current_parent_id,
                seeking,
                scope_hint: Some(include_text.to_string()),
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

    fn process_preproc_def(
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
        let name = node_text(&name_node, source).trim();
        if name.is_empty() {
            return;
        }

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        if let Some(val_node) = ts_node.child_by_field_name("value") {
            attrs.insert(
                "value".to_string(),
                serde_json::json!(node_text(&val_node, source).trim()),
            );
        }

        let qualified = if container_chain.is_empty() {
            Some(name.to_string())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), name))
        };

        let claim = builder.make_node(
            NodeKind::Constant,
            name,
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

    fn process_preproc_function_def(
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
        let name = node_text(&name_node, source).trim();
        if name.is_empty() {
            return;
        }

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        if let Some(params_node) = ts_node.child_by_field_name("parameters") {
            attrs.insert(
                "parameters".to_string(),
                serde_json::json!(node_text(&params_node, source).trim()),
            );
        }

        let qualified = if container_chain.is_empty() {
            Some(name.to_string())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), name))
        };

        let claim = builder.make_node(
            NodeKind::Function,
            name,
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

    fn process_function_definition(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let declarator = match ts_node.child_by_field_name("declarator") {
            Some(d) => d,
            None => return,
        };

        let (fn_name, params_text) = Self::unwrap_function_declarator(&declarator, source);
        if fn_name.is_empty() {
            return;
        }

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        if let Some(type_node) = ts_node.child_by_field_name("type") {
            attrs.insert(
                "return_type".to_string(),
                serde_json::json!(node_text(&type_node, source).trim()),
            );
        }

        if !params_text.is_empty() {
            attrs.insert("parameters".to_string(), serde_json::json!(params_text));
        }

        let qualified = if container_chain.is_empty() {
            Some(fn_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), fn_name))
        };

        let fn_claim = builder.make_node(
            NodeKind::Function,
            &fn_name,
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

        if let Some(body_node) = ts_node.child_by_field_name("body") {
            Self::traverse_body_references(&body_node, source, builder, fn_id, facts);
        }
    }

    fn unwrap_function_declarator(node: &TsNode<'_>, source: &[u8]) -> (String, String) {
        let mut curr = *node;
        let mut params_str = String::new();

        loop {
            match curr.kind() {
                "function_declarator" => {
                    if let Some(p) = curr.child_by_field_name("parameters") {
                        params_str = node_text(&p, source).trim().to_string();
                    }
                    if let Some(inner) = curr.child_by_field_name("declarator") {
                        curr = inner;
                    } else {
                        break;
                    }
                }
                "pointer_declarator" | "parenthesized_declarator" | "attributed_declarator" => {
                    if let Some(inner) = curr.child_by_field_name("declarator") {
                        curr = inner;
                    } else {
                        let mut found = None;
                        let mut cursor = curr.walk();
                        for child in curr.children(&mut cursor) {
                            if child.kind().ends_with("declarator") || child.kind() == "identifier" {
                                found = Some(child);
                                break;
                            }
                        }
                        if let Some(f) = found {
                            curr = f;
                        } else {
                            break;
                        }
                    }
                }
                "identifier" => {
                    return (node_text(&curr, source).trim().to_string(), params_str);
                }
                _ => {
                    let mut cursor = curr.walk();
                    for child in curr.children(&mut cursor) {
                        if child.kind() == "identifier" {
                            return (node_text(&child, source).trim().to_string(), params_str);
                        }
                    }
                    break;
                }
            }
        }

        (node_text(&curr, source).trim().to_string(), params_str)
    }

    fn process_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
    ) {
        let current_parent_id = *parent_id_stack.last().unwrap();
        let mut has_nested_type = false;

        let mut cursor = ts_node.walk();
        for child in ts_node.children(&mut cursor) {
            match child.kind() {
                "struct_specifier" => {
                    has_nested_type = true;
                    Self::process_struct_specifier(
                        &child,
                        source,
                        builder,
                        container_chain,
                        parent_id_stack,
                        facts,
                    );
                }
                "union_specifier" => {
                    has_nested_type = true;
                    Self::process_union_specifier(
                        &child,
                        source,
                        builder,
                        container_chain,
                        parent_id_stack,
                        facts,
                    );
                }
                "enum_specifier" => {
                    has_nested_type = true;
                    Self::process_enum_specifier(
                        &child,
                        source,
                        builder,
                        container_chain,
                        current_parent_id,
                        facts,
                    );
                }
                _ => {}
            }
        }

        if has_nested_type {
            return;
        }

        // Global variable / constant
        let decl_text = node_text(ts_node, source);
        let is_const = decl_text.contains("const ");
        let type_text = ts_node
            .child_by_field_name("type")
            .map(|t| node_text(&t, source).trim())
            .unwrap_or("");

        let mut declarators = Vec::new();
        let mut cursor = ts_node.walk();
        for child in ts_node.children(&mut cursor) {
            if child.kind() == "init_declarator" {
                if let Some(decl) = child.child_by_field_name("declarator") {
                    let name = Self::extract_declarator_name(&decl, source);
                    if !name.is_empty() {
                        declarators.push((name, child));
                    }
                }
            } else if child.kind() == "identifier" {
                declarators.push((node_text(&child, source).trim().to_string(), child));
            }
        }

        for (var_name, var_node) in declarators {
            let mut attrs = Attributes::default();
            if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
                attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
            }
            if !type_text.is_empty() {
                attrs.insert("type".to_string(), serde_json::json!(type_text));
            }

            let kind = if is_const {
                NodeKind::Constant
            } else {
                NodeKind::Variable
            };

            let qualified = if container_chain.is_empty() {
                Some(var_name.clone())
            } else {
                Some(format!("{}::{}", container_chain.join("::"), var_name))
            };

            let claim = builder.make_node(
                kind,
                &var_name,
                qualified,
                container_chain,
                &var_node,
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

    fn extract_declarator_name(node: &TsNode<'_>, source: &[u8]) -> String {
        let mut curr = *node;
        loop {
            match curr.kind() {
                "identifier" => return node_text(&curr, source).trim().to_string(),
                "pointer_declarator" | "array_declarator" | "parenthesized_declarator" => {
                    if let Some(inner) = curr.child_by_field_name("declarator") {
                        curr = inner;
                    } else {
                        break;
                    }
                }
                _ => {
                    let mut cursor = curr.walk();
                    for child in curr.children(&mut cursor) {
                        if child.kind() == "identifier" {
                            return node_text(&child, source).trim().to_string();
                        }
                    }
                    break;
                }
            }
        }
        node_text(&curr, source).trim().to_string()
    }

    fn process_type_definition(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
    ) {
        let current_parent_id = *parent_id_stack.last().unwrap();
        let mut type_name = String::new();
        let mut typedef_type_node = None;

        let mut cursor = ts_node.walk();
        for child in ts_node.children(&mut cursor) {
            if child.kind() == "type_identifier" {
                type_name = node_text(&child, source).trim().to_string();
            } else if child.kind() == "struct_specifier"
                || child.kind() == "union_specifier"
                || child.kind() == "enum_specifier"
            {
                typedef_type_node = Some(child);
            }
        }

        if let Some(nested_spec) = typedef_type_node {
            match nested_spec.kind() {
                "struct_specifier" => {
                    Self::process_struct_specifier(
                        &nested_spec,
                        source,
                        builder,
                        container_chain,
                        parent_id_stack,
                        facts,
                    );
                }
                "union_specifier" => {
                    Self::process_union_specifier(
                        &nested_spec,
                        source,
                        builder,
                        container_chain,
                        parent_id_stack,
                        facts,
                    );
                }
                "enum_specifier" => {
                    Self::process_enum_specifier(
                        &nested_spec,
                        source,
                        builder,
                        container_chain,
                        current_parent_id,
                        facts,
                    );
                }
                _ => {}
            }
        }

        if type_name.is_empty()
            && let Some(declarator) = ts_node.child_by_field_name("declarator")
        {
            type_name = Self::extract_declarator_name(&declarator, source);
        }

        if !type_name.is_empty() {
            let mut attrs = Attributes::default();
            if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
                attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
            }
            if let Some(type_node) = ts_node.child_by_field_name("type") {
                attrs.insert(
                    "alias_target".to_string(),
                    serde_json::json!(node_text(&type_node, source).trim()),
                );
            }

            let qualified = if container_chain.is_empty() {
                Some(type_name.clone())
            } else {
                Some(format!("{}::{}", container_chain.join("::"), type_name))
            };

            let claim = builder.make_node(
                NodeKind::Type,
                &type_name,
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
    }

    fn process_struct_specifier(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
    ) {
        let current_parent_id = *parent_id_stack.last().unwrap();
        let name = ts_node
            .child_by_field_name("name")
            .map(|n| node_text(&n, source).trim().to_string())
            .unwrap_or_default();

        let body = ts_node.child_by_field_name("body");
        if body.is_none() && name.is_empty() {
            return;
        }

        let struct_name = if name.is_empty() {
            "AnonymousStruct".to_string()
        } else {
            name
        };

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        let qualified = if container_chain.is_empty() {
            Some(struct_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), struct_name))
        };

        let struct_claim = builder.make_node(
            NodeKind::Struct,
            &struct_name,
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

        if let Some(body_node) = body {
            container_chain.push(struct_name);
            parent_id_stack.push(struct_id);

            Self::process_field_declarations(&body_node, source, builder, container_chain, struct_id, facts);

            parent_id_stack.pop();
            container_chain.pop();
        }
    }

    fn process_union_specifier(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
    ) {
        let current_parent_id = *parent_id_stack.last().unwrap();
        let name = ts_node
            .child_by_field_name("name")
            .map(|n| node_text(&n, source).trim().to_string())
            .unwrap_or_default();

        let body = ts_node.child_by_field_name("body");
        if body.is_none() && name.is_empty() {
            return;
        }

        let union_name = if name.is_empty() {
            "AnonymousUnion".to_string()
        } else {
            name
        };

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }

        let qualified = if container_chain.is_empty() {
            Some(union_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), union_name))
        };

        let union_claim = builder.make_node(
            NodeKind::Struct,
            &union_name,
            qualified,
            container_chain,
            ts_node,
            attrs,
        );
        let union_id = union_claim.node.id;
        let union_range = union_claim.node.range;
        facts.nodes.push(union_claim);

        facts.edges.push(builder.make_edge(
            current_parent_id,
            union_id,
            EdgeKind::Contains,
            union_range,
            Attributes::default(),
        ));

        if let Some(body_node) = body {
            container_chain.push(union_name);
            parent_id_stack.push(union_id);

            Self::process_field_declarations(&body_node, source, builder, container_chain, union_id, facts);

            parent_id_stack.pop();
            container_chain.pop();
        }
    }

    fn process_field_declarations(
        field_list_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let mut cursor = field_list_node.walk();
        for field in field_list_node.children(&mut cursor) {
            if field.kind() == "field_declaration" {
                let type_text = field
                    .child_by_field_name("type")
                    .map(|t| node_text(&t, source).trim())
                    .unwrap_or("");

                let mut field_names = Vec::new();
                let mut field_cursor = field.walk();
                for sub in field.children(&mut field_cursor) {
                    if sub.kind() == "field_identifier" {
                        field_names.push((node_text(&sub, source).trim().to_string(), sub));
                    } else if sub.kind().ends_with("declarator") {
                        let name = Self::extract_declarator_name(&sub, source);
                        if !name.is_empty() {
                            field_names.push((name, sub));
                        }
                    }
                }

                for (field_name, field_node) in field_names {
                    let mut attrs = Attributes::default();
                    if let Some(doc) = Self::extract_doc_comment(&field, source) {
                        attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }
                    if !type_text.is_empty() {
                        attrs.insert("type".to_string(), serde_json::json!(type_text));
                    }

                    let qualified = Some(format!(
                        "{}::{}",
                        container_chain.join("::"),
                        field_name
                    ));

                    let field_claim = builder.make_node(
                        NodeKind::Field,
                        &field_name,
                        qualified,
                        container_chain,
                        &field_node,
                        attrs,
                    );
                    let field_id = field_claim.node.id;
                    let field_range = field_claim.node.range;
                    facts.nodes.push(field_claim);

                    facts.edges.push(builder.make_edge(
                        parent_id,
                        field_id,
                        EdgeKind::Contains,
                        field_range,
                        Attributes::default(),
                    ));
                }
            }
        }
    }

    fn process_enum_specifier(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let name = ts_node
            .child_by_field_name("name")
            .map(|n| node_text(&n, source).trim().to_string())
            .unwrap_or_default();

        let body = ts_node.child_by_field_name("body");
        if body.is_none() && name.is_empty() {
            return;
        }

        let enum_name = if name.is_empty() {
            "AnonymousEnum".to_string()
        } else {
            name
        };

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
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

        if let Some(body_node) = body {
            let mut cursor = body_node.walk();
            for child in body_node.children(&mut cursor) {
                if child.kind() == "enumerator"
                    && let Some(enumerator_name_node) = child.child_by_field_name("name")
                {
                    let enum_val_name = node_text(&enumerator_name_node, source).trim();
                    let mut enum_val_attrs = Attributes::default();
                    if let Some(doc) = Self::extract_doc_comment(&child, source) {
                        enum_val_attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }
                    if let Some(val_node) = child.child_by_field_name("value") {
                        enum_val_attrs.insert(
                            "value".to_string(),
                            serde_json::json!(node_text(&val_node, source).trim()),
                        );
                    }

                    let val_qualified = Some(format!("{enum_name}::{enum_val_name}"));
                    let val_claim = builder.make_node(
                        NodeKind::Constant,
                        enum_val_name,
                        val_qualified,
                        std::slice::from_ref(&enum_name),
                        &child,
                        enum_val_attrs,
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

    fn extract_doc_comment(ts_node: &TsNode<'_>, source: &[u8]) -> Option<String> {
        let mut prev = ts_node.prev_sibling();
        let mut doc_lines = Vec::new();

        while let Some(sibling) = prev {
            if sibling.kind() == "comment" {
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
                        let fn_name = match func_node.kind() {
                            "identifier" => node_text(&func_node, source).trim().to_string(),
                            "field_expression" => {
                                func_node
                                    .child_by_field_name("field")
                                    .map(|f| node_text(&f, source).trim().to_string())
                                    .unwrap_or_default()
                            }
                            _ => String::new(),
                        };

                        if !fn_name.is_empty() {
                            facts.unresolved.push(UnresolvedRef {
                                from: from_id,
                                seeking: fn_name.clone(),
                                scope_hint: Some(fn_name),
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
                "type_identifier" => {
                    let type_name = node_text(&child, source).trim();
                    if !type_name.is_empty() {
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
