use repin_core::hash::ContentHash;
use repin_core::model::registries::{ArtifactClass, NodeKind};
use repin_packs::{ProseLanguagePack, PyLanguagePack, RustLanguagePack, TsLanguagePack};
use repin_core::ports::fs::FileSnapshot;
use repin_core::ports::pack::LanguagePack;

#[test]
fn test_rust_comprehensive_extraction() {
    let source = br#"
use std::collections::HashMap;
use repin_core::model::NodeId;

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

#[test]
fn test_py_comprehensive_extraction() {
    let source = br#"
import os
import numpy as np
from typing import Dict, List, Optional
from .base import BaseService

DEFAULT_TIMEOUT: int = 30
MAX_RETRIES = 3

class WorkerService(BaseService):
    """Service for background jobs."""
    concurrency: int = 4

    def __init__(self, name: str) -> None:
        """Initialize worker."""
        self.name = name

    @classmethod
    def from_env(cls) -> "WorkerService":
        return cls("env-worker")

    @property
    def is_active(self) -> bool:
        return True

    async def process_task(self, task_id: str) -> bool:
        """Process single task asynchronously."""
        return True

def standalone_helper(x: int, y: int) -> int:
    """Helper math function."""
    return x + y
"#;

    let snapshot = FileSnapshot {
        root: "root".to_string(),
        path: "src/worker.py".to_string(),
        content: source.to_vec(),
        content_hash: ContentHash::of_bytes(source),
        artifact_class: ArtifactClass::Code,
    };

    let pack = PyLanguagePack::new();
    assert!(pack.can_handle("src/worker.py", source));

    let facts = pack.extract(&snapshot).unwrap();

    let node_names: Vec<(&str, NodeKind)> = facts
        .nodes
        .iter()
        .map(|n| (n.node.name.as_str(), n.node.kind))
        .collect();

    assert!(node_names.contains(&("src/worker.py", NodeKind::File)));
    assert!(node_names.contains(&("DEFAULT_TIMEOUT", NodeKind::Constant)));
    assert!(node_names.contains(&("MAX_RETRIES", NodeKind::Constant)));
    assert!(node_names.contains(&("WorkerService", NodeKind::Class)));
    assert!(node_names.contains(&("concurrency", NodeKind::Field)));
    assert!(node_names.contains(&("__init__", NodeKind::Constructor)));
    assert!(node_names.contains(&("from_env", NodeKind::Method)));
    assert!(node_names.contains(&("is_active", NodeKind::Method)));
    assert!(node_names.contains(&("process_task", NodeKind::Method)));
    assert!(node_names.contains(&("standalone_helper", NodeKind::Function)));

    // Verify docstring summary on class and function
    let class_node = facts
        .nodes
        .iter()
        .find(|n| n.node.name == "WorkerService")
        .unwrap();
    assert_eq!(
        class_node
            .node
            .attributes
            .get("doc_summary")
            .unwrap()
            .as_str()
            .unwrap(),
        "Service for background jobs."
    );

    let helper_node = facts
        .nodes
        .iter()
        .find(|n| n.node.name == "standalone_helper")
        .unwrap();
    assert_eq!(
        helper_node
            .node
            .attributes
            .get("doc_summary")
            .unwrap()
            .as_str()
            .unwrap(),
        "Helper math function."
    );

    // Verify async attribute
    let async_node = facts
        .nodes
        .iter()
        .find(|n| n.node.name == "process_task")
        .unwrap();
    assert!(
        async_node
            .node
            .attributes
            .get("is_async")
            .unwrap()
            .as_bool()
            .unwrap()
    );

    // Verify imports
    let seeking: Vec<&str> = facts
        .unresolved
        .iter()
        .map(|u| u.seeking.as_str())
        .collect();
    assert!(seeking.contains(&"os"));
    assert!(seeking.contains(&"numpy"));
    assert!(seeking.contains(&"Dict"));
    assert!(seeking.contains(&"List"));
    assert!(seeking.contains(&"BaseService"));

    // Verify extends edge in unresolved
    let extends_ref = facts
        .unresolved
        .iter()
        .find(|u| u.seeking == "BaseService" && u.edge_kind == repin_core::model::registries::EdgeKind::Extends);
    assert!(extends_ref.is_some());
}

#[test]
fn test_py_shebang_and_stubs_detection() {
    let script = b"#!/usr/bin/env python3\nprint('hello')\n";
    let pack = PyLanguagePack::new();

    assert!(pack.can_handle("bin/run-task", script));
    assert!(pack.can_handle("types/stub.pyi", b""));
    assert!(pack.can_handle("gui/app.pyw", b""));
    assert!(!pack.can_handle("src/other.txt", b"plain text"));
}
