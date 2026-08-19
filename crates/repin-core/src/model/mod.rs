pub mod edge;
pub mod identity;
pub mod node;
pub mod provenance;
pub mod registries;
pub mod unresolved;

pub use edge::{Edge, EdgeClaim, FactClaimKey};
pub use identity::{EdgeId, NodeId};
pub use node::{Attributes, Node, NodeClaim};
pub use provenance::{Confidence, Derivation, FactOwner, Provenance, Revision};
pub use registries::{ArtifactClass, EdgeKind, NodeKind};
pub use unresolved::{UnresolvedKey, UnresolvedRef};
