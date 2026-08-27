pub mod extractor_util;

#[cfg(feature = "prose")]
pub mod prose_pack;
#[cfg(feature = "rust")]
pub mod rust_pack;
#[cfg(feature = "typescript")]
pub mod ts_pack;

#[cfg(feature = "prose")]
pub use prose_pack::{PROSE_PACK_VERSION, ProseLanguagePack};
#[cfg(feature = "rust")]
pub use rust_pack::{RUST_PACK_VERSION, RustLanguagePack};
#[cfg(feature = "typescript")]
pub use ts_pack::{TS_PACK_VERSION, TsLanguagePack};

use repin_core::ports::pack::LanguagePack;

#[allow(clippy::vec_init_then_push)]
pub fn default_packs() -> Vec<Box<dyn LanguagePack>> {
    let mut packs: Vec<Box<dyn LanguagePack>> = Vec::new();
    #[cfg(feature = "rust")]
    packs.push(Box::new(RustLanguagePack::new()));
    #[cfg(feature = "typescript")]
    packs.push(Box::new(TsLanguagePack::new()));
    #[cfg(feature = "prose")]
    packs.push(Box::new(ProseLanguagePack::new()));
    packs
}

#[cfg(test)]
mod tests {
    use super::*;
    use repin_core::hash::ContentHash;
    use repin_core::model::registries::ArtifactClass;
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
}
