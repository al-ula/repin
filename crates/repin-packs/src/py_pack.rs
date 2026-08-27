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

pub const PY_PACK_VERSION: &str = "0.2.0";

#[derive(Debug, Default)]
pub struct PyLanguagePack;

impl PyLanguagePack {
    pub fn new() -> Self {
        Self
    }
}

impl LanguagePack for PyLanguagePack {
    fn name(&self) -> &'static str {
        "py_pack"
    }

    fn version(&self) -> &'static str {
        PY_PACK_VERSION
    }

    fn can_handle(&self, path: &str, sample_content: &[u8]) -> bool {
        if path.ends_with(".py") || path.ends_with(".pyi") || path.ends_with(".pyw") {
            return true;
        }

        if sample_content.starts_with(b"#!") {
            let first_line = sample_content
                .split(|&b| b == b'\n')
                .next()
                .unwrap_or(sample_content);
            if let Ok(line_str) = std::str::from_utf8(first_line) {
                return line_str.contains("python");
            }
        }

        false
    }

    fn extract(&self, snapshot: &FileSnapshot) -> Result<ExtractedFacts, ExtractionError> {
        let mut parser = Parser::new();
        let language = tree_sitter_python::LANGUAGE.into();

        parser
            .set_language(&language)
            .map_err(|e| ExtractionError::ParseFailure(e.to_string()))?;

        let tree = parser.parse(&snapshot.content, None).ok_or_else(|| {
            ExtractionError::ParseFailure("failed to parse python source".to_string())
        })?;

        let line_index = LineIndex::build(&snapshot.content);
        let mut builder = FactBuilder::new(
            &snapshot.root,
            &snapshot.path,
            "python",
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

impl PyLanguagePack {
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
            "decorated_definition" => {
                // Collect decorators and delegate to inner definition
                let mut decorators = Vec::new();
                let mut definition_node = None;

                let mut cursor = ts_node.walk();
                for child in ts_node.children(&mut cursor) {
                    if child.kind() == "decorator" {
                        let dec_text = node_text(&child, source).trim();
                        if !dec_text.is_empty() {
                            decorators.push(dec_text.to_string());
                        }
                    } else if child.kind() == "function_definition"
                        || child.kind() == "class_definition"
                    {
                        definition_node = Some(child);
                    }
                }

                if let Some(def_node) = definition_node {
                    Self::process_definition(
                        ts_node, // use decorated_definition node for complete range
                        &def_node,
                        decorators,
                        source,
                        builder,
                        container_chain,
                        parent_id_stack,
                        facts,
                    );
                }
            }
            "function_definition" => {
                Self::process_definition(
                    ts_node,
                    ts_node,
                    Vec::new(),
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                );
            }
            "class_definition" => {
                Self::process_definition(
                    ts_node,
                    ts_node,
                    Vec::new(),
                    source,
                    builder,
                    container_chain,
                    parent_id_stack,
                    facts,
                );
            }
            "type_alias_statement" => {
                // Python 3.12+ `type Alias = int`
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
            "expression_statement" => {
                // Check for top-level or class-level variable assignments or type alias assignments
                let mut cursor = ts_node.walk();
                for child in ts_node.children(&mut cursor) {
                    if child.kind() == "assignment" {
                        Self::process_assignment(
                            &child,
                            ts_node,
                            source,
                            builder,
                            container_chain,
                            current_parent_id,
                            facts,
                        );
                    }
                }
            }
            "import_statement" => {
                // e.g. `import os`, `import math, sys`, `import numpy as np`
                let import_text = node_text(ts_node, source);
                let mut cursor = ts_node.walk();
                for child in ts_node.children(&mut cursor) {
                    if child.kind() == "dotted_name" {
                        let mod_name = node_text(&child, source).trim();
                        if !mod_name.is_empty() {
                            Self::add_import_ref(
                                builder,
                                current_parent_id,
                                mod_name.to_string(),
                                Some(import_text.to_string()),
                                facts,
                            );
                        }
                    } else if child.kind() == "aliased_import"
                        && let Some(name_node) = child.child_by_field_name("name")
                    {
                        let mod_name = node_text(&name_node, source).trim();
                        if !mod_name.is_empty() {
                            Self::add_import_ref(
                                builder,
                                current_parent_id,
                                mod_name.to_string(),
                                Some(import_text.to_string()),
                                facts,
                            );
                        }
                    }
                }
            }
            "import_from_statement" => {
                // e.g. `from os import path`, `from .module import foo as bar`, `from typing import (List, Dict)`
                let import_text = node_text(ts_node, source);
                let module_name = if let Some(mod_node) = ts_node.child_by_field_name("module_name")
                {
                    node_text(&mod_node, source).trim().to_string()
                } else {
                    "".to_string()
                };

                let mut cursor = ts_node.walk();
                for child in ts_node.children(&mut cursor) {
                    if child.kind() == "dotted_name"
                        && ts_node.child_by_field_name("module_name") != Some(child)
                    {
                        let item_name = node_text(&child, source).trim();
                        if !item_name.is_empty() {
                            Self::add_import_ref(
                                builder,
                                current_parent_id,
                                item_name.to_string(),
                                Some(if module_name.is_empty() {
                                    import_text.to_string()
                                } else {
                                    format!("from {module_name}")
                                }),
                                facts,
                            );
                        }
                    } else if child.kind() == "aliased_import"
                        && let Some(name_node) = child.child_by_field_name("name")
                    {
                        let item_name = node_text(&name_node, source).trim();
                        if !item_name.is_empty() {
                            Self::add_import_ref(
                                builder,
                                current_parent_id,
                                item_name.to_string(),
                                Some(if module_name.is_empty() {
                                    import_text.to_string()
                                } else {
                                    format!("from {module_name}")
                                }),
                                facts,
                            );
                        }
                    }
                }

                // If module_name itself is present, record seeking module as well
                if !module_name.is_empty() {
                    let base_mod = module_name.trim_start_matches('.').to_string();
                    if !base_mod.is_empty() {
                        Self::add_import_ref(
                            builder,
                            current_parent_id,
                            base_mod,
                            Some(import_text.to_string()),
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

    #[allow(clippy::too_many_arguments)]
    fn process_definition(
        range_node: &TsNode<'_>,
        def_node: &TsNode<'_>,
        decorators: Vec<String>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &mut Vec<String>,
        parent_id_stack: &mut Vec<NodeId>,
        facts: &mut ExtractedFacts,
    ) {
        let current_parent_id = *parent_id_stack.last().unwrap();
        let kind = def_node.kind();

        match kind {
            "function_definition" => {
                if let Some(name_node) = def_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let mut attrs = Attributes::default();

                    if !decorators.is_empty() {
                        attrs.insert("decorators".to_string(), serde_json::json!(decorators));
                    }

                    // Check async
                    let is_async = def_node
                        .children(&mut def_node.walk())
                        .any(|c| c.kind() == "async")
                        || range_node
                            .children(&mut range_node.walk())
                            .any(|c| c.kind() == "async");
                    if is_async {
                        attrs.insert("is_async".to_string(), serde_json::json!(true));
                    }

                    // Extract docstring
                    if let Some(body_node) = def_node.child_by_field_name("body")
                        && let Some(doc) = Self::extract_docstring(&body_node, source)
                    {
                        attrs.insert("doc_summary".to_string(), serde_json::json!(doc));
                    }

                    // Extract return type if present
                    if let Some(return_type_node) = def_node.child_by_field_name("return_type") {
                        let ret_type = node_text(&return_type_node, source).trim();
                        if !ret_type.is_empty() {
                            attrs.insert("return_type".to_string(), serde_json::json!(ret_type));
                        }
                    }

                    let in_class = !container_chain.is_empty()
                        && container_chain.last().unwrap().starts_with("class ");

                    let node_kind = if in_class {
                        if name == "__init__" || name == "__new__" {
                            NodeKind::Constructor
                        } else {
                            NodeKind::Method
                        }
                    } else {
                        NodeKind::Function
                    };

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
                        range_node,
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

                    // Process nested definitions if any
                    if let Some(body_node) = def_node.child_by_field_name("body") {
                        container_chain.push(format!("fn {name}"));
                        parent_id_stack.push(fn_id);

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
            }
            "class_definition" => {
                if let Some(name_node) = def_node.child_by_field_name("name") {
                    let name = node_text(&name_node, source);
                    let mut attrs = Attributes::default();

                    if !decorators.is_empty() {
                        attrs.insert("decorators".to_string(), serde_json::json!(decorators));
                    }
                    // Extract docstring
                    if let Some(body_node) = def_node.child_by_field_name("body")
                        && let Some(doc) = Self::extract_docstring(&body_node, source)
                    {
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
                        range_node,
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

                    // Extract base classes / inheritance (superclasses)
                    if let Some(arg_list) = def_node.child_by_field_name("superclasses") {
                        let mut cursor = arg_list.walk();
                        for child in arg_list.children(&mut cursor) {
                            if child.kind() == "identifier" || child.kind() == "attribute" {
                                let base_name = node_text(&child, source).trim();
                                if !base_name.is_empty() {
                                    facts.unresolved.push(UnresolvedRef {
                                        from: class_id,
                                        seeking: base_name.to_string(),
                                        scope_hint: Some(format!("class {name}")),
                                        edge_kind: EdgeKind::Extends,
                                        provenance: Provenance {
                                            root: builder.root.to_string(),
                                            path: builder.path.to_string(),
                                            range: None,
                                            extractor: builder.extractor_name.to_string(),
                                            extractor_version: builder
                                                .extractor_version
                                                .to_string(),
                                            derivation: Derivation::Extracted,
                                            confidence: Confidence::EXACT,
                                            revision: Revision::INITIAL,
                                        },
                                    });
                                }
                            }
                        }
                    }

                    // Process class body
                    if let Some(body_node) = def_node.child_by_field_name("body") {
                        container_chain.push(format!("class {name}"));
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
            }
            _ => {}
        }
    }

    fn process_assignment(
        assignment_node: &TsNode<'_>,
        stmt_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        container_chain: &[String],
        current_parent_id: NodeId,
        facts: &mut ExtractedFacts,
    ) {
        if let Some(left_node) = assignment_node.child_by_field_name("left")
            && left_node.kind() == "identifier"
        {
            let name = node_text(&left_node, source).trim();
            if name.is_empty() || name.starts_with('_') && name.ends_with('_') && name != "__all__"
            {
                return;
            }

            let in_class = !container_chain.is_empty()
                && container_chain.last().unwrap().starts_with("class ");

            // Detect if it is a type alias or constant or field or variable
            let is_constant = name.chars().all(|c| !c.is_alphabetic() || c.is_uppercase());
            let node_kind = if in_class {
                NodeKind::Field
            } else if is_constant {
                NodeKind::Constant
            } else {
                NodeKind::Variable
            };

            let mut attrs = Attributes::default();
            if let Some(type_node) = assignment_node.child_by_field_name("type") {
                let type_text = node_text(&type_node, source).trim();
                if !type_text.is_empty() {
                    attrs.insert("type_annotation".to_string(), serde_json::json!(type_text));
                }
            }

            let qualified = if container_chain.is_empty() {
                Some(name.to_string())
            } else {
                Some(format!("{}::{}", container_chain.join("::"), name))
            };

            let var_claim = builder.make_node(
                node_kind,
                name,
                qualified,
                container_chain,
                stmt_node,
                attrs,
            );
            let var_id = var_claim.node.id;
            let var_range = var_claim.node.range;
            facts.nodes.push(var_claim);

            facts.edges.push(builder.make_edge(
                current_parent_id,
                var_id,
                EdgeKind::Contains,
                var_range,
                Attributes::default(),
            ));
        }
    }

    fn add_import_ref(
        builder: &FactBuilder<'_>,
        from_id: NodeId,
        seeking: String,
        scope_hint: Option<String>,
        facts: &mut ExtractedFacts,
    ) {
        facts.unresolved.push(UnresolvedRef {
            from: from_id,
            seeking,
            scope_hint,
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

    fn extract_docstring(body_node: &TsNode<'_>, source: &[u8]) -> Option<String> {
        let mut cursor = body_node.walk();
        for child in body_node.children(&mut cursor) {
            if child.kind() == "expression_statement" {
                if let Some(expr) = child.child(0)
                    && expr.kind() == "string"
                {
                    let text = node_text(&expr, source).trim();
                    let cleaned = text
                        .trim_start_matches(['r', 'u', 'f', 'b', 'R', 'U', 'F', 'B'])
                        .trim_matches('\'')
                        .trim_matches('"')
                        .lines()
                        .map(|l| l.trim())
                        .filter(|l| !l.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ");
                    if !cleaned.is_empty() {
                        return Some(cleaned);
                    }
                }
                break;
            } else if child.kind() != "comment" {
                break;
            }
        }
        None
    }
}
