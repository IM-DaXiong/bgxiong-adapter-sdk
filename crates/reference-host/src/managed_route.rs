//! A short-lived embedding-host grant for an original durable local task.
//! This changes transport only; Session still validates the exact frozen binding.
use crate::policy::{error, parse_url, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use url::{Host, Url};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagedRecoveryRoute {
    pub schema_version: u32,
    pub package: Value,
    pub binding_id: String,
    pub request_id: String,
    pub request_digest: String,
    pub expected_revision: String,
    pub submission_scope: Value,
    pub logical_origin: String,
    pub configuration_origin_pointer: String,
    pub physical_origin: String,
    pub physical_instance_id: String,
    pub expires_at_unix_ms: u64,
}

fn denied() -> bgx_adapter_v2_ports::wire::host::HostError {
    error("PERMISSION_DENIED", "Managed recovery route does not authorize this original local task")
}
fn origin(raw: &str) -> Result<Url> {
    let url = parse_url(raw)?;
    let loopback = match url.host() {
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        _ => false,
    };
    if !loopback || url.scheme() != "http" || url.port().is_none()
        || url.path() != "/" || url.query().is_some()
        || raw != url.origin().ascii_serialization()
    { return Err(denied()); }
    Ok(url)
}
impl ManagedRecoveryRoute {
    pub fn check_live(&self) -> Result<()> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_err(|_| denied())?.as_millis();
        if self.schema_version != 1 || self.physical_instance_id.trim().is_empty()
            || self.physical_instance_id.len() > 256
            || self.physical_instance_id.chars().any(char::is_control)
            || u128::from(self.expires_at_unix_ms) <= now
            || u128::from(self.expires_at_unix_ms) > now + 300_000
        { return Err(denied()); }
        origin(&self.logical_origin)?;
        origin(&self.physical_origin)?;
        Ok(())
    }
    pub(crate) fn authorize(&self, invocation: &Value, checkpoint: &Value) -> Result<()> {
        self.check_live()?;
        let payload: Value = serde_json::from_str(invocation["payloadJson"].as_str().ok_or_else(denied)?)
            .map_err(|_| denied())?;
        let allowed = match invocation["operation"].as_str() {
            Some("query" | "cancel") => checkpoint["state"] == "accepted",
            Some("probe") => checkpoint["state"] == "acceptance_unknown"
                && payload["kind"] == "submission_resolution",
            _ => false,
        };
        if !allowed || checkpoint["package"] != self.package
            || checkpoint["bindingId"] != self.binding_id
            || invocation["bindingId"] != self.binding_id
            || checkpoint["requestId"] != self.request_id
            || checkpoint["requestDigest"] != self.request_digest
            || checkpoint["revision"] != self.expected_revision
            || !self.submission_scope.is_object() || self.submission_scope.as_object().is_some_and(|s| s.is_empty())
            || checkpoint["submissionScope"] != self.submission_scope
            || !self.configuration_origin_pointer.starts_with('/')
            || self.configuration_origin_pointer.len() > 2048
            || self.configuration_origin_pointer.replace("~0", "").replace("~1", "").contains('~')
            || payload["configuration"].pointer(&self.configuration_origin_pointer).and_then(Value::as_str) != Some(self.logical_origin.as_str())
        { return Err(denied()); }
        Ok(())
    }
    pub(crate) fn destination(&self, plan: &Value, credential: Option<&str>) -> Result<String> {
        self.check_live()?;
        if credential.is_some() || plan["generatedHeaders"].as_array().is_none_or(|a| !a.is_empty()) {
            return Err(denied());
        }
        for header in plan["headers"].as_array().ok_or_else(denied)? {
            let name = header["name"].as_str().ok_or_else(denied)?;
            if !["content-type", "accept", "range"].iter().any(|n| name.eq_ignore_ascii_case(n)) {
                return Err(denied());
            }
        }
        let raw = plan["url"].as_str().ok_or_else(denied)?;
        let logical = parse_url(raw)?;
        if logical.origin().ascii_serialization() != self.logical_origin { return Err(denied()); }
        // The parsed path/query is exactly the same one the ordinary HTTP owner uses.
        Ok(format!("{}{}", self.physical_origin, &logical[url::Position::BeforePath..]))
    }
    pub(crate) fn audit(&self) -> Value {
        json!({"schemaVersion":1,"package":self.package,"bindingId":self.binding_id,
            "requestId":self.request_id,"requestDigest":self.request_digest,"expectedRevision":self.expected_revision,
            "logicalOrigin":self.logical_origin,"physicalOrigin":self.physical_origin,
            "configurationOriginPointer":self.configuration_origin_pointer,
            "physicalInstanceId":self.physical_instance_id,"expiresAtUnixMs":self.expires_at_unix_ms})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (ManagedRecoveryRoute, Value, Value) {
        let checkpoint = json!({"state":"accepted","package":{"pluginId":"local.fixture","version":"1"},
            "bindingId":"binding","requestId":"original","requestDigest":"d".repeat(64),
            "revision":"r1","submissionScope":{"storeId":"original-store","instanceId":"old-process"}});
        let route = ManagedRecoveryRoute {schema_version:1, package:checkpoint["package"].clone(),
            binding_id:"binding".into(),request_id:"original".into(),request_digest:"d".repeat(64),expected_revision:"r1".into(),
            submission_scope:checkpoint["submissionScope"].clone(),logical_origin:"http://127.0.0.1:12001".into(),configuration_origin_pointer:"/origin".into(),
            physical_origin:"http://127.0.0.1:12002".into(),physical_instance_id:"new-process".into(),
            expires_at_unix_ms:SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64+120_000};
        (route,checkpoint,json!({"operation":"query","bindingId":"binding","payloadJson":json!({"configuration":{"origin":"http://127.0.0.1:12001"}}).to_string()}))
    }
    #[test]
    fn original_identity_revision_scope_and_operation_are_required() {
        let (route, checkpoint, invocation) = fixture();
        route.authorize(&invocation,&checkpoint).unwrap();
        for key in ["package","bindingId","requestId","requestDigest","revision","submissionScope","state"] {
            let mut bad=checkpoint.clone();bad[key]=json!("replacement");
            assert!(route.authorize(&invocation,&bad).is_err(),"{key}");
        }
        for op in ["submit","upload","capabilities","probe"] {
            let mut bad=invocation.clone();bad["operation"]=json!(op);
            assert!(route.authorize(&bad,&checkpoint).is_err(),"{op}");
        }
        let mut unknown=checkpoint.clone();unknown["state"]=json!("acceptance_unknown");
        let probe=json!({"operation":"probe","bindingId":"binding","payloadJson":json!({"kind":"submission_resolution","configuration":{"origin":"http://127.0.0.1:12001"}}).to_string()});
        route.authorize(&probe,&unknown).unwrap();
        let mut wrong=route.clone();wrong.submission_scope["storeId"]=json!("new-store");
        assert!(wrong.authorize(&probe,&unknown).is_err());
        wrong=route.clone();wrong.expires_at_unix_ms=1;
        assert!(wrong.authorize(&invocation,&checkpoint).is_err());
        wrong=route.clone();wrong.configuration_origin_pointer="/missing".into();
        assert!(wrong.authorize(&invocation,&checkpoint).is_err());
    }
    #[test]
    fn transport_preserves_path_query_and_rejects_remote_or_credentials() {
        let (route,_,_)=fixture();
        let plan=json!({"url":"http://127.0.0.1:12001/a%20b/file?offset=2&x=%2F", "headers":[], "generatedHeaders":[]});
        assert_eq!(route.destination(&plan,None).unwrap(),"http://127.0.0.1:12002/a%20b/file?offset=2&x=%2F");
        assert!(route.destination(&plan,Some("credential")).is_err());
        for raw in ["https://127.0.0.1:12002","http://localhost:12002","http://192.168.1.2:12002", "http://127.0.0.1:12002/x", "http://u:p@127.0.0.1:12002", "http://127.0.0.1:12002?q=x"] {
            let mut bad=route.clone();bad.physical_origin=raw.into();assert!(bad.destination(&plan,None).is_err(),"{raw}");
        }
        for name in ["Authorization","Cookie","Host","X-Api-Key"] {
            let mut bad=plan.clone();bad["headers"]=json!([{"name":name,"value":"secret"}]);assert!(route.destination(&bad,None).is_err());
        }
        let mut bad=plan.clone();bad["generatedHeaders"]=json!([{"kind":"bearer"}]);assert!(route.destination(&bad,None).is_err());
        bad=plan;bad["url"]=json!("http://127.0.0.1:12003/a");assert!(route.destination(&bad,None).is_err());
    }
}
