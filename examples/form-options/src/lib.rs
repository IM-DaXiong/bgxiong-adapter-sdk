#![cfg_attr(target_arch = "wasm32", no_std)]
extern crate alloc;
use bgx_adapter_v2_sdk::{guest, Adapter, Host, Invocation, Response};
use serde_json::{json, Value};

struct Palette;
impl Adapter for Palette {
    fn invoke(request: Invocation, _: &mut dyn Host) -> Response {
        let transport = serde_json::to_value(&request.r#operation).unwrap();
        let payload: Value = serde_json::from_str(&request.r#payload_json).unwrap();
        let result = match transport.as_str() {
            Some("capabilities") => {
                serde_json::from_str(include_str!("../capabilities.json")).unwrap()
            }
            Some("probe") if payload["kind"] == "operation_form_options" => json!({
                "kind":"operation_form_options","fields":[{"fieldPointer":"/palette","choices":["ink","chalk"]}]
            }),
            Some("probe") => json!({"packageValid":true,"connection":"not_checked",
                "authentication":"not_checked","resources":"not_checked","issues":[]}),
            Some("submit") => {
                let text = payload["input"]["palette"].as_str().unwrap_or("ink");
                json!({"kind":"completed","vendorTaskId":null,"appliedParams":null,
                    "outputs":[{"kind":"text","role":"answer","text":text}],"data":{"text":text},"usage":null})
            }
            _ => {
                return serde_json::from_value(
                    json!({"protocolVersion":2,"operation":request.r#operation,
                "error":{"code":"UNSUPPORTED_FEATURE","message":"Unsupported palette operation",
                "retryable":false,"vendorCode":null,"vendorRequestId":null}}),
                )
                .unwrap()
            }
        };
        guest::completed_response(&request, &result).unwrap()
    }
}
bgx_adapter_v2_sdk::export_adapter!(Palette);
