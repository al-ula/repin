use repin_core::protocol::ipc::RebuildTarget;
use repin_core::protocol::{
    BOOTSTRAP_VERSION, PROTOCOL_MAX, PROTOCOL_MIN, replacement_allowed, select_protocol,
};

#[test]
fn protocol_selection_is_highest_common_and_build_identity_independent() {
    assert_eq!(select_protocol(1, 3, 2, 4), Some(3));
    assert_eq!(select_protocol(1, 1, 2, 2), None);
    assert_eq!(BOOTSTRAP_VERSION, 1);
    const { assert!(PROTOCOL_MIN <= PROTOCOL_MAX) };
    assert!(!replacement_allowed(PROTOCOL_MIN, PROTOCOL_MAX, true));
    assert!(replacement_allowed(PROTOCOL_MAX + 1, PROTOCOL_MAX, true));
    assert!(!replacement_allowed(PROTOCOL_MAX + 1, PROTOCOL_MAX, false));

    for target in [
        RebuildTarget::Graph,
        RebuildTarget::Lexical,
        RebuildTarget::Vector,
        RebuildTarget::All,
    ] {
        let encoded = serde_json::to_string(&target).unwrap();
        assert_eq!(
            serde_json::from_str::<RebuildTarget>(&encoded).unwrap(),
            target
        );
    }
}
