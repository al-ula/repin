pub mod direct_regex;
pub mod direct_scanner;

pub use direct_regex::{DirectRegex, DirectRegexError};
pub use direct_scanner::DirectScanner;

#[cfg(test)]
mod tests {
    use super::*;
    use repin_core::hash::ContentHash;
    use repin_core::model::registries::ArtifactClass;
    use repin_core::ports::fs::FileSnapshot;

    #[test]
    fn test_direct_regex_search() {
        let content = b"fn main() {\n    println!(\"hello\");\n}\n";
        let snapshot = FileSnapshot {
            root: "root".to_string(),
            path: "src/main.rs".to_string(),
            content: content.to_vec(),
            content_hash: ContentHash::of_bytes(content),
            artifact_class: ArtifactClass::Code,
        };

        let regex = DirectRegex::compile("println", false).unwrap();
        let evidence = DirectScanner::scan_snapshot(&regex, &snapshot, 10).unwrap();
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].path, "src/main.rs");
        assert!(evidence[0].range.is_some());
        assert_eq!(evidence[0].range.unwrap().start.line, 2);
    }
}
