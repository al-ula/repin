use globset::{Glob, GlobSet, GlobSetBuilder};
use repin_core::config::IndexingConfig;
use repin_core::model::registries::ArtifactClass;

pub struct ExclusionFilter {
    safety_names: Vec<&'static str>,
    safety_extensions: Vec<&'static str>,
    custom_extensions: Vec<String>,
    custom_globs: Option<GlobSet>,
}

impl Default for ExclusionFilter {
    fn default() -> Self {
        Self {
            safety_names: vec![
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
            safety_extensions: vec![
                "pem", "key", "pfx", "p12", "lock", "exe", "dll", "so", "dylib", "bin",
            ],
            custom_extensions: Vec::new(),
            custom_globs: None,
        }
    }
}

impl ExclusionFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: &IndexingConfig) -> Self {
        let mut filter = Self {
            custom_extensions: config.exclude_extensions.clone(),
            ..Default::default()
        };

        if !config.exclude_paths.is_empty() {
            let mut builder = GlobSetBuilder::new();
            for pat in &config.exclude_paths {
                if let Ok(glob) = Glob::new(pat) {
                    builder.add(glob);
                } else if let Ok(glob) = Glob::new(&format!("**/{}/**", pat.trim_matches('/'))) {
                    builder.add(glob);
                }
            }
            if let Ok(set) = builder.build() {
                filter.custom_globs = Some(set);
            }
        }

        filter
    }

    pub fn is_excluded(&self, path: &str) -> bool {
        let normalized = path.replace('\\', "/");
        let segments: Vec<&str> = normalized.split('/').filter(|s| !s.is_empty()).collect();

        // 1. Immutable safety floor names
        for seg in &segments {
            if self.safety_names.contains(seg) {
                return true;
            }
        }

        // 2. Extensions check (safety floors + custom)
        if let Some(file_name) = segments.last() {
            if let Some(dot_idx) = file_name.rfind('.') {
                let ext = &file_name[dot_idx + 1..];
                if self.safety_extensions.contains(&ext) {
                    return true;
                }
            }
            for custom_ext in &self.custom_extensions {
                let clean = custom_ext.trim_start_matches('.');
                if file_name.ends_with(&format!(".{}", clean)) || file_name.ends_with(custom_ext) {
                    return true;
                }
            }
        }

        // 3. Custom path globs
        if let Some(ref globs) = self.custom_globs {
            if globs.is_match(&normalized) {
                return true;
            }
            for seg in &segments {
                if globs.is_match(*seg) {
                    return true;
                }
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
    fn test_default_exclusions_and_safety_floors() {
        let filter = ExclusionFilter::default();
        assert!(filter.is_excluded(".git/config"));
        assert!(filter.is_excluded("project/.env"));
        assert!(filter.is_excluded("keys/server.key"));
        assert!(filter.is_excluded("node_modules/pkg/index.js"));
        assert!(!filter.is_excluded("src/main.rs"));
    }

    #[test]
    fn test_custom_config_exclusions() {
        let mut config = IndexingConfig::default();
        config.exclude_paths = vec!["**/build/**".to_string(), "vendor/**".to_string()];
        config.exclude_extensions = vec!["min.js".to_string(), "snap".to_string()];

        let filter = ExclusionFilter::with_config(&config);

        // Safety floors still hold
        assert!(filter.is_excluded(".git/HEAD"));
        assert!(filter.is_excluded(".env"));

        // Custom path exclusions match
        assert!(filter.is_excluded("build/output.js"));
        assert!(filter.is_excluded("vendor/lib/foo.rs"));

        // Custom extension exclusions match
        assert!(filter.is_excluded("static/bundle.min.js"));
        assert!(filter.is_excluded("tests/snapshots/foo.snap"));

        // Normal files remain included
        assert!(!filter.is_excluded("src/lib.rs"));
    }

    #[test]
    fn test_classification() {
        assert_eq!(classify_artifact("src/main.rs"), ArtifactClass::Code);
        assert_eq!(classify_artifact("tests/unit.rs"), ArtifactClass::Tests);
        assert_eq!(classify_artifact("docs/arch.md"), ArtifactClass::Docs);
        assert_eq!(classify_artifact("config.toml"), ArtifactClass::Config);
    }
}
