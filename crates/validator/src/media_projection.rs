//! Negotiated image/video projection. Platforms and scene graphs stay in adapters.
use crate::{ContractError, SchemaRegistry, ValidatedOperationInput};
use serde_json::Value;
use std::collections::BTreeSet;

pub const FEATURE: &str = "media-projection-v1";
const OUTPUT: &str = "https://bgxiong.local/vendor-adapter/v2/profiles/media-projection-v1/output.schema.json";
fn invalid(path: &str, reason: &str) -> ContractError { ContractError::new("INVALID_INPUT", path, reason) }
fn visual(kind: &Value) -> bool { kind == "image" || kind == "video" }
fn projection(value: &Value) -> bool { value == "flat" || value == "equirectangular180" || value == "equirectangular360" }

pub fn validate_contract(contract: &Value) -> Result<(), ContractError> {
    let kind = match contract["capabilityKey"].as_str() {
        Some("image.generate") => "image", Some("video.generate") => "video", _ => return Ok(()),
    };
    let registry = crate::input::registry(contract)?;
    for operation in contract["operations"].as_array().unwrap() {
        let id = operation["inputSchema"]["id"].as_str().unwrap();
        let root = registry.schema(id).unwrap();
        let field = registry.input_field_schema(id, "/outputProjection")?;
        let values = field["enum"].as_array().ok_or_else(|| invalid("/outputProjection", "projection enum is required"))?;
        if field["type"] != "string" || values.is_empty() || values.iter().any(|v| !projection(v))
            || values.iter().filter_map(Value::as_str).collect::<BTreeSet<_>>().len() != values.len()
            || !values.contains(&field["default"])
            || !root["required"].as_array().is_some_and(|a| a.iter().any(|v| v == "outputProjection"))
            || operation["execution"]["streaming"] != false {
            return Err(invalid("/outputProjection", "projection requires a closed enum, valid default, required input and non-streaming operation"));
        }
        if !operation["outputSlots"].as_array().unwrap().iter().any(|slot| slot["kind"] == kind && slot["minCount"].as_u64().unwrap_or(0) > 0) {
            return Err(invalid("/outputSlots", "projection requires a successful visual output of the declared capability kind"));
        }
    }
    Ok(())
}

pub fn enable(mut context: ValidatedOperationInput, features: &BTreeSet<String>) -> Result<ValidatedOperationInput, ContractError> {
    if features.contains(FEATURE) && features.contains(crate::image_projection::FEATURE) {
        return Err(invalid("/requiredFeatures", "only one projection profile can be negotiated"));
    }
    if features.contains(FEATURE) && matches!(context.contract()["capabilityKey"].as_str(), Some("image.generate" | "video.generate")) {
        validate_contract(context.contract())?;
        if !projection(&context.request()["input"]["outputProjection"]) { return Err(invalid("/input/outputProjection", "projection choice is required")); }
        context.media_projection = true;
    }
    Ok(context)
}

pub fn validate_result(context: &ValidatedOperationInput, result: &Value) -> Result<(), ContractError> {
    if !context.media_projection || result["kind"] == "accepted" { return Ok(()); }
    SchemaRegistry::public()?.validate(OUTPUT, &result["data"])?;
    let entries = result["data"]["mediaArtifacts"].as_array().unwrap();
    let outputs = result["outputs"].as_array().ok_or_else(|| invalid("/outputs", "outputs are required"))?;
    let visuals: Vec<_> = outputs.iter().filter(|o| o["kind"] == "media" && visual(&o["media"]["kind"])).collect();
    if result["kind"] != "completed" && result["status"] != "succeeded" {
        if !entries.is_empty() || !visuals.is_empty() { return Err(invalid("/data/mediaArtifacts", "non-success cannot publish visual media")); }
        return Ok(());
    }
    if visuals.is_empty() || entries.len() != visuals.len() { return Err(invalid("/data/mediaArtifacts", "each successful visual output needs exactly one projection")); }
    let mut seen = BTreeSet::new();
    for entry in entries {
        if !seen.insert(entry["mediaId"].as_str().unwrap()) || entry["projection"] != context.request()["input"]["outputProjection"]
            || !visuals.iter().any(|o| o["media"]["id"] == entry["mediaId"] && o["role"] == entry["role"] && o["media"]["kind"] == entry["kind"]) {
            return Err(invalid("/data/mediaArtifacts", "projection identity, kind, role or request choice differs"));
        }
    }
    Ok(())
}

pub fn validate_geometry(context: &ValidatedOperationInput, kind: &str, width: Option<u32>, height: Option<u32>) -> Result<(), ContractError> {
    if !context.media_projection || !matches!(kind, "image" | "video") { return Ok(()); }
    let ratio = match context.request()["input"]["outputProjection"].as_str() {
        Some("equirectangular180") => 1, Some("equirectangular360") => 2, _ => return Ok(()),
    };
    if !height.is_some_and(|h| h > 0 && h.checked_mul(ratio) == width) {
        return Err(invalid("/outputs/media", "decoded or probed dimensions do not match the declared equirectangular projection"));
    }
    Ok(())
}
