pub mod extractor_util;

#[cfg(feature = "prose")]
pub mod prose_pack;
#[cfg(feature = "rust")]
pub mod rust_pack;
#[cfg(feature = "typescript")]
pub mod ts_pack;
#[cfg(feature = "python")]
pub mod py_pack;
#[cfg(feature = "go")]
pub mod go_pack;
#[cfg(feature = "c")]
pub mod c_pack;

#[cfg(feature = "prose")]
pub use prose_pack::{PROSE_PACK_VERSION, ProseLanguagePack};
#[cfg(feature = "rust")]
pub use rust_pack::{RUST_PACK_VERSION, RustLanguagePack};
#[cfg(feature = "typescript")]
pub use ts_pack::{TS_PACK_VERSION, TsLanguagePack};
#[cfg(feature = "python")]
pub use py_pack::{PY_PACK_VERSION, PyLanguagePack};
#[cfg(feature = "go")]
pub use go_pack::{GO_PACK_VERSION, GoLanguagePack};
#[cfg(feature = "c")]
pub use c_pack::{C_PACK_VERSION, CLanguagePack};

use repin_core::ports::pack::LanguagePack;

#[allow(clippy::vec_init_then_push)]
pub fn default_packs() -> Vec<Box<dyn LanguagePack>> {
    let mut packs: Vec<Box<dyn LanguagePack>> = Vec::new();
    #[cfg(feature = "rust")]
    packs.push(Box::new(RustLanguagePack::new()));
    #[cfg(feature = "typescript")]
    packs.push(Box::new(TsLanguagePack::new()));
    #[cfg(feature = "python")]
    packs.push(Box::new(PyLanguagePack::new()));
    #[cfg(feature = "prose")]
    packs.push(Box::new(ProseLanguagePack::new()));
    #[cfg(feature = "go")]
    packs.push(Box::new(GoLanguagePack::new()));
    #[cfg(feature = "c")]
    packs.push(Box::new(CLanguagePack::new()));
    packs
}

#[cfg(test)]
mod tests {
    use super::*;
    use repin_core::hash::ContentHash;
    use repin_core::model::registries::{ArtifactClass, EdgeKind, NodeKind};
    use repin_core::ports::fs::FileSnapshot;

    #[cfg(feature = "rust")]
    #[test]
    fn test_rust_pack_extraction() {
        let code = b"use std::io;\n\npub fn hello() {}\n";
        let snapshot = FileSnapshot {
            root: "root".to_string(),
            path: "src/lib.rs".to_string(),
            content: code.to_vec(),
            content_hash: ContentHash::of_bytes(code),
            artifact_class: ArtifactClass::Code,
        };

        let pack = RustLanguagePack::new();
        assert!(pack.can_handle("src/lib.rs", code));

        let facts = pack.extract(&snapshot).unwrap();
        assert_eq!(facts.nodes.len(), 2); // File + Function
        assert_eq!(facts.edges.len(), 1); // File -> Function contains
        assert_eq!(facts.unresolved.len(), 1); // use std::io -> seeking io
    }
    #[cfg(feature = "python")]
    #[test]
    fn test_py_pack_extraction() {
        let code = b"import os\n\ndef hello():\n    \"\"\"Docstring.\"\"\"\n    pass\n";
        let snapshot = FileSnapshot {
            root: "root".to_string(),
            path: "src/main.py".to_string(),
            content: code.to_vec(),
            content_hash: ContentHash::of_bytes(code),
            artifact_class: ArtifactClass::Code,
        };

        let pack = PyLanguagePack::new();
        assert!(pack.can_handle("src/main.py", code));

        let facts = pack.extract(&snapshot).unwrap();
        assert_eq!(facts.nodes.len(), 2); // File + Function
        assert_eq!(facts.edges.len(), 1); // File -> Function contains
        assert_eq!(facts.unresolved.len(), 1); // import os -> seeking os
    }
    #[cfg(feature = "go")]
    #[test]
    fn test_go_pack_extraction() {
        let code = b"package main\n\nimport \"fmt\"\n\nfunc Hello() {}\n";
        let snapshot = FileSnapshot {
            root: "root".to_string(),
            path: "main.go".to_string(),
            content: code.to_vec(),
            content_hash: ContentHash::of_bytes(code),
            artifact_class: ArtifactClass::Code,
        };

        let pack = GoLanguagePack::new();
        assert!(pack.can_handle("main.go", code));

        let facts = pack.extract(&snapshot).unwrap();
        assert_eq!(facts.nodes.len(), 3); // File + Package + Function
        assert_eq!(facts.edges.len(), 2); // File -> Package contains, File -> Function contains
        assert_eq!(facts.unresolved.len(), 1); // import "fmt" -> seeking fmt
    }
    #[cfg(feature = "c")]
    #[test]
    fn test_c_pack_extraction() {
        let c_code = r#"
        #include <stdio.h>
        #include "custom.h"

        #define MAX_SIZE 1024

        // Point structure
        struct Point {
            int x;
            int y;
        };

        typedef enum Status {
            OK = 0,
            ERROR = 1
        } Status;

        // Calculate distance
        double distance(struct Point p1, struct Point p2) {
            printf("calculating\n");
            return 0.0;
        }
        "#;

        let snapshot = FileSnapshot {
            root: "root".to_string(),
            path: "main.c".to_string(),
            content: c_code.as_bytes().to_vec(),
            artifact_class: ArtifactClass::Code,
            content_hash: ContentHash::of_bytes(c_code.as_bytes()),
        };

        let pack = CLanguagePack::new();
        let facts = pack.extract(&snapshot).unwrap();

        let node_names: Vec<(&str, NodeKind)> = facts
            .nodes
            .iter()
            .map(|n| (n.node.name.as_str(), n.node.kind))
            .collect();

        assert!(node_names.contains(&("main.c", NodeKind::File)));
        assert!(node_names.contains(&("MAX_SIZE", NodeKind::Constant)));
        assert!(node_names.contains(&("Point", NodeKind::Struct)));
        assert!(node_names.contains(&("x", NodeKind::Field)));
        assert!(node_names.contains(&("y", NodeKind::Field)));
        assert!(node_names.contains(&("Status", NodeKind::Enum)));
        assert!(node_names.contains(&("OK", NodeKind::Constant)));
        assert!(node_names.contains(&("distance", NodeKind::Function)));

        let calls: Vec<&str> = facts
            .unresolved
            .iter()
            .filter(|u| u.edge_kind == EdgeKind::Calls)
            .map(|u| u.seeking.as_str())
            .collect();
        assert!(calls.contains(&"printf"));
    }
}
