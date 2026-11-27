use bgx_adapter_v2_validator::media_target_range_repair::{
    validate, validate_anchor, validate_patch_parent, Message,
};
use serde_json::{json, Value};

fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../../profiles/media-target-range-repair-v1/fixtures/receipts.json"
    ))
    .unwrap()
}

#[test]
fn accepts_manual_box_without_candidate_and_rejects_invented_candidate() {
    let sample = fixture();
    let mut anchor = sample["retrack"]["anchor"].clone();
    validate_anchor(&sample["seed"], &anchor).unwrap();
    anchor["candidateId"] = json!("invented");
    assert!(validate_anchor(&sample["seed"], &anchor).is_err());
    anchor.as_object_mut().unwrap().remove("candidateId");
    anchor["box"]["width"] = json!(0.90);
    assert!(validate_anchor(&sample["seed"], &anchor).is_err());
}

#[test]
fn rejects_wrong_target_revision_and_anchor_at_exclusive_end() {
    let sample = fixture();
    let parent = &sample["trajectory"];
    let mut patch = sample["retrack"].clone();
    validate_patch_parent(parent, &patch).unwrap();
    patch["binding"]["targetId"] = json!("other");
    assert!(validate_patch_parent(parent, &patch).is_err());
    patch = sample["retrack"].clone();
    patch["parent"]["revision"] = json!("old");
    assert!(validate_patch_parent(parent, &patch).is_err());
    patch["anchorFrame"] = json!(150);
    assert!(validate(Message::Patch, &patch).is_err());
}

#[test]
fn rejects_incomplete_overlap_or_claimed_full_coverage() {
    let sample = fixture();
    let mut parent = sample["trajectory"].clone();
    parent["runs"][1]["startFrame"] = json!(101);
    assert!(validate(Message::Trajectory, &parent).is_err());
    parent["runs"][1]["startFrame"] = json!(99);
    assert!(validate(Message::Trajectory, &parent).is_err());
    parent["runs"][1]["startFrame"] = json!(100);
    parent["runs"].as_array_mut().unwrap().pop();
    assert!(validate(Message::Trajectory, &parent).is_err());
}
