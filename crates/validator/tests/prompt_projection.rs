use bgx_adapter_v2_sdk::CapabilityContractV2;
use serde_json::{json, Value};

#[test]
fn prompt_projection_is_optional_and_profile_id_stays_opaque() {
    let raw = include_str!("../../../profiles/speech-v1/fixtures/capability.json");
    let bare: CapabilityContractV2 = serde_json::from_str(raw).unwrap();
    assert!(bare.r#operations[0].r#prompt_projection.is_none());
    let mut declared: Value = serde_json::from_str(raw).unwrap();
    declared["operations"][0]["promptProjection"] = json!({"profileId":"profile-a","version":1});
    let parsed: CapabilityContractV2 = serde_json::from_value(declared).unwrap();
    let projection = parsed.r#operations[0].r#prompt_projection.as_ref().unwrap();
    assert_eq!(projection.r#profile_id, "profile-a");
    assert_eq!(projection.r#version, 1);
    let schema = include_str!("../../../schemas/capability-contract.schema.json");
    let lower = schema.to_ascii_lowercase();
    for vendor in ["ltx", "runninghub", "comfy", "seedance", "wan"] {
        assert!(!lower.contains(vendor), "{vendor}");
    }
}
