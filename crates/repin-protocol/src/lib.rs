pub mod envelope;
pub mod errors;
pub mod evidence;
pub mod freshness;
pub mod ipc;
pub mod provider;

pub const BOOTSTRAP_VERSION: u32 = 1;
pub const PROTOCOL_MIN: u32 = 1;
pub const PROTOCOL_MAX: u32 = 1;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const MAX_BOOTSTRAP_FRAME_BYTES: usize = 64 * 1024;
pub const BOOTSTRAP_DEADLINE_MS: u64 = 2_000;

pub use envelope::{ResultEnvelope, ResultProvenance, SourceKind, Status, Warning};
pub use errors::ErrorCode;
pub use evidence::Evidence;
pub use freshness::{
    CoverageState, Freshness, GraphState, LexicalState, Truncation, TruncationReason,
};
pub use ipc::{
    BootstrapHandshake, BootstrapHandshakeOk, BootstrapRejected, IpcMessage, IpcRequest,
    IpcResponse, IpcResponseEnvelope, replacement_allowed, select_protocol,
};
pub use provider::{ProviderId, ProviderInfo, ProviderKind, ProviderLocation};

#[cfg(test)]
mod tests {
    use super::*;
    use repin_core::line_index::{ByteSpan, Position, Range};

    #[test]
    fn test_envelope_serde_roundtrip() {
        let mut env = ResultEnvelope::ok(serde_json::json!({ "items": ["a", "b"] }));
        env.evidence.push(
            Evidence::new("src/main.rs")
                .with_range(Range {
                    span: ByteSpan::new(0, 10),
                    start: Position::new(1, 1),
                    end: Position::new(1, 11),
                })
                .with_preview("fn main()"),
        );

        let json = serde_json::to_string(&env).unwrap();
        let deserialized: ResultEnvelope<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(env.status, deserialized.status);
        assert_eq!(env.evidence.len(), deserialized.evidence.len());
    }

    #[test]
    fn test_ipc_serde_roundtrip() {
        let req = IpcRequest::Handshake {
            client_version: "0.1.0".to_string(),
            project_db_path: "/test/.repin/graph.sqlite3".to_string(),
        };
        let msg = IpcMessage {
            request_id: 42,
            body: req,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: IpcMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn highest_common_protocol_is_selected() {
        assert_eq!(select_protocol(1, 4, 2, 3), Some(3));
        assert_eq!(select_protocol(1, 1, 2, 2), None);
        assert!(replacement_allowed(3, 2, true));
        assert!(!replacement_allowed(2, 2, true));
        assert!(!replacement_allowed(3, 2, false));
    }
}
