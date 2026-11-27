//! Dynamic form choices belong to one installed operation and configuration.
use crate::{
    capabilities::{Binding, CapabilityRef, CapabilityRegistry},
    package::VerifiedPackage,
    ContractError, SchemaRegistry,
};
use serde_json::Value;
use std::collections::BTreeSet;

pub const FEATURE: &str = "operation-form-options-v1";
pub const KIND: &str = "operation_form_options";
fn invalid(path: &str, reason: &str) -> ContractError {
    ContractError::new("INVALID_INPUT", path, reason)
}

pub(crate) fn validate_declaration(
    op: &Value,
    registry: &SchemaRegistry,
) -> Result<(), ContractError> {
    let Some(fields) = op["uiSchema"]["dynamicOptions"].as_array() else {
        return Ok(());
    };
    let controls = op["uiSchema"]["controls"].as_array().unwrap();
    let id = op["inputSchema"]["id"].as_str().unwrap();
    if registry
        .schema(id)
        .is_some_and(|root| root["readOnly"] == true || root.get("const").is_some())
    {
        return Err(invalid(
            "/inputSchema",
            "A frozen input cannot expose dynamic controls",
        ));
    }
    let mut pointers = BTreeSet::new();
    for field in fields {
        let target = field["fieldPointer"].as_str().unwrap();
        if !pointers.insert(target) {
            return Err(invalid(target, "Duplicate dynamic option declaration"));
        }
        for (pointer, ty) in [
            (Some(target), "string"),
            (field["enabledBy"].as_str(), "boolean"),
        ] {
            let Some(pointer) = pointer else { continue };
            let s = registry.input_field_schema(id, pointer)?;
            let mut matches = controls.iter().filter(|c| c["fieldPointer"] == pointer);
            let widget = matches.next().and_then(|c| c["widget"].as_str());
            let valid_widget = match ty {
                "string" => matches!(widget, Some("text" | "select")),
                _ => matches!(widget, Some("checkbox" | "toggle")),
            };
            if matches.next().is_some()
                || !valid_widget
                || s["type"] != ty
                || s.get("const").is_some()
                || s["readOnly"] == true
                || op["mediaSlots"].as_array().unwrap().iter().any(|slot| {
                    let media = slot["inputPointer"].as_str().unwrap();
                    pointer == media
                        || pointer
                            .strip_prefix(media)
                            .is_some_and(|rest| rest.starts_with('/'))
                })
            {
                return Err(invalid(
                    pointer,
                    "Dynamic options require declared editable scalar controls",
                ));
            }
            for (end, _) in pointer.match_indices('/').skip(1) {
                let parent = registry.input_field_schema(id, &pointer[..end])?;
                if parent["readOnly"] == true || parent.get("const").is_some() {
                    return Err(invalid(
                        pointer,
                        "Dynamic option field belongs to a frozen input",
                    ));
                }
            }
        }
    }
    Ok(())
}

pub fn selected_operation(
    catalog: &CapabilityRegistry,
    selected: &CapabilityRef,
    binding: &Binding,
    package: &VerifiedPackage,
    request: &Value,
) -> Result<(Value, SchemaRegistry), ContractError> {
    if !package.enabled_features().contains(FEATURE) {
        return Err(ContractError::new(
            "UNSUPPORTED_FEATURE",
            "/kind",
            "Form options feature was not negotiated",
        ));
    }
    if request["kind"] != KIND || selected.package != *package.identity() {
        return Err(invalid(
            "/selection",
            "Probe kind or selected package differs",
        ));
    }
    let contract = catalog.contract_for(selected, binding)?;
    if request["modelId"] != selected.model_id
        || request["capabilityKey"] != selected.capability_key
        || request["operationId"] != selected.operation_id
        || request["operationVersion"] != selected.operation_version
        || request["inputSchemaDigest"] != selected.input_schema_digest
        || request["configuration"] != *binding.configuration()
    {
        return Err(invalid(
            "/selection",
            "Form options selection or configuration changed",
        ));
    }
    crate::validate_configuration(contract, binding.configuration())?;
    let c = crate::input::as_value(contract)?;
    let op = c["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|op| {
            op["id"] == selected.operation_id
                && op["version"] == selected.operation_version
                && op["uiSchema"]["dynamicOptions"].is_array()
        })
        .cloned()
        .ok_or_else(|| {
            invalid(
                "/operationId",
                "Selected operation did not declare dynamic options",
            )
        })?;
    let registry = crate::input::registry(&c)?;
    registry.validate_dispatch("probe", "input", request)?;
    Ok((op, registry))
}

pub fn validate_fields(
    (op, registry): &(Value, SchemaRegistry),
    result: &Value,
) -> Result<(), ContractError> {
    if result["kind"] != KIND {
        return Err(invalid("/kind", "Wrong form options response kind"));
    }
    registry.validate_dispatch("probe", "result", result)?;
    let expected: BTreeSet<_> = op["uiSchema"]["dynamicOptions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field["fieldPointer"].as_str().unwrap())
        .collect();
    let mut seen = BTreeSet::new();
    for field in result["fields"]
        .as_array()
        .ok_or_else(|| invalid("/fields", "Missing form fields"))?
    {
        let name = field["fieldPointer"]
            .as_str()
            .ok_or_else(|| invalid("/fields", "Field pointer missing"))?;
        let choices = field["choices"]
            .as_array()
            .ok_or_else(|| invalid(name, "Choice list missing"))?;
        if !expected.contains(name) || !seen.insert(name) {
            return Err(invalid(name, "Undeclared or repeated dynamic field"));
        }
        let reason = field.get("reason");
        if choices.is_empty() {
            if reason
                .and_then(Value::as_str)
                .is_none_or(|s| s.trim().is_empty())
            {
                return Err(invalid(name, "Unavailable fields require a reason"));
            }
        } else if reason.is_some() {
            return Err(invalid(
                name,
                "Available fields cannot report an unavailable reason",
            ));
        }
        let constraint =
            registry.input_field_validator(op["inputSchema"]["id"].as_str().unwrap(), name)?;
        let mut values = BTreeSet::new();
        for choice in choices {
            let value = choice
                .as_str()
                .ok_or_else(|| invalid(name, "Expected a string option"))?;
            if value.trim().is_empty() || !values.insert(value) || !constraint.is_valid(choice) {
                return Err(invalid(
                    name,
                    "Invalid or repeated choice for declared input schema",
                ));
            }
        }
    }
    if expected != seen {
        return Err(invalid(
            "/fields",
            "Response must cover exactly the declared fields",
        ));
    }
    Ok(())
}
