use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidQuery,
    PathOutsideRoot,
    ScopeExcluded,
    CapabilityUnconfigured,
    CapabilityUnavailable,
    CapabilityUnsupported,
    StatePermissions,
    EntityAmbiguous,
    EntityNotFound,
    NoPathFound,
    StaleResult,
    IndexBuilding,
    RevisionTooOld,
    Timeout,
    Cancelled,
    ResultTruncated,
    ProjectLeaseUnavailable,
    InternalError,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InvalidQuery => "INVALID_QUERY",
            Self::PathOutsideRoot => "PATH_OUTSIDE_ROOT",
            Self::ScopeExcluded => "SCOPE_EXCLUDED",
            Self::CapabilityUnconfigured => "CAPABILITY_UNCONFIGURED",
            Self::CapabilityUnavailable => "CAPABILITY_UNAVAILABLE",
            Self::CapabilityUnsupported => "CAPABILITY_UNSUPPORTED",
            Self::StatePermissions => "STATE_PERMISSIONS",
            Self::EntityAmbiguous => "ENTITY_AMBIGUOUS",
            Self::EntityNotFound => "ENTITY_NOT_FOUND",
            Self::NoPathFound => "NO_PATH_FOUND",
            Self::StaleResult => "STALE_RESULT",
            Self::IndexBuilding => "INDEX_BUILDING",
            Self::RevisionTooOld => "REVISION_TOO_OLD",
            Self::Timeout => "TIMEOUT",
            Self::Cancelled => "CANCELLED",
            Self::ResultTruncated => "RESULT_TRUNCATED",
            Self::ProjectLeaseUnavailable => "PROJECT_LEASE_UNAVAILABLE",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}
