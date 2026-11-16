//! Generic operation input checks over the package-owned declaration and frozen capability.
use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::{
    canonical::canonical_json,
    capabilities::{Binding, CapabilityRef, CapabilityRegistry},
    hash_bytes,
    package::VerifiedPackage,
    ContractError, SchemaRegistry,
};

pub const FEATURE: &str = "operation-input-preflight-v2";
const PROFILE: &str = "operation-input-v1";
const SCHEMA: &str = "https://bgxiong.local/vendor-adapter/v2/operation-input.schema.json";

pub fn is_probe(value: &Value) -> bool {
    value["kind"] == bgx_adapter_v2_sdk::OPERATION_INPUT_PREFLIGHT_KIND
}

pub use bgx_adapter_v2_sdk::is_metadata_only_probe as metadata_only_probe;

pub fn uses_profile(operation: &Value) -> bool {
    operation["inputProfile"] == PROFILE
}

pub(crate) fn validate_declaration(package: &VerifiedPackage, operation: &Value) -> Result<(), ContractError> {
    if uses_profile(operation)
        && !(package.enabled_features().contains(FEATURE)
            && package.manifest()["requiredFeatures"].as_array().unwrap().iter().any(|feature| feature == FEATURE))
    {
        return Err(ContractError::new("UNSUPPORTED_FEATURE", "/inputProfile", "operation input profile requires negotiated preflight"));
    }
    Ok(())
}

pub fn digest(value: &Value) -> String {
    hash_bytes(canonical_json(value).as_bytes())
}

pub fn validate_value_budget(value: &Value, path: &str) -> Result<(), ContractError> {
    let policy = crate::schema_registry::policy();
    let limits = &policy["limits"];
    let max_depth = limits["maxOperationInputDepth"].as_u64().unwrap() as usize;
    let max_nodes = limits["maxOperationInputNodes"].as_u64().unwrap() as usize;
    let max_bytes = limits["maxOperationInputBytes"].as_u64().unwrap() as usize;
    let mut queue = vec![(0, value)];
    let mut visited = 0;
    let error = || ContractError::new("INVALID_INPUT", path, "operation value exceeds transport budget");
    while let Some((depth, item)) = queue.pop() {
        visited += 1;
        if depth > max_depth || visited + queue.len() > max_nodes {
            return Err(error());
        }
        if let Some(items) = item.as_array() {
            queue.extend(items.iter().map(|value| (depth + 1, value)));
        } else if let Some(properties) = item.as_object() {
            queue.extend(properties.values().map(|value| (depth + 1, value)));
        }
    }
    if serde_json::to_vec(value).unwrap().len() > max_bytes {
        return Err(error());
    }
    Ok(())
}

pub fn request_digest(request: &Value) -> Result<String, ContractError> {
    validate_value_budget(&request["input"], "/input")?;
    let mut identity = request.as_object().cloned().ok_or_else(|| {
        ContractError::new("INVALID_INPUT", "/", "preflight request must be an object")
    })?;
    identity.remove("requestDigest");
    // Media ordering belongs to the pointers in input. Normalize only the facts
    // table, whose uniqueness is checked by validate_request.
    if let Some(items) = identity.get_mut("mediaMetadata").and_then(Value::as_array_mut) {
        items.sort_by(|a, b| a["mediaId"].as_str().cmp(&b["mediaId"].as_str()));
    }
    Ok(digest(&Value::Object(identity)))
}

/// Preflight has no execution scope or live media handle. The host provides
/// measured facts here and the SDK compares them to live leases before submit.
pub fn validate_request(
    catalog: &CapabilityRegistry,
    selected: &CapabilityRef,
    binding: &Binding,
    request: &Value,
) -> Result<(), ContractError> {
    SchemaRegistry::public()?.validate_at(SCHEMA, "/$defs/request", request)?;
    let contract = catalog.contract_for(selected, binding)?;
    if request["selection"] != serde_json::to_value(selected).unwrap()
        || request["configuration"] != *binding.configuration()
        || request["requestDigest"] != request_digest(request)?
    {
        return Err(ContractError::new("PREPARATION_STALE", "/requestDigest", "preflight request does not match the frozen selection and input"));
    }
    let contract_json = crate::input::as_value(contract)?;
    let operation = contract_json["operations"].as_array().unwrap().iter()
        .find(|op| op["id"] == selected.operation_id && op["version"] == selected.operation_version && uses_profile(op))
        .ok_or_else(|| ContractError::new("UNSUPPORTED_FEATURE", "/operationId", "selected operation has no generic input profile"))?;
    let registry = crate::input::registry(&contract_json)?;
    registry.validate(operation["inputSchema"]["id"].as_str().unwrap(), &request["input"])?;
    crate::validate_configuration(contract, binding.configuration())?;
    let mut facts = BTreeMap::new();
    for media in request["mediaMetadata"].as_array().unwrap() {
        if facts.insert(media["mediaId"].as_str().unwrap(), media).is_some() {
            return Err(ContractError::new("INVALID_INPUT", "/mediaMetadata", "duplicate media identity"));
        }
        if let Some(interval) = media.get("sourceInterval") {
            let duration = media["sourceDurationMs"].as_u64().ok_or_else(|| {
                ContractError::new("INVALID_INPUT", "/sourceDurationMs", "selected interval requires measured source duration")
            })?;
            crate::operation_preflight::validate_interval(interval, duration)?;
        } else if media.get("sourceDurationMs").is_some() {
            return Err(ContractError::new("INVALID_INPUT", "/sourceInterval", "source duration requires a selected interval"));
        }
    }
    let used = crate::media::validate_slot_references(&request["input"], &operation["mediaSlots"], &facts)?;
    if used.len() != facts.len() {
        return Err(ContractError::new("INVALID_INPUT", "/mediaMetadata", "preflight facts must cover exactly the declared media references"));
    }
    Ok(())
}

pub fn validate_result(request: &Value, result: &Value) -> Result<(), ContractError> {
    let registry = SchemaRegistry::public()?;
    registry.validate_at(SCHEMA, "/$defs/request", request)?;
    registry.validate_at(SCHEMA, "/$defs/result", result)?;
    if request["requestDigest"] != request_digest(request)? || request["requestDigest"] != result["requestDigest"] {
        return Err(ContractError::new("PREPARATION_STALE", "/requestDigest", "preflight result belongs to different input"));
    }
    if result["status"] == "ready" {
        validate_value_budget(&result["material"], "/material")?;
    }
    if result["status"] == "ready" && request["environment"]["ready"] != true {
        return Err(ContractError::new("INVALID_INPUT", "/status", "unavailable environment cannot produce ready preflight"));
    }
    Ok(())
}

pub fn seal_result(request: &Value, result: &Value) -> Result<Option<Value>, ContractError> {
    validate_result(request, result)?;
    Ok((result["status"] == "ready").then(|| json!({
        "requestDigest": result["requestDigest"],
        "materialDigest": digest(&result["material"]),
        "material": result["material"],
        "environmentRevisionDigest": request["environment"]["revisionDigest"],
    })))
}
