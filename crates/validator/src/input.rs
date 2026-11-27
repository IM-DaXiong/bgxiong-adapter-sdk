use crate::{hash_bytes, ContractError, SchemaRegistry, ValidatedOperationInput};
use bgx_adapter_v2_sdk::{CapabilityContractV2, OperationSubmitV2};
use serde_json::Value;

pub(crate) fn as_value<T: serde::Serialize>(value: &T) -> Result<Value, ContractError> {
    serde_json::to_value(value)
        .map_err(|_| ContractError::new("INVALID_INPUT", "/", "wire value cannot be encoded"))
}

pub(crate) fn registry(contract: &Value) -> Result<SchemaRegistry, ContractError> {
    SchemaRegistry::register(
        contract["schemas"]
            .as_array()
            .ok_or_else(|| {
                ContractError::new("INVALID_PACKAGE", "/schemas", "schema list missing")
            })?
            .iter()
            .map(|r| {
                (
                    r["id"].as_str().unwrap().to_owned(),
                    r["schemaJson"].as_str().unwrap().to_owned(),
                    r["digest"].as_str().unwrap().to_owned(),
                )
            })
            .collect(),
    )
}

fn check_ref(
    registry: &SchemaRegistry,
    contract: &Value,
    reference: &Value,
) -> Result<(), ContractError> {
    if reference.is_null() {
        return Ok(());
    }
    let resource = contract["schemas"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == reference["id"])
        .ok_or_else(|| {
            ContractError::new(
                "INVALID_PACKAGE",
                "/schemas",
                "schema reference does not belong to package",
            )
        })?;
    if resource["digest"] != reference["digest"]
        || registry.schema(reference["id"].as_str().unwrap()).is_none()
    {
        return Err(ContractError::new(
            "INVALID_PACKAGE",
            "/schemas",
            "schema reference digest mismatch",
        ));
    }
    Ok(())
}

fn accepts_type(schema: &Value, kind: &str) -> bool {
    schema["type"] == kind
        || schema["type"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v == kind))
}

fn required_controls(
    registry: &SchemaRegistry,
    id: &str,
    schema: &Value,
    prefix: &str,
    pointers: &std::collections::BTreeSet<String>,
) -> Result<(), ContractError> {
    for key in schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let key = key.as_str().unwrap().replace('~', "~0").replace('/', "~1");
        let pointer = format!("{prefix}/{key}");
        if pointers.contains(&pointer) {
            continue;
        }
        let field = registry.input_field_schema(id, &pointer)?;
        if field.get("const").is_some() || field["readOnly"] == true {
            continue;
        }
        if accepts_type(&field, "object") && field.get("properties").is_some() {
            required_controls(registry, id, &field, &pointer, pointers)?;
        } else {
            return Err(ContractError::new(
                "UNSUPPORTED_FEATURE",
                pointer,
                "required user input has no supported control",
            ));
        }
    }
    Ok(())
}

pub fn validate_contract(contract: &CapabilityContractV2) -> Result<(), ContractError> {
    let c = as_value(contract)?;
    SchemaRegistry::validate_public("capability-contract", &c)?;
    let r = registry(&c)?;
    check_ref(&r, &c, &c["configurationSchema"])?;
    let mut operations = std::collections::BTreeSet::new();
    for op in c["operations"].as_array().unwrap() {
        if !operations.insert((
            op["id"].clone().to_string(),
            op["version"].as_u64().unwrap(),
        )) {
            return Err(ContractError::new(
                "INVALID_PACKAGE",
                "/operations",
                "duplicate operation version",
            ));
        }
        for key in ["inputSchema", "outputSchema", "resumeSchema"] {
            check_ref(&r, &c, &op[key])?;
        }
        crate::form_options::validate_declaration(op, &r)?;
        if op["execution"]["resumable"] == true
            && op["execution"]["mode"] == "asynchronous"
            && op["resumeSchema"].is_null()
        {
            return Err(ContractError::new(
                "INVALID_PACKAGE",
                "/resumeSchema",
                "resumable asynchronous operation requires resume schema",
            ));
        }
        // best-effort / sync+resumable allowed for third-party packages; durable resume still requires async + resumeSchema above
        validate_standard_profile(op)?;
        if crate::operation_input::uses_profile(op) {
            validate_controls(&r, op)?;
        }
    }
    Ok(())
}

pub fn validate_input(
    contract: &CapabilityContractV2,
    request: &OperationSubmitV2,
) -> Result<ValidatedOperationInput, ContractError> {
    validate_contract(contract)?;
    let c = as_value(contract)?;
    let input = as_value(request)?;
    let r = registry(&c)?;
    r.validate_dispatch("submit", "input", &input)?;
    if input["capabilityKey"] != c["capabilityKey"]
        || !c["modelIds"]
            .as_array()
            .unwrap()
            .contains(&input["modelId"])
    {
        return Err(ContractError::new(
            "INVALID_INPUT",
            "/modelId",
            "model or capability does not match frozen declaration",
        ));
    }
    let op = c["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|op| op["id"] == input["operationId"] && op["version"] == input["operationVersion"])
        .ok_or_else(|| {
            ContractError::new("INVALID_INPUT", "/operationId", "unknown operation version")
        })?
        .clone();
    if input["inputSchemaDigest"] != op["inputSchema"]["digest"] {
        return Err(ContractError::new(
            "INVALID_INPUT",
            "/inputSchemaDigest",
            "input schema changed",
        ));
    }
    if crate::operation_input::uses_profile(&op) {
        crate::operation_input::validate_value_budget(&input["input"], "/input")?;
    } else if input.get("preparation").is_some() || input.get("environmentRevisionDigest").is_some() {
        return Err(ContractError::new("INVALID_INPUT", "/preparation", "selected operation does not declare the operation input profile"));
    }
    r.validate(op["inputSchema"]["id"].as_str().unwrap(), &input["input"])?;
    r.validate(
        c["configurationSchema"]["id"].as_str().unwrap(),
        &input["configuration"],
    )?;
    let contract_digest = hash_bytes(&serde_json::to_vec(&c).unwrap());
    Ok(ValidatedOperationInput {
        video_speech: false,
        image_projection: false,
        media_projection: false,
        contract: c,
        operation: op,
        request: input,
        registry: r,
        transport_operation: "submit".into(),
        contract_digest,
    })
}

/// Presentation diagnostics never determine whether a legal operation can run.
pub fn ui_diagnostics(contract: &CapabilityContractV2) -> Vec<ContractError> {
    if let Err(error) = validate_contract(contract) { return vec![error]; }
    let c = match as_value(contract) { Ok(v) => v, Err(e) => return vec![e] };
    let r = match registry(&c) { Ok(v) => v, Err(e) => return vec![e] };
    c["operations"].as_array().into_iter().flatten()
        .filter_map(|op| validate_controls(&r, op).err()).collect()
}
fn validate_controls(r: &SchemaRegistry, op: &Value) -> Result<(), ContractError> {
    let id = op["inputSchema"]["id"].as_str().unwrap();
    let schema = r.schema(id).unwrap();
        let mut pointers = std::collections::BTreeSet::new();
        for control in op["uiSchema"]["controls"].as_array().unwrap() {
            let pointer = control["fieldPointer"].as_str().unwrap();
            let field = r.input_field_schema(id, pointer)?;
            let mut read_only = field["readOnly"] == true;
            for (end, _) in pointer.match_indices('/').skip(1) {
                read_only |= r.input_field_schema(id, &pointer[..end])?["readOnly"] == true;
            }
            if read_only {
                return Err(ContractError::new(
                    "INVALID_PACKAGE",
                    pointer,
                    "read-only input cannot be supplied by an editable control",
                ));
            }
            if !pointers.insert(pointer.to_owned()) {
                return Err(ContractError::new(
                    "INVALID_PACKAGE",
                    pointer,
                    "duplicate or unresolved UI field pointer",
                ));
            }
            let compatible = match control["widget"].as_str().unwrap() {
                "text" | "multiline" => accepts_type(&field, "string"),
                "number" => accepts_type(&field, "number") || accepts_type(&field, "integer"),
                "integer" => accepts_type(&field, "integer"),
                "toggle" | "checkbox" => accepts_type(&field, "boolean"),
                "select" => field.get("enum").is_some_and(Value::is_array) || op["uiSchema"]["dynamicOptions"].as_array().is_some_and(|fields| fields.iter().any(|f| f["fieldPointer"] == pointer)),
                "object" => accepts_type(&field, "object"),
                "repeat" => accepts_type(&field, "array")
                    && (field.get("items").is_some() || field.get("prefixItems").is_some()),
                "media" => op["mediaSlots"].as_array().unwrap().iter()
                    .any(|slot| slot["inputPointer"] == pointer),
                "time_span" | "point" | "box" | "mask" => accepts_type(&field, "object"),
                _ => false,
            };
            if !compatible {
                return Err(ContractError::new(
                    "UNSUPPORTED_FEATURE",
                    pointer,
                    "declared control cannot render the field type",
                ));
            }
        }
        for slot in op["mediaSlots"].as_array().unwrap() {
            let pointer = slot["inputPointer"].as_str().unwrap();
            r.input_field_schema(id, pointer)?;
            pointers.insert(pointer.to_owned());
        }
        required_controls(
            r,
            id,
            schema,
            "",
            &pointers,
        )?;
    Ok(())
}
fn validate_standard_profile(op: &Value) -> Result<(), ContractError> {
    let Some(id) = op["id"].as_str().filter(|id| id.starts_with("std/")) else { return Ok(()); };
    let protocol = crate::identity::protocol();
    let published = protocol["standardProfiles"].as_array().unwrap().iter()
        .find(|p| p["operationId"] == id && p["operationVersion"] == op["version"])
        .ok_or_else(|| ContractError::new("INVALID_PACKAGE", id, "standard operation version is not published"))?;
    for field in ["inputSchema", "outputSchema", "resumeSchema"] {
        if op[field] != published[field] {
            return Err(ContractError::new("INVALID_PACKAGE", id, "standard operation schema differs from the published identity"));
        }
    }
    Ok(())
}
