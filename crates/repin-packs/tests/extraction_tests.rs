use repin_core::hash::ContentHash;
use repin_core::model::registries::{ArtifactClass, NodeKind};
use repin_packs::{CLanguagePack, CppLanguagePack, GoLanguagePack, JavaLanguagePack, ProseLanguagePack, PyLanguagePack, RustLanguagePack, TsLanguagePack};
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

#[test]
fn test_go_comprehensive_extraction() {
    let source = br#"
package server

import (
	"context"
	"sync"
	json "encoding/json"
	_ "net/http/pprof"
)

const (
	// DefaultPort is the fallback TCP port.
	DefaultPort = 8080
	MaxRetries  = 3
)

// GlobalState tracks running state.
var GlobalState = "idle"

// RequestID represents unique request identifier.
type RequestID string

// ServiceConfig defines server settings.
type ServiceConfig struct {
	// Port to listen on.
	Port int `json:"port"`
	Host string `json:"host"`
}

// AdvancedConfig embeds ServiceConfig.
type AdvancedConfig struct {
	ServiceConfig
	DebugMode bool
}

// Runner defines lifecycle execution.
type Runner interface {
	// Run starts the runner loop.
	Run(ctx context.Context) error
	Stop()
}

func validateHost(host string) error {
	return nil
}

// NewConfig creates initialized configuration.
func NewConfig(host string, port int) (*ServiceConfig, error) {
	if err := validateHost(host); err != nil {
		return nil, err
	}
	return &ServiceConfig{Host: host, Port: port}, nil
}

// Address returns formatted network host and port.
func (c ServiceConfig) Address() string {
	return ""
}

// UpdatePort sets new port value.
func (c *ServiceConfig) UpdatePort(port int) {
	c.Port = port
}
"#;
    let snapshot = FileSnapshot {
        root: "root".to_string(),
        path: "pkg/server/server.go".to_string(),
        content: source.to_vec(),
        content_hash: ContentHash::of_bytes(source),
        artifact_class: ArtifactClass::Code,
    };

    let pack = GoLanguagePack::new();
    assert!(pack.can_handle("pkg/server/server.go", source));

    let facts = pack.extract(&snapshot).unwrap();

    let node_names: Vec<(&str, NodeKind)> = facts
        .nodes
        .iter()
        .map(|n| (n.node.name.as_str(), n.node.kind))
        .collect();

    assert!(node_names.contains(&("pkg/server/server.go", NodeKind::File)));
    assert!(node_names.contains(&("server", NodeKind::Package)));
    assert!(node_names.contains(&("DefaultPort", NodeKind::Constant)));
    assert!(node_names.contains(&("MaxRetries", NodeKind::Constant)));
    assert!(node_names.contains(&("GlobalState", NodeKind::Variable)));
    assert!(node_names.contains(&("RequestID", NodeKind::Type)));
    assert!(node_names.contains(&("ServiceConfig", NodeKind::Struct)));
    assert!(node_names.contains(&("Port", NodeKind::Field)));
    assert!(node_names.contains(&("Host", NodeKind::Field)));
    assert!(node_names.contains(&("Runner", NodeKind::Interface)));
    assert!(node_names.contains(&("Run", NodeKind::Method)));
    assert!(node_names.contains(&("Stop", NodeKind::Method)));
    assert!(node_names.contains(&("NewConfig", NodeKind::Function)));
    assert!(node_names.contains(&("Address", NodeKind::Method)));
    assert!(node_names.contains(&("UpdatePort", NodeKind::Method)));

    // Check doc summary
    let config_node = facts
        .nodes
        .iter()
        .find(|n| n.node.name == "ServiceConfig")
        .unwrap();
    assert_eq!(
        config_node.node.attributes.get("doc_summary").unwrap(),
        &serde_json::json!("ServiceConfig defines server settings.")
    );

    let port_field = facts
        .nodes
        .iter()
        .find(|n| n.node.name == "Port")
        .unwrap();
    assert_eq!(
        port_field.node.attributes.get("tag").unwrap(),
        &serde_json::json!("`json:\"port\"`")
    );
    assert_eq!(
        port_field.node.attributes.get("doc_summary").unwrap(),
        &serde_json::json!("Port to listen on.")
    );

    let address_method = facts
        .nodes
        .iter()
        .find(|n| n.node.name == "Address")
        .unwrap();
    assert_eq!(
        address_method.node.qualified_name.as_deref(),
        Some("ServiceConfig::Address")
    );

    let update_port_method = facts
        .nodes
        .iter()
        .find(|n| n.node.name == "UpdatePort")
        .unwrap();
    assert_eq!(
        update_port_method.node.qualified_name.as_deref(),
        Some("ServiceConfig::UpdatePort")
    );

    // Check imports and cross-file references
    let seeking: Vec<&str> = facts
        .unresolved
        .iter()
        .map(|u| u.seeking.as_str())
        .collect();
    assert!(seeking.contains(&"context"));
    assert!(seeking.contains(&"sync"));
    assert!(seeking.contains(&"json"));
    assert!(seeking.contains(&"pprof"));

    // Check Calls edge for unimported/intra-package function call
    let calls_ref = facts
        .unresolved
        .iter()
        .find(|u| u.seeking == "validateHost" && u.edge_kind == repin_core::model::registries::EdgeKind::Calls);
    assert!(calls_ref.is_some(), "expected Calls unresolved ref for validateHost");

    // Check Instantiates edge for struct literal
    let inst_ref = facts
        .unresolved
        .iter()
        .find(|u| u.seeking == "ServiceConfig" && u.edge_kind == repin_core::model::registries::EdgeKind::Instantiates);
    assert!(inst_ref.is_some(), "expected Instantiates unresolved ref for ServiceConfig");

    // Check Extends edge for embedded struct
    let extends_ref = facts
        .unresolved
        .iter()
        .find(|u| u.seeking == "ServiceConfig" && u.edge_kind == repin_core::model::registries::EdgeKind::Extends);
    assert!(extends_ref.is_some(), "expected Extends unresolved ref for embedded ServiceConfig");
}

#[test]
fn test_c_comprehensive_extraction() {
    let source = br#"
#include <stdio.h>
#include <stdlib.h>
#include "my_types.h"

#define BUFFER_SIZE 4096
#define MIN(a, b) (((a) < (b)) ? (a) : (b))

/// Point structure in 2D space
struct Point {
    int x;
    int y;
};

union Value {
    int i_val;
    double d_val;
};

typedef enum {
    STATUS_OK = 0,
    STATUS_ERR = 1
} Status;

typedef unsigned long long uint64;

static const int MAX_RETRIES = 5;

/// Compute Manhattan distance between two points
int manhattan_distance(struct Point p1, struct Point p2) {
    int dx = abs(p1.x - p2.x);
    int dy = abs(p1.y - p2.y);
    printf("computed dx=%d dy=%d\n", dx, dy);
    return dx + dy;
}
"#;

    let snapshot = FileSnapshot {
        root: "root".to_string(),
        path: "src/geometry.c".to_string(),
        content: source.to_vec(),
        content_hash: ContentHash::of_bytes(source),
        artifact_class: ArtifactClass::Code,
    };

    let pack = CLanguagePack::new();
    assert!(pack.can_handle("src/geometry.c", &[]));
    assert!(pack.can_handle("include/geometry.h", &[]));
    assert!(!pack.can_handle("src/geometry.rs", &[]));

    let facts = pack.extract(&snapshot).unwrap();

    let node_names: Vec<(&str, NodeKind)> = facts
        .nodes
        .iter()
        .map(|n| (n.node.name.as_str(), n.node.kind))
        .collect();

    assert!(node_names.contains(&("src/geometry.c", NodeKind::File)));
    assert!(node_names.contains(&("BUFFER_SIZE", NodeKind::Constant)));
    assert!(node_names.contains(&("MIN", NodeKind::Function)));
    assert!(node_names.contains(&("Point", NodeKind::Struct)));
    assert!(node_names.contains(&("x", NodeKind::Field)));
    assert!(node_names.contains(&("y", NodeKind::Field)));
    assert!(node_names.contains(&("Value", NodeKind::Struct)));
    assert!(node_names.contains(&("i_val", NodeKind::Field)));
    assert!(node_names.contains(&("d_val", NodeKind::Field)));
    assert!(node_names.contains(&("STATUS_OK", NodeKind::Constant)));
    assert!(node_names.contains(&("STATUS_ERR", NodeKind::Constant)));
    assert!(node_names.contains(&("uint64", NodeKind::Type)));
    assert!(node_names.contains(&("MAX_RETRIES", NodeKind::Constant)));
    assert!(node_names.contains(&("manhattan_distance", NodeKind::Function)));

    // Check doc summary
    let point_node = facts
        .nodes
        .iter()
        .find(|n| n.node.name == "Point")
        .unwrap();
    assert_eq!(
        point_node.node.attributes.get("doc_summary").unwrap().as_str().unwrap(),
        "Point structure in 2D space"
    );

    let fn_node = facts
        .nodes
        .iter()
        .find(|n| n.node.name == "manhattan_distance")
        .unwrap();
    assert_eq!(
        fn_node.node.attributes.get("doc_summary").unwrap().as_str().unwrap(),
        "Compute Manhattan distance between two points"
    );

    // Check includes
    let seeking_imports: Vec<&str> = facts
        .unresolved
        .iter()
        .filter(|u| u.edge_kind == repin_core::model::registries::EdgeKind::Imports)
        .map(|u| u.seeking.as_str())
        .collect();
    assert!(seeking_imports.contains(&"stdio"));
    assert!(seeking_imports.contains(&"stdlib"));
    assert!(seeking_imports.contains(&"my_types"));

    // Check calls
    let seeking_calls: Vec<&str> = facts
        .unresolved
        .iter()
        .filter(|u| u.edge_kind == repin_core::model::registries::EdgeKind::Calls)
        .map(|u| u.seeking.as_str())
        .collect();
    assert!(seeking_calls.contains(&"abs"));
    assert!(seeking_calls.contains(&"printf"));
}

#[test]
fn test_cpp_comprehensive_extraction() {
    let source = br#"
#include <iostream>
#include <vector>
#include "engine/base.hpp"

using namespace std;
using Engine::BaseNode;

namespace engine::rendering {

/// Rendering engine interface
class IRenderer {
public:
    virtual ~IRenderer() = default;
    virtual void render() = 0;
};

/// Vulkan renderer implementation
template <typename DeviceTraits>
class VulkanRenderer : public IRenderer, public BaseNode {
public:
    VulkanRenderer(int device_id) : device_id_(device_id) {}
    ~VulkanRenderer() override {}

    void render() override {
        setup_pipeline();
        cout << "rendering frame" << endl;
    }

private:
    void setup_pipeline() {}
    int device_id_;
};

struct RenderConfig {
    bool enable_vsync;
    int width;
    int height;
};

enum class PipelineState {
    Ready,
    Executing,
    Failed
};

using RendererPtr = VulkanRenderer<void>*;

} // namespace engine::rendering
"#;

    let snapshot = FileSnapshot {
        root: "root".to_string(),
        path: "src/renderer.cpp".to_string(),
        content: source.to_vec(),
        content_hash: ContentHash::of_bytes(source),
        artifact_class: ArtifactClass::Code,
    };

    let pack = CppLanguagePack::new();
    assert!(pack.can_handle("src/renderer.cpp", &[]));
    assert!(pack.can_handle("include/renderer.hpp", &[]));
    assert!(pack.can_handle("include/renderer.hh", &[]));
    assert!(pack.can_handle("include/renderer.h", b"namespace engine { class Foo {}; }"));
    assert!(!pack.can_handle("src/renderer.rs", &[]));

    let facts = pack.extract(&snapshot).unwrap();

    let node_names: Vec<(&str, NodeKind)> = facts
        .nodes
        .iter()
        .map(|n| (n.node.name.as_str(), n.node.kind))
        .collect();

    assert!(node_names.contains(&("src/renderer.cpp", NodeKind::File)));
    assert!(node_names.contains(&("engine::rendering", NodeKind::Namespace)));
    assert!(node_names.contains(&("IRenderer", NodeKind::Class)));
    assert!(node_names.contains(&("~IRenderer", NodeKind::Method)));
    assert!(node_names.contains(&("render", NodeKind::Method)));
    assert!(node_names.contains(&("VulkanRenderer", NodeKind::Class)));
    assert!(node_names.contains(&("VulkanRenderer", NodeKind::Constructor)));
    assert!(node_names.contains(&("~VulkanRenderer", NodeKind::Method)));
    assert!(node_names.contains(&("setup_pipeline", NodeKind::Method)));
    assert!(node_names.contains(&("device_id_", NodeKind::Field)));
    assert!(node_names.contains(&("RenderConfig", NodeKind::Struct)));
    assert!(node_names.contains(&("enable_vsync", NodeKind::Field)));
    assert!(node_names.contains(&("width", NodeKind::Field)));
    assert!(node_names.contains(&("height", NodeKind::Field)));
    assert!(node_names.contains(&("PipelineState", NodeKind::Enum)));
    assert!(node_names.contains(&("Ready", NodeKind::Constant)));
    assert!(node_names.contains(&("Executing", NodeKind::Constant)));
    assert!(node_names.contains(&("Failed", NodeKind::Constant)));
    assert!(node_names.contains(&("RendererPtr", NodeKind::Type)));

    // Check doc summary
    let renderer_node = facts
        .nodes
        .iter()
        .find(|n| n.node.name == "IRenderer")
        .unwrap();
    assert_eq!(
        renderer_node.node.attributes.get("doc_summary").unwrap().as_str().unwrap(),
        "Rendering engine interface"
    );

    // Check extends inheritance
    let seeking_extends: Vec<&str> = facts
        .unresolved
        .iter()
        .filter(|u| u.edge_kind == repin_core::model::registries::EdgeKind::Extends)
        .map(|u| u.seeking.as_str())
        .collect();
    assert!(seeking_extends.contains(&"IRenderer"));
    assert!(seeking_extends.contains(&"BaseNode"));

    // Check imports (includes and using)
    let seeking_imports: Vec<&str> = facts
        .unresolved
        .iter()
        .filter(|u| u.edge_kind == repin_core::model::registries::EdgeKind::Imports)
        .map(|u| u.seeking.as_str())
        .collect();
    assert!(seeking_imports.contains(&"iostream"));
    assert!(seeking_imports.contains(&"vector"));
    assert!(seeking_imports.contains(&"base"));
    assert!(seeking_imports.contains(&"std"));
    assert!(seeking_imports.contains(&"BaseNode"));

    // Check calls
    let seeking_calls: Vec<&str> = facts
        .unresolved
        .iter()
        .filter(|u| u.edge_kind == repin_core::model::registries::EdgeKind::Calls)
        .map(|u| u.seeking.as_str())
        .collect();
    assert!(seeking_calls.contains(&"setup_pipeline"));
}

#[test]
fn test_java_comprehensive_extraction() {
    let source = br#"
package org.repin.server;

import java.util.Map;
import java.util.HashMap;
import java.io.Serializable;
import static org.junit.Assert.assertNotNull;

/**
 * Core order management service interface.
 */
public interface IOrderService extends Serializable {
    void processOrder(String orderId);
}

/**
 * Order processing status enum.
 */
public enum OrderStatus {
    PENDING,
    PROCESSING,
    COMPLETED
}

/**
 * Order entity record.
 */
public record OrderItem(String itemId, double price, int quantity) {}

/**
 * Implementation of order service.
 */
public class OrderServiceImpl extends BaseOrderService implements IOrderService {
    public static final int DEFAULT_TIMEOUT = 3000;
    private final Map<String, OrderItem> orders;

    /**
     * Constructor for OrderServiceImpl.
     */
    public OrderServiceImpl() {
        this.orders = new HashMap<>();
    }

    @Override
    public void processOrder(String orderId) {
        assertNotNull(orderId);
        validateOrder(orderId);
        System.out.println("processing " + orderId);
    }

    private void validateOrder(String orderId) {}
}
"#;

    let snapshot = FileSnapshot {
        root: "root".to_string(),
        path: "src/main/java/org/repin/server/OrderServiceImpl.java".to_string(),
        content: source.to_vec(),
        content_hash: ContentHash::of_bytes(source),
        artifact_class: ArtifactClass::Code,
    };

    let pack = JavaLanguagePack::new();
    assert!(pack.can_handle("src/main/java/org/repin/server/OrderServiceImpl.java", &[]));
    assert!(!pack.can_handle("src/main/java/org/repin/server/OrderServiceImpl.rs", &[]));

    let facts = pack.extract(&snapshot).unwrap();

    let node_names: Vec<(&str, NodeKind)> = facts
        .nodes
        .iter()
        .map(|n| (n.node.name.as_str(), n.node.kind))
        .collect();

    assert!(node_names.contains(&("src/main/java/org/repin/server/OrderServiceImpl.java", NodeKind::File)));
    assert!(node_names.contains(&("org.repin.server", NodeKind::Package)));
    assert!(node_names.contains(&("IOrderService", NodeKind::Interface)));
    assert!(node_names.contains(&("processOrder", NodeKind::Method)));
    assert!(node_names.contains(&("OrderStatus", NodeKind::Enum)));
    assert!(node_names.contains(&("PENDING", NodeKind::Constant)));
    assert!(node_names.contains(&("PROCESSING", NodeKind::Constant)));
    assert!(node_names.contains(&("COMPLETED", NodeKind::Constant)));
    assert!(node_names.contains(&("OrderItem", NodeKind::Struct)));
    assert!(node_names.contains(&("itemId", NodeKind::Field)));
    assert!(node_names.contains(&("price", NodeKind::Field)));
    assert!(node_names.contains(&("quantity", NodeKind::Field)));
    assert!(node_names.contains(&("OrderServiceImpl", NodeKind::Class)));
    assert!(node_names.contains(&("DEFAULT_TIMEOUT", NodeKind::Constant)));
    assert!(node_names.contains(&("orders", NodeKind::Field)));
    assert!(node_names.contains(&("OrderServiceImpl", NodeKind::Constructor)));
    assert!(node_names.contains(&("validateOrder", NodeKind::Method)));

    // Check doc summary
    let iface_node = facts
        .nodes
        .iter()
        .find(|n| n.node.name == "IOrderService")
        .unwrap();
    assert_eq!(
        iface_node.node.attributes.get("doc_summary").unwrap().as_str().unwrap(),
        "Core order management service interface."
    );

    // Check extends and implements
    let seeking_extends: Vec<&str> = facts
        .unresolved
        .iter()
        .filter(|u| u.edge_kind == repin_core::model::registries::EdgeKind::Extends)
        .map(|u| u.seeking.as_str())
        .collect();
    assert!(seeking_extends.contains(&"Serializable"));
    assert!(seeking_extends.contains(&"BaseOrderService"));

    let seeking_implements: Vec<&str> = facts
        .unresolved
        .iter()
        .filter(|u| u.edge_kind == repin_core::model::registries::EdgeKind::Implements)
        .map(|u| u.seeking.as_str())
        .collect();
    assert!(seeking_implements.contains(&"IOrderService"));

    // Check imports
    let seeking_imports: Vec<&str> = facts
        .unresolved
        .iter()
        .filter(|u| u.edge_kind == repin_core::model::registries::EdgeKind::Imports)
        .map(|u| u.seeking.as_str())
        .collect();
    assert!(seeking_imports.contains(&"Map"));
    assert!(seeking_imports.contains(&"HashMap"));
    assert!(seeking_imports.contains(&"Serializable"));
    assert!(seeking_imports.contains(&"assertNotNull"));

    // Check instantiations
    let seeking_instantiations: Vec<&str> = facts
        .unresolved
        .iter()
        .filter(|u| u.edge_kind == repin_core::model::registries::EdgeKind::Instantiates)
        .map(|u| u.seeking.as_str())
        .collect();
    assert!(seeking_instantiations.contains(&"HashMap"));

    // Check calls
    let seeking_calls: Vec<&str> = facts
        .unresolved
        .iter()
        .filter(|u| u.edge_kind == repin_core::model::registries::EdgeKind::Calls)
        .map(|u| u.seeking.as_str())
        .collect();
    assert!(seeking_calls.contains(&"assertNotNull"));
    assert!(seeking_calls.contains(&"validateOrder"));
}
