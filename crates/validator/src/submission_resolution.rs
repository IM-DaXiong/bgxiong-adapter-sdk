//! Online reconciliation uses a frozen durable service domain, never a missing-job guess.
use crate::{ContractError, SchemaRegistry};
use serde_json::Value;
pub const FEATURE: &str = "submission-resolution-v1";
pub fn is_kind(value: &Value) -> bool {
    matches!(value["kind"].as_str(), Some("submission_identity" | "submission_resolution"))
}
pub fn validate_result(request: &Value, result: &Value) -> Result<(), ContractError> {
    let registry = SchemaRegistry::public()?;
    registry.validate_at("https://bgxiong.local/vendor-adapter/v2/submission-resolution.schema.json", "/$defs/request", request)?;
    registry.validate_at("https://bgxiong.local/vendor-adapter/v2/submission-resolution.schema.json", "/$defs/result", result)?;
    if ["kind", "requestId", "requestDigest", "expectedRevision"].iter().any(|k| request[*k] != result[*k])
        || (request["kind"] == "submission_resolution" && request["submissionScope"] != result["submissionScope"])
        || (result["status"] == "accepted" && result["accepted"]["kind"] != "accepted") {
        return Err(ContractError::new("INVALID_INPUT", "/submissionResolution", "Resolution changed frozen request, revision, scope or accepted result kind"));
    }
    Ok(())
}
