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

pub const PROSE_PACK_VERSION: &str = "0.2.0";
pub struct ProseLanguagePack;

impl Default for ProseLanguagePack {
    fn default() -> Self {
        Self
    }
}

impl ProseLanguagePack {
    pub fn new() -> Self {
        Self
    }
}

impl LanguagePack for ProseLanguagePack {
    fn name(&self) -> &'static str {
        "prose_pack"
    }

    fn version(&self) -> &'static str {
        PROSE_PACK_VERSION
    }

    fn can_handle(&self, path: &str, _sample_content: &[u8]) -> bool {
        path.ends_with(".md") || path.ends_with(".markdown") || path.ends_with(".txt")
    }

    fn extract(&self, snapshot: &FileSnapshot) -> Result<ExtractedFacts, ExtractionError> {
        let mut parser = Parser::new();
        let language = tree_sitter_md::LANGUAGE.into();
        parser
            .set_language(&language)
            .map_err(|e| ExtractionError::ParseFailure(e.to_string()))?;

        let tree = parser.parse(&snapshot.content, None).ok_or_else(|| {
            ExtractionError::ParseFailure("failed to parse markdown source".to_string())
        })?;

        let line_index = LineIndex::build(&snapshot.content);
        let mut builder = FactBuilder::new(
            &snapshot.root,
            &snapshot.path,
            "markdown",
            snapshot.artifact_class,
            self.name(),
            self.version(),
            &line_index,
            &snapshot.content,
        );

        let doc_node_id = NodeId::new(
            NodeKind::Document,
            &snapshot.root,
            &snapshot.path,
            &[],
            &snapshot.path,
            0,
        );

        let doc_claim = builder.make_node(
            NodeKind::Document,
            &snapshot.path,
            Some(snapshot.path.clone()),
            &[],
            &tree.root_node(),
            Attributes::default(),
        );

        let mut facts = ExtractedFacts::default();
        facts.nodes.push(doc_claim);

        // Track active heading stack for hierarchical section containment: Vec<(level, NodeId, String)>
        let mut section_stack: Vec<(u32, NodeId, String)> =
            vec![(0, doc_node_id, snapshot.path.clone())];

        let root_node = tree.root_node();
        Self::traverse_markdown(
            &root_node,
            &snapshot.content,
            &mut builder,
            &mut section_stack,
            &mut facts,
        );

        Ok(facts)
    }
}

impl ProseLanguagePack {
    fn traverse_markdown(
        ts_node: &TsNode<'_>,
        source: &[u8],
        builder: &mut FactBuilder<'_>,
        section_stack: &mut Vec<(u32, NodeId, String)>,
        facts: &mut ExtractedFacts,
    ) {
        let kind = ts_node.kind();

        if kind == "atx_heading" || kind == "setext_heading" {
            let (level, heading_text) = Self::parse_heading(ts_node, source);
            if !heading_text.is_empty() {
                // Pop stack until parent level is strictly less than this heading level
                while section_stack.len() > 1 && section_stack.last().unwrap().0 >= level {
                    section_stack.pop();
                }

                let parent_id = section_stack.last().unwrap().1;
                let container_chain: Vec<String> = section_stack
                    .iter()
                    .skip(1)
                    .map(|(_, _, name)| name.clone())
                    .collect();

                let mut attrs = Attributes::default();
                attrs.insert("level".to_string(), serde_json::json!(level));

                let heading_claim = builder.make_node(
                    NodeKind::Heading,
                    &heading_text,
                    Some(heading_text.clone()),
                    &container_chain,
                    ts_node,
                    attrs.clone(),
                );
                let heading_id = heading_claim.node.id;
                let heading_range = heading_claim.node.range;
                facts.nodes.push(heading_claim);

                // Edge: parent contains heading
                facts.edges.push(builder.make_edge(
                    parent_id,
                    heading_id,
                    EdgeKind::Contains,
                    heading_range,
                    Attributes::default(),
                ));

                // Also create Section node representing the section body
                let section_claim = builder.make_node(
                    NodeKind::Section,
                    &heading_text,
                    Some(format!("Section: {}", heading_text)),
                    &container_chain,
                    ts_node,
                    attrs,
                );
                let section_id = section_claim.node.id;
                let section_range = section_claim.node.range;
                facts.nodes.push(section_claim);

                facts.edges.push(builder.make_edge(
                    parent_id,
                    section_id,
                    EdgeKind::Contains,
                    section_range,
                    Attributes::default(),
                ));

                // Push this section onto hierarchy stack
                section_stack.push((level, section_id, heading_text));
            }
        } else if kind == "paragraph"
            || kind == "inline"
            || kind == "link_destination"
            || kind == "link"
        {
            let raw_text = node_text(ts_node, source);
            let links = Self::extract_links(raw_text);
            let current_parent_id = section_stack.last().unwrap().1;

            for clean_dest in links {
                if (clean_dest.ends_with(".md")
                    || clean_dest.contains(".md#")
                    || clean_dest.starts_with("../")
                    || clean_dest.starts_with("./"))
                    && !clean_dest.starts_with("http://")
                    && !clean_dest.starts_with("https://")
                {
                    facts.unresolved.push(UnresolvedRef {
                        from: current_parent_id,
                        seeking: clean_dest.clone(),
                        scope_hint: Some(clean_dest),
                        edge_kind: EdgeKind::LinksTo,
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

        let mut cursor = ts_node.walk();
        for child in ts_node.children(&mut cursor) {
            Self::traverse_markdown(&child, source, builder, section_stack, facts);
        }
    }

    fn extract_links(text: &str) -> Vec<String> {
        let mut links = Vec::new();
        let mut cursor = text;
        while let Some(start_bracket) = cursor.find('[') {
            let after_bracket = &cursor[start_bracket + 1..];
            if let Some(end_bracket) = after_bracket.find(']') {
                let after_end_bracket = &after_bracket[end_bracket + 1..];
                if after_end_bracket.starts_with('(')
                    && let Some(end_paren) = after_end_bracket.find(')')
                {
                    let link = &after_end_bracket[1..end_paren];
                    links.push(link.trim().to_string());
                    cursor = &after_end_bracket[end_paren + 1..];
                    continue;
                }
            }
            cursor = after_bracket;
        }
        links
    }

    fn parse_heading(ts_node: &TsNode<'_>, source: &[u8]) -> (u32, String) {
        let full_text = node_text(ts_node, source).trim();
        let mut level = 1;

        if full_text.starts_with("######") {
            level = 6;
        } else if full_text.starts_with("#####") {
            level = 5;
        } else if full_text.starts_with("####") {
            level = 4;
        } else if full_text.starts_with("###") {
            level = 3;
        } else if full_text.starts_with("##") {
            level = 2;
        } else if full_text.starts_with('#') {
            level = 1;
        }

        let text = full_text.trim_start_matches('#').trim().to_string();
        (level, text)
    }
}
