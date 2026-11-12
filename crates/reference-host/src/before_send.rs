//! Awaitable, vendor-neutral outbound persistence. No model-specific text mapping.
use crate::{
    credential::CredentialVault,
    http::MediaAccess,
    policy::{error, Result},
};
use bgx_adapter_v2_transport::Control;
use bgx_adapter_v2_validator::hash_bytes;
use serde_json::{json, Value};
use std::{future::Future, pin::Pin};

pub type PersistFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
pub trait BeforeSendSink: Send + Sync {
    /// Resolve successfully only after durable persistence. Failure prevents HTTP.
    fn persist<'a>(&'a self, record: &'a Value) -> PersistFuture<'a>;
}

// Serde's Value sorts object keys. Walk the original JSON stream separately so
// the audit text follows the submitted field/array order, without key heuristics.
struct Texts<'a>(&'a mut Vec<String>);
impl<'de> serde::de::DeserializeSeed<'de> for Texts<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> std::result::Result<(), D::Error> {
        deserializer.deserialize_any(self)
    }
}
impl<'de> serde::de::Visitor<'de> for Texts<'_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("JSON value")
    }
    fn visit_str<E: serde::de::Error>(self, value: &str) -> std::result::Result<(), E> {
        self.0.push(value.into());
        Ok(())
    }
    fn visit_bool<E: serde::de::Error>(self, _: bool) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: serde::de::Error>(self, _: i64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: serde::de::Error>(self, _: u64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: serde::de::Error>(self, _: f64) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: serde::de::Error>(self) -> std::result::Result<(), E> {
        Ok(())
    }
    fn visit_seq<A: serde::de::SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> std::result::Result<(), A::Error> {
        while seq.next_element_seed(Texts(self.0))?.is_some() {}
        Ok(())
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(
        self,
        mut map: A,
    ) -> std::result::Result<(), A::Error> {
        while map.next_key::<String>()?.is_some() {
            map.next_value_seed(Texts(self.0))?;
        }
        Ok(())
    }
}
pub(crate) fn redact_url(raw: &str, vault: &CredentialVault) -> String {
    let Ok(url) = url::Url::parse(raw) else {
        return "[INVALID_URL]".into();
    };
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return "[INVALID_URL]".into();
    }
    redact_parsed_url(url, vault)
}

fn redact_parsed_url(mut url: url::Url, vault: &CredentialVault) -> String {
    url.set_fragment(None);
    if url.set_password(None).is_err() || url.set_username("").is_err() {
        return "[INVALID_URL]".into();
    }
    if url.query().is_some() {
        // Provider-issued signatures need not be present in the credential vault.
        let keys: Vec<_> = url.query_pairs().map(|(key, _)| vault.redact(&key)).collect();
        url.query_pairs_mut().clear().extend_pairs(keys.iter().map(|key| (key, "[REDACTED]")));
    }
    vault.redact(url.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_url_removes_userinfo_fragment_and_query_values() {
        let vault = CredentialVault::new().unwrap();
        let audit = redact_url(
            "https://name:password@example.invalid:8443/media%20file?k%2Bey=secret%2Bvalue&k%2Bey=&flag#private",
            &vault,
        );
        let url = url::Url::parse(&audit).unwrap();
        assert_eq!(url.origin().ascii_serialization(), "https://example.invalid:8443");
        assert_eq!(url.path(), "/media%20file");
        assert!(url.username().is_empty());
        assert!(url.password().is_none());
        assert!(url.fragment().is_none());
        assert_eq!(
            url.query_pairs().into_owned().collect::<Vec<_>>(),
            ["k+ey", "k+ey", "flag"].map(|key| (key.to_owned(), "[REDACTED]".to_owned()))
        );
    }

    #[test]
    fn audit_url_keeps_plain_urls_and_omits_unparseable_input() {
        let vault = CredentialVault::new().unwrap();
        let plain = "https://example.invalid/media";
        assert_eq!(redact_url(plain, &vault), plain);
        assert_eq!(redact_url("bad url?secret=raw", &vault), "[INVALID_URL]");
        assert_eq!(redact_url("data:text/plain,raw", &vault), "[INVALID_URL]");
    }
}

pub(crate) fn redact(value: &mut Value, vault: &CredentialVault) {
    match value {
        Value::String(s) => *s = vault.redact(s),
        Value::Array(values) => values.iter_mut().for_each(|v| redact(v, vault)),
        Value::Object(values) => values.values_mut().for_each(|v| redact(v, vault)),
        _ => {}
    }
}

fn redact_json_url(text: &mut String, vault: &CredentialVault) {
    let Some((scheme, _)) = text.split_once("://") else {
        return;
    };
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
        || text.chars().any(|ch| ch.is_whitespace() || ch.is_control())
    {
        return;
    }
    let Ok(url) = url::Url::parse(text) else {
        return;
    };
    if url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        *text = redact_parsed_url(url, vault);
    }
}

fn redact_json_urls(value: &mut Value, vault: &CredentialVault) {
    match value {
        Value::String(text) => redact_json_url(text, vault),
        Value::Array(values) => values.iter_mut().for_each(|v| redact_json_urls(v, vault)),
        Value::Object(values) => values.values_mut().for_each(|v| redact_json_urls(v, vault)),
        _ => {}
    }
}

pub(crate) struct AuditBody {
    pub value: Value,
    pub text: Vec<String>,
    parsed_json: bool,
}

impl AuditBody {
    pub(crate) fn redact_urls(&mut self, vault: &CredentialVault) {
        if self.parsed_json {
            redact_json_urls(&mut self.value, vault);
            for text in &mut self.text {
                redact_json_url(text, vault);
            }
        }
    }
}

async fn media_description(access: &MediaAccess, handle: &str) -> Result<Value> {
    let handle = handle.to_owned();
    access
        .run(move |broker, scope| {
            let stat = broker.info(scope, &handle)?;
            Ok(json!({"mediaId":stat.r#id,"sha256":stat.r#sha256,"byteLength":stat.r#byte_length}))
        })
        .await
}
/// Capture an audit-only projection. Binary uses measured identity.
pub(crate) async fn body(
    plan: &Value,
    access: &MediaAccess,
    control: &Control,
) -> Result<AuditBody> {
    let body = &plan["body"];
    let mut text = vec![];
    let mut parsed_json = false;
    let value = match body["kind"].as_str() {
        Some("none") => Value::Null,
        Some("bytes") => {
            let bytes: Vec<u8> = serde_json::from_value(body["bytes"].clone())
                .map_err(|_| error("INVALID_INPUT", "Invalid outbound byte body"))?;
            match std::str::from_utf8(&bytes) {
                Ok(s) => match serde_json::from_str::<Value>(s) {
                    Ok(value) => {
                        use serde::de::DeserializeSeed;
                        Texts(&mut text)
                            .deserialize(&mut serde_json::Deserializer::from_str(s))
                            .map_err(|_| {
                                error("INVALID_INPUT", "Cannot project JSON audit text")
                            })?;
                        parsed_json = true;
                        value
                    }
                    Err(_) => {
                        text.push(s.to_owned());
                        Value::String(s.to_owned())
                    }
                },
                Err(_) => json!({"sha256":hash_bytes(&bytes),"byteLength":bytes.len().to_string()}),
            }
        }
        Some("media") => media_description(access, body["handle"].as_str().unwrap()).await?,
        Some("multipart") => {
            let mut parts = vec![];
            for part in body["parts"].as_array().unwrap() {
                if control.is_cancelled() {
                    return Err(error("CANCELLED", "Outbound persistence cancelled"));
                }
                if part["kind"] == "field" {
                    text.push(part["value"].as_str().unwrap().to_owned());
                    parts.push(part.clone());
                } else {
                    parts.push(json!({"name":part["name"],"kind":"media",
                        "media":media_description(access, part["handle"].as_str().unwrap()).await?}));
                }
            }
            json!({"kind":"multipart","parts":parts})
        }
        _ => return Err(error("INVALID_INPUT", "Unknown outbound body")),
    };
    Ok(AuditBody {
        value,
        text,
        parsed_json,
    })
}
