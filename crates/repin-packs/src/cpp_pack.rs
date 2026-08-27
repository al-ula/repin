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

pub const CPP_PACK_VERSION: &str = "0.2.0";

#[derive(Debug, Default)]
pub struct CppLanguagePack;

impl CppLanguagePack {
    pub fn new() -> Self {
        Self
    }
}

impl LanguagePack for CppLanguagePack {
    fn name(&self) -> &'static str {
        "cpp_pack"
    }

    fn version(&self) -> &'static str {
        CPP_PACK_VERSION
    }

    fn can_handle(&self, path: &str, sample_content: &[u8]) -> bool {
        if path.ends_with(".cpp")
            || path.ends_with(".cc")
            || path.ends_with(".cxx")
            || path.ends_with(".c++")
            || path.ends_with(".hpp")
            || path.ends_with(".hh")
            || path.ends_with(".hxx")
            || path.ends_with(".h++")
        {
            return true;
        }

        if path.ends_with(".h")
            && let Ok(text) = std::str::from_utf8(sample_content)
        {
            return text.contains("class ")
                || text.contains("namespace ")
                || text.contains("template<")
                || text.contains("template <")
                || text.contains("public:")
                || text.contains("private:")
                || text.contains("protected:")
                || text.contains("std::");
        }

        false
    }

    fn extract(&self, snapshot: &FileSnapshot) -> Result<ExtractedFacts, ExtractionError> {
        let mut parser = Parser::new();
        let language = tree_sitter_cpp::LANGUAGE.into();

        parser
            .set_language(&language)
            .map_err(|e| ExtractionError::ParseFailure(e.to_string()))?;

        let tree = parser.parse(&snapshot.content, None).ok_or_else(|| {
            ExtractionError::ParseFailure("failed to parse cpp source".to_string())
        })?;

        let line_index = LineIndex::build(&snapshot.content);
        let mut builder = FactBuilder::new(
            &snapshot.root,
            &snapshot.path,
            "cpp",
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
            None,
        );

        Ok(facts)
    }
}

impl CppLanguagePack {
    fn traverse_node(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
        current_access: Option<&str>,
    ) {
        let kind = ts_node.kind();
        let current_parent_id = *parent_id_stack.last().unwrap();

        match kind {
            "preproc_include" => {
                Self::process_include(ts_node, source, builder, current_parent_id, facts);
            }
            "preproc_def" => {
                Self::process_preproc_def(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    current_parent_id,
                    facts,
                );
            }
            "preproc_function_def" => {
                Self::process_preproc_function_def(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    current_parent_id,
                    facts,
                );
            }
            "using_declaration" => {
                Self::process_using_declaration(ts_node, source, builder, current_parent_id, facts);
            }
            "namespace_definition" => {
                Self::process_namespace(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                );
            }
            "class_specifier" => {
                Self::process_class_or_struct(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                    NodeKind::Class,
                    current_access,
                );
            }
            "struct_specifier" => {
                Self::process_class_or_struct(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                    NodeKind::Struct,
                    current_access,
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
                    current_access,
                );
            }
            "template_declaration" => {
                Self::process_template_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                    current_access,
                );
            }
            "alias_declaration" => {
                Self::process_alias_declaration(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    current_parent_id,
                    facts,
                    current_access,
                );
            }
            "type_definition" => {
                Self::process_type_definition(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    current_parent_id,
                    facts,
                );
            }
            "function_definition" => {
                Self::process_function_definition(
                    ts_node,
                    source,
                    builder,
                    container_chain,
                    current_parent_id,
                    facts,
                    current_access,
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
                    current_access,
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
                        current_access,
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
            .trim_end_matches(".hpp")
            .trim_end_matches(".hh")
            .trim_end_matches(".hxx")
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

    fn process_using_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &FactBuilder<'_>,
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let text = node_text(ts_node, source).trim();
        let cleaned = text
            .trim_start_matches("using namespace ")
            .trim_start_matches("using ")
            .trim_end_matches(';')
            .trim();

        let seeking = cleaned.rsplit("::").next().unwrap_or(cleaned).to_string();
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

    fn process_namespace(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
    ) {
        let current_parent_id = *parent_id_stack.last().unwrap();
        let name_node = ts_node.child_by_field_name("name");
        let ns_name = name_node
            .map(|n| node_text(&n, source).trim().to_string())
            .unwrap_or_else(|| "anonymous_namespace".to_string());

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

        if let Some(body_node) = ts_node.child_by_field_name("body") {
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
                    None,
                );
            }

            parent_id_stack.pop();
            container_chain.pop();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_class_or_struct(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
        default_kind: NodeKind,
        current_access: Option<&str>,
    ) {
        let current_parent_id = *parent_id_stack.last().unwrap();
        let name_node = ts_node.child_by_field_name("name");
        let raw_name = name_node
            .map(|n| node_text(&n, source).trim().to_string())
            .unwrap_or_default();

        let body = ts_node.child_by_field_name("body");
        if body.is_none() && raw_name.is_empty() {
            return;
        }

        let type_name = if raw_name.is_empty() {
            if default_kind == NodeKind::Class {
                "AnonymousClass".to_string()
            } else {
                "AnonymousStruct".to_string()
            }
        } else {
            raw_name
        };

        let mut attrs = Attributes::default();
        if let Some(doc) = Self::extract_doc_comment(ts_node, source) {
            attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
        }
        if let Some(acc) = current_access {
            attrs.insert("visibility".to_string(), serde_json::json!(acc));
        }

        let qualified = if container_chain.is_empty() {
            Some(type_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), type_name))
        };

        let claim = builder.make_node(
            default_kind,
            &type_name,
            qualified,
            container_chain,
            ts_node,
            attrs,
        );
        let class_id = claim.node.id;
        let class_range = claim.node.range;
        facts.nodes.push(claim);

        facts.edges.push(builder.make_edge(
            current_parent_id,
            class_id,
            EdgeKind::Contains,
            class_range,
            Attributes::default(),
        ));

        // Base class inheritance
        let mut cursor = ts_node.walk();
        for child in ts_node.children(&mut cursor) {
            if child.kind() == "base_class_clause" {
                let mut base_cursor = child.walk();
                for base in child.children(&mut base_cursor) {
                    if base.kind() == "type_identifier"
                        || base.kind() == "qualified_identifier"
                        || base.kind() == "template_type"
                    {
                        let base_text = node_text(&base, source).trim();
                        let clean_seeking = base_text
                            .split('<')
                            .next()
                            .unwrap_or(base_text)
                            .rsplit("::")
                            .next()
                            .unwrap_or(base_text)
                            .trim();

                        if !clean_seeking.is_empty() {
                            facts.unresolved.push(UnresolvedRef {
                                from: class_id,
                                seeking: clean_seeking.to_string(),
                                scope_hint: Some(base_text.to_string()),
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
        }

        if let Some(body_node) = body {
            container_chain.push(type_name);
            parent_id_stack.push(class_id);

            let mut active_access = if default_kind == NodeKind::Class {
                "private"
            } else {
                "public"
            };

            let mut body_cursor = body_node.walk();
            for member in body_node.children(&mut body_cursor) {
                if member.kind() == "access_specifier" {
                    let acc_text = node_text(&member, source).trim();
                    if acc_text.starts_with("public") {
                        active_access = "public";
                    } else if acc_text.starts_with("protected") {
                        active_access = "protected";
                    } else if acc_text.starts_with("private") {
                        active_access = "private";
                    }
                } else if member.kind() == "field_declaration" {
                    Self::process_field_declaration(
                        &member,
                        source,
                        builder,
                        container_chain,
                        class_id,
                        facts,
                        Some(active_access),
                    );
                } else {
                    Self::traverse_node(
                        &member,
                        source,
                        builder,
                        container_chain,
                        parent_id_stack,
                        facts,
                        Some(active_access),
                    );
                }
            }

            parent_id_stack.pop();
            container_chain.pop();
        }
    }

    fn process_field_declaration(
        field_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        parent_id: NodeId,
        facts: &mut ExtractedFacts,
        current_access: Option<&str>,
    ) {
        let type_text = field_node
            .child_by_field_name("type")
            .map(|t| node_text(&t, source).trim())
            .unwrap_or("");

        let mut field_names = Vec::new();
        let mut is_method = false;

        let mut cursor = field_node.walk();
        for child in field_node.children(&mut cursor) {
            if child.kind() == "field_identifier" {
                field_names.push((node_text(&child, source).trim().to_string(), child));
            } else if child.kind() == "function_declarator" {
                is_method = true;
                let (fn_name, _) = Self::unwrap_function_declarator(&child, source);
                if !fn_name.is_empty() {
                    field_names.push((fn_name, child));
                }
            } else if child.kind().ends_with("declarator") {
                let name = Self::extract_declarator_name(&child, source);
                if !name.is_empty() {
                    field_names.push((name, child));
                }
            }
        }

        let node_kind = if is_method {
            NodeKind::Method
        } else {
            NodeKind::Field
        };

        for (name, item_node) in field_names {
            let mut attrs = Attributes::default();
            if let Some(doc) = Self::extract_doc_comment(field_node, source) {
                attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
            }
            if !type_text.is_empty() {
                attrs.insert("type".to_string(), serde_json::json!(type_text));
            }
            if let Some(acc) = current_access {
                attrs.insert("visibility".to_string(), serde_json::json!(acc));
            }

            let qualified = Some(format!("{}::{}", container_chain.join("::"), name));

            let claim = builder.make_node(
                node_kind,
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
                parent_id,
                id,
                EdgeKind::Contains,
                range,
                Attributes::default(),
            ));
        }
    }

    fn process_function_definition(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
        current_access: Option<&str>,
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

        if let Some(acc) = current_access {
            attrs.insert("visibility".to_string(), serde_json::json!(acc));
        }

        let is_in_class = !container_chain.is_empty();
        let is_constructor = if let Some(last_container) = container_chain.last() {
            fn_name == *last_container
        } else {
            false
        };

        let is_destructor = fn_name.starts_with('~');

        let node_kind = if is_constructor {
            NodeKind::Constructor
        } else if is_in_class || fn_name.contains("::") || is_destructor {
            NodeKind::Method
        } else {
            NodeKind::Function
        };

        let simple_name = fn_name.rsplit("::").next().unwrap_or(&fn_name);
        let qualified = if container_chain.is_empty() {
            Some(fn_name.clone())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), simple_name))
        };

        let fn_claim = builder.make_node(
            node_kind,
            simple_name,
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
                "pointer_declarator"
                | "reference_declarator"
                | "parenthesized_declarator"
                | "attributed_declarator" => {
                    if let Some(inner) = curr.child_by_field_name("declarator") {
                        curr = inner;
                    } else {
                        let mut found = None;
                        let mut cursor = curr.walk();
                        for child in curr.children(&mut cursor) {
                            if child.kind().ends_with("declarator")
                                || child.kind() == "identifier"
                                || child.kind() == "qualified_identifier"
                                || child.kind() == "destructor_name"
                                || child.kind() == "operator_name"
                            {
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
                "identifier" | "qualified_identifier" | "destructor_name" | "operator_name" => {
                    return (node_text(&curr, source).trim().to_string(), params_str);
                }
                _ => {
                    let mut cursor = curr.walk();
                    for child in curr.children(&mut cursor) {
                        if child.kind() == "identifier"
                            || child.kind() == "qualified_identifier"
                            || child.kind() == "destructor_name"
                        {
                            return (node_text(&child, source).trim().to_string(), params_str);
                        }
                    }
                    break;
                }
            }
        }

        (node_text(&curr, source).trim().to_string(), params_str)
    }

    fn extract_declarator_name(node: &TsNode<'_>, source: &[u8]) -> String {
        let mut curr = *node;
        loop {
            match curr.kind() {
                "identifier" | "qualified_identifier" | "field_identifier" => {
                    return node_text(&curr, source).trim().to_string();
                }
                "pointer_declarator"
                | "reference_declarator"
                | "array_declarator"
                | "parenthesized_declarator" => {
                    if let Some(inner) = curr.child_by_field_name("declarator") {
                        curr = inner;
                    } else {
                        break;
                    }
                }
                _ => {
                    let mut cursor = curr.walk();
                    for child in curr.children(&mut cursor) {
                        if child.kind() == "identifier"
                            || child.kind() == "qualified_identifier"
                            || child.kind() == "field_identifier"
                        {
                            return node_text(&child, source).trim().to_string();
                        }
                    }
                    break;
                }
            }
        }
        node_text(&curr, source).trim().to_string()
    }

    fn process_template_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
        current_access: Option<&str>,
    ) {
        let mut cursor = ts_node.walk();
        for child in ts_node.children(&mut cursor) {
            if child.kind() != "template_parameter_list" {
                Self::traverse_node(
                    &child,
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                    current_access,
                );
            }
        }
    }

    fn process_alias_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
        current_access: Option<&str>,
    ) {
        let name_node = match ts_node.child_by_field_name("name") {
            Some(n) => n,
            None => return,
        };
        let type_name = node_text(&name_node, source).trim();
        if type_name.is_empty() {
            return;
        }

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
        if let Some(acc) = current_access {
            attrs.insert("visibility".to_string(), serde_json::json!(acc));
        }

        let qualified = if container_chain.is_empty() {
            Some(type_name.to_string())
        } else {
            Some(format!("{}::{}", container_chain.join("::"), type_name))
        };

        let claim = builder.make_node(
            NodeKind::Type,
            type_name,
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

    fn process_type_definition(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        let mut type_name = String::new();

        let mut cursor = ts_node.walk();
        for child in ts_node.children(&mut cursor) {
            if child.kind() == "type_identifier" {
                type_name = node_text(&child, source).trim().to_string();
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

    fn process_enum_specifier(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
        current_access: Option<&str>,
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
        if let Some(acc) = current_access {
            attrs.insert("visibility".to_string(), serde_json::json!(acc));
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

    fn process_declaration(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
        current_access: Option<&str>,
    ) {
        let current_parent_id = *parent_id_stack.last().unwrap();
        let mut has_nested_type = false;

        let mut cursor = ts_node.walk();
        for child in ts_node.children(&mut cursor) {
            match child.kind() {
                "class_specifier" => {
                    has_nested_type = true;
                    Self::process_class_or_struct(
                        &child,
                        source,
                        builder,
                        container_chain,
                        parent_id_stack,
                        facts,
                        NodeKind::Class,
                        current_access,
                    );
                }
                "struct_specifier" => {
                    has_nested_type = true;
                    Self::process_class_or_struct(
                        &child,
                        source,
                        builder,
                        container_chain,
                        parent_id_stack,
                        facts,
                        NodeKind::Struct,
                        current_access,
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
                        current_access,
                    );
                }
                _ => {}
            }
        }

        if has_nested_type {
            return;
        }

        // Global variable / constant / function prototype
        let decl_text = node_text(ts_node, source);
        let is_const = decl_text.contains("const ") || decl_text.contains("constexpr ");
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
            if let Some(acc) = current_access {
                attrs.insert("visibility".to_string(), serde_json::json!(acc));
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
                            "field_expression" => func_node
                                .child_by_field_name("field")
                                .map(|f| node_text(&f, source).trim().to_string())
                                .unwrap_or_default(),
                            "qualified_identifier" => {
                                node_text(&func_node, source).trim().to_string()
                            }
                            _ => String::new(),
                        };

                        if !fn_name.is_empty() {
                            let simple_name =
                                fn_name.rsplit("::").next().unwrap_or(&fn_name).to_string();

                            facts.unresolved.push(UnresolvedRef {
                                from: from_id,
                                seeking: simple_name,
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
                    if !type_name.is_empty() && !is_builtin_cpp_type(type_name) {
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

fn is_builtin_cpp_type(name: &str) -> bool {
    matches!(
        name,
        "bool"
            | "char"
            | "char8_t"
            | "char16_t"
            | "char32_t"
            | "wchar_t"
            | "short"
            | "int"
            | "long"
            | "signed"
            | "unsigned"
            | "float"
            | "double"
            | "void"
            | "auto"
            | "size_t"
            | "int8_t"
            | "int16_t"
            | "int32_t"
            | "int64_t"
            | "uint8_t"
            | "uint16_t"
            | "uint32_t"
            | "uint64_t"
    )
}
