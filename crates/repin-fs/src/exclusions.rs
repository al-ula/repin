use repin_core::model::registries::ArtifactClass;

pub struct ExclusionFilter {
    excluded_names: Vec<&'static str>,
    excluded_extensions: Vec<&'static str>,
}

impl Default for ExclusionFilter {
    fn default() -> Self {
        Self {
            excluded_names: vec![
                ".git",
                ".repin",
                ".env",
                ".env.local",
                ".env.production",
                "id_rsa",
                "id_ed25519",
                "node_modules",
                "target",
                ".DS_Store",
            ],
            excluded_extensions: vec![
                "pem", "key", "pfx", "p12", "lock", "exe", "dll", "so", "dylib", "bin",
            ],
        }
    }
}

impl ExclusionFilter {
    pub fn is_excluded(&self, path: &str) -> bool {
        let normalized = path.replace('\\', "/");
        let segments: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();

        for seg in &segments {
            if self.excluded_names.contains(seg) {
                return true;
            }
        }

        if let Some(file_name) = segments.last()
            && let Some(dot_idx) = file_name.rfind('.')
        {
            let ext = &file_name[dot_idx + 1..];
            if self.excluded_extensions.contains(&ext) {
                return true;
            }
        }

        false
    }
}

pub fn classify_artifact(path: &str) -> ArtifactClass {
    let normalized = path.replace('\\', "/");
    let lower = normalized.to_lowercase();

    if lower.starts_with("tests/")
        || lower.starts_with("test/")
        || lower.contains("/tests/")
        || lower.contains("/test/")
        || lower.ends_with("_test.rs")
        || lower.ends_with(".test.ts")
        || lower.ends_with(".spec.ts")
    {
        ArtifactClass::Tests
    } else if lower.ends_with(".md")
        || lower.starts_with("docs/")
        || lower.contains("/docs/")
        || lower.starts_with("book/")
        || lower.contains("/book/")
    {
        ArtifactClass::Docs
    } else if lower.ends_with(".toml")
        || lower.ends_with(".yaml")
        || lower.ends_with(".yml")
        || lower.ends_with(".json")
    {
        if lower.contains("/schema") || lower.ends_with(".schema.json") {
            ArtifactClass::Schema
        } else {
            ArtifactClass::Config
        }
    } else if lower.contains("/.github/") || lower.contains("/.gitlab-ci/") {
        ArtifactClass::Ci
    } else if lower.contains("/terraform/") || lower.contains("/k8s/") || lower.contains("/docker")
    {
        ArtifactClass::Infra
    } else if lower.ends_with(".sql") || lower.contains("/migrations/") {
        ArtifactClass::Data
    } else if lower.ends_with(".rs")
        || lower.ends_with(".ts")
        || lower.ends_with(".js")
        || lower.ends_with(".go")
        || lower.ends_with(".py")
        || lower.ends_with(".c")
        || lower.ends_with(".cpp")
    {
        ArtifactClass::Code
    } else {
        ArtifactClass::Build
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exclusions() {
        let filter = ExclusionFilter::default();
        assert!(filter.is_excluded(".git/config"));
        assert!(filter.is_excluded("project/.env"));
        assert!(filter.is_excluded("keys/server.key"));
        assert!(filter.is_excluded("node_modules/pkg/index.js"));
        assert!(!filter.is_excluded("src/main.rs"));
    }

    #[test]
    fn test_classification() {
        assert_eq!(classify_artifact("src/main.rs"), ArtifactClass::Code);
        assert_eq!(classify_artifact("tests/unit.rs"), ArtifactClass::Tests);
        assert_eq!(classify_artifact("docs/arch.md"), ArtifactClass::Docs);
        assert_eq!(classify_artifact("config.toml"), ArtifactClass::Config);
    }
}
