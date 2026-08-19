use repin_core::hash::ContentHash;
use repin_core::model::registries::{ArtifactClass, NodeKind};
use repin_core::ports::fs::FileSnapshot;
use repin_core::ports::pack::LanguagePack;
use repin_packs::{ProseLanguagePack, RustLanguagePack, TsLanguagePack};

#[test]
fn test_rust_comprehensive_extraction() {
    let source = br#"
use std::collections::HashMap;
use crate::model::NodeId;

/// A documented engine service
pub struct EngineService {
    id: u64,
}

impl EngineService {
    /// Constructs a new service
    pub fn new(id: u64) -> Self {
        Self { id }
    }

    fn internal_tick(&mut self) {}
}

pub enum State {
    Active,
    Idle,
}
"#;

    let snapshot = FileSnapshot {
        root: "root".to_string(),
        path: "src/service.rs".to_string(),
        content: source.to_vec(),
        content_hash: ContentHash::of_bytes(source),
        artifact_class: ArtifactClass::Code,
    };

    let pack = RustLanguagePack::new();
    let facts = pack.extract(&snapshot).unwrap();

    let node_names: Vec<(&str, NodeKind)> = facts
        .nodes
        .iter()
        .map(|n| (n.node.name.as_str(), n.node.kind))
        .collect();

    assert!(node_names.contains(&("src/service.rs", NodeKind::File)));
    assert!(node_names.contains(&("EngineService", NodeKind::Struct)));
    assert!(node_names.contains(&("new", NodeKind::Method)));
    assert!(node_names.contains(&("internal_tick", NodeKind::Method)));
    assert!(node_names.contains(&("State", NodeKind::Enum)));

    // Check doc summary
    let service_node = facts
        .nodes
        .iter()
        .find(|n| n.node.name == "EngineService")
        .unwrap();
    assert_eq!(
        service_node
            .node
            .attributes
            .get("doc_summary")
            .unwrap()
            .as_str()
            .unwrap(),
        "A documented engine service"
    );

    // Check imports
    let seeking: Vec<&str> = facts
        .unresolved
        .iter()
        .map(|u| u.seeking.as_str())
        .collect();
    assert!(seeking.contains(&"HashMap"));
    assert!(seeking.contains(&"NodeId"));
}

#[test]
fn test_ts_comprehensive_extraction() {
    let source = br#"
import { Client } from './client';
import express from 'express';

/**
 * Main application class
 */
export class Application {
    private client: Client;

    /**
     * Start the app
     */
    start(): void {
        console.log("running");
    }
}

export interface AppConfig {
    port: number;
}

export const launchApp = () => {
    return new Application();
};
"#;

    let snapshot = FileSnapshot {
        root: "root".to_string(),
        path: "src/app.ts".to_string(),
        content: source.to_vec(),
        content_hash: ContentHash::of_bytes(source),
        artifact_class: ArtifactClass::Code,
    };

    let pack = TsLanguagePack::new();
    let facts = pack.extract(&snapshot).unwrap();

    let node_names: Vec<(&str, NodeKind)> = facts
        .nodes
        .iter()
        .map(|n| (n.node.name.as_str(), n.node.kind))
        .collect();

    assert!(node_names.contains(&("src/app.ts", NodeKind::File)));
    assert!(node_names.contains(&("Application", NodeKind::Class)));
    assert!(node_names.contains(&("start", NodeKind::Method)));
    assert!(node_names.contains(&("AppConfig", NodeKind::Interface)));
    assert!(node_names.contains(&("launchApp", NodeKind::Function)));

    // Check JSDoc summary
    let app_node = facts
        .nodes
        .iter()
        .find(|n| n.node.name == "Application")
        .unwrap();
    assert_eq!(
        app_node
            .node
            .attributes
            .get("doc_summary")
            .unwrap()
            .as_str()
            .unwrap(),
        "Main application class"
    );

    // Check imports
    let seeking: Vec<&str> = facts
        .unresolved
        .iter()
        .map(|u| u.seeking.as_str())
        .collect();
    assert!(seeking.contains(&"client"));
    assert!(seeking.contains(&"express"));
}

#[test]
fn test_markdown_comprehensive_extraction() {
    let source = br#"
# System Architecture

High level system design.

## Storage Layer

Details about storage. See [Graph Model](./graph-model.md).

### SQLite Backend

SQLite embedded implementation.

## Retrieval

Details on deterministic ranking.
"#;

    let snapshot = FileSnapshot {
        root: "root".to_string(),
        path: "docs/arch.md".to_string(),
        content: source.to_vec(),
        content_hash: ContentHash::of_bytes(source),
        artifact_class: ArtifactClass::Docs,
    };

    let pack = ProseLanguagePack::new();
    let facts = pack.extract(&snapshot).unwrap();

    let headings: Vec<&str> = facts
        .nodes
        .iter()
        .filter(|n| n.node.kind == NodeKind::Heading)
        .map(|n| n.node.name.as_str())
        .collect();

    assert!(headings.contains(&"System Architecture"));
    assert!(headings.contains(&"Storage Layer"));
    assert!(headings.contains(&"SQLite Backend"));
    assert!(headings.contains(&"Retrieval"));

    // Check links
    let seeking: Vec<&str> = facts
        .unresolved
        .iter()
        .map(|u| u.seeking.as_str())
        .collect();
    assert!(seeking.contains(&"./graph-model.md"));
}
