//! Source-only coverage added during the packaging freeze. NOT_RUN.
use bgx_adapter_v2_validator::{operation_preflight, SchemaRegistry};
use serde_json::{json, Value};

fn request() -> Value {
    json!({"schemaVersion":2,"kind":"operation_preflight","environmentReady":true,"modelId":"model",
        "capabilityKey":"video.speech","operationId":"local.example/speech","operationVersion":1,
        "inputSchemaDigest":"a".repeat(64),"requestDigest":"b".repeat(64),"configuration":{},
        "input":{"schemaVersion":1,"portrait":[{"mediaId":"portrait"}],"drivingAudio":[{"mediaId":"drive"}],
            "audio":{"durationMs":2000,"sampleRateHz":24000,"channels":1,"sha256":"c".repeat(64)},
            "instruction":"","parameters":{"steps":20}},
        "mediaMetadata":[
            {"mediaId":"portrait","kind":"image","mime":"image/png","sha256":"d".repeat(64),"width":512,"height":512},
            {"mediaId":"drive","kind":"audio","mime":"audio/wav","sha256":"c".repeat(64),
             "durationMs":2000,"sampleRateHz":24000,"channels":1,
             "sourceInterval":{"startMs":1000,"endMs":3000},"sourceDurationMs":5000}]
    })
}
fn ready() -> Value {
    json!({"kind":"operation_preflight","status":"ready","requestDigest":"b".repeat(64),
        "specification":{"width":512,"height":512,"fps":25,"frameCount":50,
            "audioDurationMs":2000,"videoDurationMs":2000,"maxTailDifferenceMs":0,
            "audioPolicy":"preserve_driving_audio"}})
}

#[test]
fn ready_preserves_prepared_segment_and_binds_response_digest() {
    let input = request();
    assert!(operation_preflight::validate_result(&input, &ready()).is_ok());
    let mut wrong = ready();
    wrong["requestDigest"] = json!("e".repeat(64));
    assert!(operation_preflight::validate_result(&input, &wrong).is_err());
    let mut wrong = ready();
    wrong["specification"]["audioDurationMs"] = json!(1500);
    wrong["specification"]["maxTailDifferenceMs"] = json!(5000);
    // A permissive container-tail limit cannot excuse loss of driving audio.
    assert!(operation_preflight::validate_result(&input, &wrong).is_err());
}

#[test]
fn suggestions_require_valid_original_source_bounds() {
    let mut result = json!({"kind":"operation_preflight","status":"adjustment_required",
        "requestDigest":"b".repeat(64),"reasons":[{"code":"INPUT_CONSTRAINT","message":"Select a shorter interval"}],
        "candidateIntervals":[{"startMs":0,"endMs":4000}]});
    assert!(operation_preflight::validate_result(&request(), &result).is_ok());
    result["candidateIntervals"][0]["endMs"] = json!(5001);
    assert!(operation_preflight::validate_result(&request(), &result).is_err());
    result["candidateIntervals"][0] = json!({"startMs":2000,"endMs":1000});
    assert!(operation_preflight::validate_result(&request(), &result).is_err());
}

#[test]
fn blocked_cannot_smuggle_ready_specification() {
    let mut result = json!({"kind":"operation_preflight","status":"blocked","requestDigest":"b".repeat(64),
        "reasons":[{"code":"RESOURCE_UNAVAILABLE","message":"Dependencies have not been verified"}]});
    assert!(operation_preflight::validate_result(&request(), &result).is_ok());
    result["specification"] = ready()["specification"].clone();
    assert!(operation_preflight::validate_result(&request(), &result).is_err());
}

#[test]
fn unknown_dependencies_cannot_be_certified_ready() {
    let mut input = request();
    input["environmentReady"] = json!(false);
    assert!(operation_preflight::validate_result(&input, &ready()).is_err());
}

#[test]
fn metadata_and_actual_interval_must_match_prepared_input() {
    let mut input = request();
    input["mediaMetadata"][1]["durationMs"] = json!(3000);
    assert!(operation_preflight::validate_result(&input, &ready()).is_err());
    let mut input = request();
    input["mediaMetadata"][1]["sourceInterval"]["startMs"] = json!(0);
    assert!(operation_preflight::validate_result(&input, &ready()).is_err());
}

#[test]
fn legacy_probe_inputs_remain_closed_and_distinct() {
    let registry = SchemaRegistry::public().unwrap();
    let mut old = json!({"schemaVersion":2,"kind":"connectivity","configuration":{}});
    assert!(registry.validate_dispatch("probe", "input", &old).is_ok());
    old["input"] = json!({});
    assert!(registry.validate_dispatch("probe", "input", &old).is_err());
    assert!(registry.validate_dispatch("probe", "input", &request()).is_ok());
    let mut input = request();
    input["mediaMetadata"][0]["handle"] = json!("local-file-handle");
    assert!(registry.validate_dispatch("probe", "input", &input).is_err());
}

#[test]
fn generic_wire_examples_round_trip_and_bind_results() {
    use bgx_adapter_v2_sdk::{OperationInputV1Draft, OperationInputV1Request};
    use bgx_adapter_v2_validator::operation_input;

    let example: Value = serde_json::from_str(include_str!("../../../profiles/operation-input-v1/examples.json")).unwrap();
    let draft: OperationInputV1Draft = serde_json::from_value(example["draft"].clone()).unwrap();
    let request: OperationInputV1Request = serde_json::from_value(example["request"].clone()).unwrap();
    assert_eq!(serde_json::to_value(draft).unwrap(), example["draft"]);
    assert_eq!(serde_json::to_value(request).unwrap(), example["request"]);

    let registry = SchemaRegistry::public().unwrap();
    registry.validate_dispatch("probe", "input", &example["request"]).unwrap();
    for result in [&example["ready"], &example["rejected"]] {
        registry.validate_dispatch("probe", "result", result).unwrap();
        operation_input::validate_result(&example["request"], result).unwrap();
    }
    let prepared = operation_input::seal_result(&example["request"], &example["ready"]).unwrap().unwrap();
    assert_eq!(prepared["material"]["effectiveSeed"], example["draft"]["input"]["seed"]);
    assert!(operation_input::seal_result(&example["request"], &example["rejected"]).unwrap().is_none());

    let mut changed = example["request"].clone();
    changed["input"]["motion"]["keepAudio"] = json!(true);
    assert_eq!(operation_input::validate_result(&changed, &example["ready"]).unwrap_err().code, "PREPARATION_STALE");
}
