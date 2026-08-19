use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    // Structural
    Repository,
    Root,
    Directory,
    File,
    // Packaging
    Package,
    Module,
    Namespace,
    // Types
    Class,
    Struct,
    Interface,
    Trait,
    Enum,
    Type,
    TypeParameter,
    // Callables
    Function,
    Method,
    Constructor,
    Property,
    Accessor,
    // Values
    Variable,
    Constant,
    Field,
    Parameter,
    // Prose
    Document,
    Section,
    Heading,
    LinkTarget,
    // Data
    Schema,
    SchemaField,
    Table,
    Column,
    Migration,
    // Operational
    Endpoint,
    Route,
    Job,
    Service,
    Resource,
    ConfigKey,
    // External
    ExternalSymbol,
    ExternalPackage,
    // Derived
    Concept,
    Topic,
    Responsibility,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Repository => "repository",
            Self::Root => "root",
            Self::Directory => "directory",
            Self::File => "file",
            Self::Package => "package",
            Self::Module => "module",
            Self::Namespace => "namespace",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Interface => "interface",
            Self::Trait => "trait",
            Self::Enum => "enum",
            Self::Type => "type",
            Self::TypeParameter => "type_parameter",
            Self::Function => "function",
            Self::Method => "method",
            Self::Constructor => "constructor",
            Self::Property => "property",
            Self::Accessor => "accessor",
            Self::Variable => "variable",
            Self::Constant => "constant",
            Self::Field => "field",
            Self::Parameter => "parameter",
            Self::Document => "document",
            Self::Section => "section",
            Self::Heading => "heading",
            Self::LinkTarget => "link_target",
            Self::Schema => "schema",
            Self::SchemaField => "schema_field",
            Self::Table => "table",
            Self::Column => "column",
            Self::Migration => "migration",
            Self::Endpoint => "endpoint",
            Self::Route => "route",
            Self::Job => "job",
            Self::Service => "service",
            Self::Resource => "resource",
            Self::ConfigKey => "config_key",
            Self::ExternalSymbol => "external_symbol",
            Self::ExternalPackage => "external_package",
            Self::Concept => "concept",
            Self::Topic => "topic",
            Self::Responsibility => "responsibility",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    // Structure
    Contains,
    Declares,
    Defines,
    // Reference
    References,
    Calls,
    Reads,
    Writes,
    Instantiates,
    // Typing
    HasType,
    Returns,
    Accepts,
    Implements,
    Extends,
    Constrains,
    // Modules
    Imports,
    Exports,
    DependsOn,
    ResolvesTo,
    // Prose
    Documents,
    LinksTo,
    Mentions,
    Anchors,
    // Data
    Queries,
    Migrates,
    ValidatesWith,
    // Operational
    Handles,
    Configures,
    TestedBy,
    Deploys,
    // Derived
    RelatesTo,
    Summarizes,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Declares => "declares",
            Self::Defines => "defines",
            Self::References => "references",
            Self::Calls => "calls",
            Self::Reads => "reads",
            Self::Writes => "writes",
            Self::Instantiates => "instantiates",
            Self::HasType => "has_type",
            Self::Returns => "returns",
            Self::Accepts => "accepts",
            Self::Implements => "implements",
            Self::Extends => "extends",
            Self::Constrains => "constrains",
            Self::Imports => "imports",
            Self::Exports => "exports",
            Self::DependsOn => "depends_on",
            Self::ResolvesTo => "resolves_to",
            Self::Documents => "documents",
            Self::LinksTo => "links_to",
            Self::Mentions => "mentions",
            Self::Anchors => "anchors",
            Self::Queries => "queries",
            Self::Migrates => "migrates",
            Self::ValidatesWith => "validates_with",
            Self::Handles => "handles",
            Self::Configures => "configures",
            Self::TestedBy => "tested_by",
            Self::Deploys => "deploys",
            Self::RelatesTo => "relates_to",
            Self::Summarizes => "summarizes",
        }
    }

    pub fn is_transitive(&self) -> bool {
        matches!(
            self,
            Self::Contains
                | Self::Calls
                | Self::Extends
                | Self::Implements
                | Self::DependsOn
                | Self::ResolvesTo
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactClass {
    Code,
    Tests,
    Docs,
    Config,
    Schema,
    Data,
    Build,
    Ci,
    Infra,
    All,
}

impl ArtifactClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Code => "code",
            Self::Tests => "tests",
            Self::Docs => "docs",
            Self::Config => "config",
            Self::Schema => "schema",
            Self::Data => "data",
            Self::Build => "build",
            Self::Ci => "ci",
            Self::Infra => "infra",
            Self::All => "all",
        }
    }
}
