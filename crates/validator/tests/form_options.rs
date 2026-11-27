use bgx_adapter_v2_sdk::{form_options_port_allowed, CapabilityContractV2};
use bgx_adapter_v2_validator::{
    capabilities::{Binding, CapabilityRegistry},
    form_options::{self, FEATURE, KIND},
    hash_bytes,
    package::{verify_package, PackageContents, TrustContext, VerifiedPackage},
    validate_contract,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn fixture() -> Value {
    serde_json::from_str::<Value>(include_str!(
        "../../../examples/form-options/capabilities.json"
    ))
    .unwrap()
}

fn package() -> VerifiedPackage {
    let mut manifest: Value =
        serde_json::from_str(include_str!("../../../examples/form-options/manifest.json")).unwrap();
    manifest["artifactSha256"] = json!(hash_bytes(b"component"));
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let files = BTreeMap::from([
        ("manifest.json".into(), hash_bytes(&bytes)),
        ("adapter.wasm".into(), hash_bytes(b"component")),
    ]);
    let inventory = files
        .iter()
        .map(|(k, v)| format!("{v} {k}\n"))
        .collect::<String>();
    let contents = PackageContents {
        manifest_bytes: &bytes,
        hash_manifest_bytes: inventory.as_bytes(),
        signature: None,
        measured_file_hashes: &files,
    };
    let mut trust = TrustContext::default();
    assert!(verify_package(&contents, &trust).is_err());
    trust.supported_features.insert(FEATURE.into());
    verify_package(&contents, &trust).unwrap()
}

#[test]
fn validates_registered_selection_and_complete_results() {
    let package = package();
    let binding = Binding::new("fixture".into(), &package, 1, json!({})).unwrap();
    let mut catalog = CapabilityRegistry::default();
    let mut contract = fixture()["capabilityContracts"][0].clone();
    let mut schema: Value =
        serde_json::from_str(contract["schemas"][0]["schemaJson"].as_str().unwrap()).unwrap();
    schema["properties"]["palette"]["enum"] = json!(["ink", "chalk"]);
    let source = schema.to_string();
    let digest = hash_bytes(source.as_bytes());
    contract["schemas"][0]["schemaJson"] = json!(source);
    contract["schemas"][0]["digest"] = json!(digest);
    contract["operations"][0]["inputSchema"]["digest"] = json!(digest);
    let refs = catalog
        .register(
            &package,
            &binding,
            &[serde_json::from_value(contract).unwrap()],
        )
        .unwrap();
    let r = &refs[0];
    let req = json!({"schemaVersion":2,"kind":KIND,"modelId":r.model_id,"capabilityKey":r.capability_key,
        "operationId":r.operation_id,"operationVersion":r.operation_version,"inputSchemaDigest":r.input_schema_digest,"configuration":{}});
    let op = form_options::selected_operation(&catalog, r, &binding, &package, &req).unwrap();
    for key in [
        "modelId",
        "capabilityKey",
        "operationId",
        "operationVersion",
        "inputSchemaDigest",
        "configuration",
    ] {
        let mut wrong = req.clone();
        wrong[key] = json!("changed");
        assert!(
            form_options::selected_operation(&catalog, r, &binding, &package, &wrong).is_err(),
            "{key}"
        );
    }
    let field = json!({"fieldPointer":"/palette","choices":["ink","chalk"]});
    let unavailable = json!({"fieldPointer":"/palette","choices":[],"reason":"Service offline"});
    for valid in [field.clone(), unavailable] {
        form_options::validate_fields(&op, &json!({"kind":KIND,"fields":[valid]})).unwrap();
    }
    for invalid in [
        json!([]),
        json!([field.clone(), field]),
        json!([{"fieldPointer":"/other","choices":["ink"]}]),
        json!([{"fieldPointer":"/palette","choices":[]}]),
        json!([{"fieldPointer":"/palette","choices":["ink","ink"]}]),
        json!([{"fieldPointer":"/palette","choices":["outside-enum"]}]),
        json!([{"fieldPointer":"/palette","choices":["ink"],"reason":" "}]),
    ] {
        assert!(
            form_options::validate_fields(&op, &json!({"kind":KIND,"fields":invalid})).is_err()
        );
    }
}

#[test]
fn rejects_undeclared_noneditable_and_duplicate_fields() {
    for wrong in [
        json!([{"fieldPointer":"/colorize"}]),
        json!([{"fieldPointer":"/other"}]),
        json!([{"fieldPointer":"/palette","enabledBy":"/palette"}]),
        json!([{"fieldPointer":"/palette"},{"fieldPointer":"/palette"}]),
    ] {
        let mut c = fixture()["capabilityContracts"][0].clone();
        c["operations"][0]["uiSchema"]["dynamicOptions"] = wrong;
        let contract: CapabilityContractV2 = serde_json::from_value(c).unwrap();
        assert!(validate_contract(&contract).is_err());
    }
}

#[test]
fn lookup_ports_only_allow_logs_and_read_only_bounded_http() {
    let valid = json!({"method":"host-http.execute","args":{"plan":{"method":"GET","body":{"kind":"none"},"sink":{"kind":"buffer","maxBytes":65536}}}});
    assert!(form_options_port_allowed(&valid));
    assert!(form_options_port_allowed(&json!({"method":"host-log.log"})));
    for method in [
        "host-http-stream.open",
        "host-websocket.connect",
        "host-media.read",
        "host-credentials.sign",
        "host-state.put",
        "host-events.emit",
    ] {
        let mut call = valid.clone();
        call["method"] = json!(method);
        assert!(!form_options_port_allowed(&call));
    }
    for (path, value) in [
        ("/args/plan/method", json!("POST")),
        ("/args/plan/body/kind", json!("media")),
        ("/args/plan/sink/kind", json!("download")),
        ("/args/plan/sink/maxBytes", json!(4194305)),
    ] {
        let mut call = valid.clone();
        *call.pointer_mut(path).unwrap() = value;
        assert!(!form_options_port_allowed(&call));
    }
}
