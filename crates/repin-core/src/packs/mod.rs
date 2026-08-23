pub mod extractor_util;
pub mod prose_pack;
pub mod rust_pack;
pub mod ts_pack;

pub use prose_pack::{PROSE_PACK_VERSION, ProseLanguagePack};
pub use rust_pack::{RUST_PACK_VERSION, RustLanguagePack};
pub use ts_pack::{TS_PACK_VERSION, TsLanguagePack};

use crate::ports::pack::LanguagePack;

pub fn default_packs() -> Vec<Box<dyn LanguagePack>> {
    vec![
        Box::new(RustLanguagePack::new()),
        Box::new(TsLanguagePack::new()),
        Box::new(ProseLanguagePack::new()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::ContentHash;
    use crate::model::registries::ArtifactClass;
    use crate::ports::fs::FileSnapshot;

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
}
