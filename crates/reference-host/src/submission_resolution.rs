//! The original Session remains the only authority to turn an unknown submission into a query.
use super::*;
use bgx_adapter_v2_validator::submission_resolution::{validate_result, FEATURE};

impl Session {
    pub fn submission_probe_request(&self, kind: &str) -> Result<Value> {
        if !self.features.contains(FEATURE) {
            return Err(fail("UNSUPPORTED_FEATURE", "Pinned package has no online submission resolution contract"));
        }
        let expected_state = match kind {
            "submission_identity" => "prepared",
            "submission_resolution" => "acceptance_unknown",
            _ => return Err(invalid()),
        };
        if self.checkpoint["state"] != expected_state {
            return Err(fail("CHECKPOINT_CONFLICT", "Submission probe requires its original checkpoint state"));
        }
        let mut request = json!({"schemaVersion":2,"kind":kind,
            "requestId":self.checkpoint["requestId"],"requestDigest":self.checkpoint["requestDigest"],
            "expectedRevision":self.checkpoint["revision"],"package":self.checkpoint["package"],
            "bindingId":self.checkpoint["bindingId"],"configuration":self.frozen.request["configuration"]});
        if kind == "submission_resolution" {
            if self.checkpoint.get("submissionScope").is_none() {
                return Err(fail("ACCEPTANCE_UNKNOWN", "Original submission has no frozen durable service scope; a replacement store cannot resolve it"));
            }
            request["submissionScope"] = self.checkpoint["submissionScope"].clone();
        }
        Ok(request)
    }

    pub fn bind_submission_scope(&mut self, store: &mut CheckpointStore, request: &Value, result: &Value) -> Result<()> {
        if request != &self.submission_probe_request("submission_identity")? || self.checkpoint.get("submissionScope").is_some() {
            return Err(fail("CHECKPOINT_CONFLICT", "Submission scope is already frozen or request changed"));
        }
        validate_result(request, result).map_err(|_| invalid())?;
        let mut next = self.next()?;
        next["submissionScope"] = result["submissionScope"].clone();
        self.commit(store, next)
    }

    pub fn apply_submission_resolution(&mut self, store: &mut CheckpointStore, request: &Value, result: &Value,
        media: &mut MediaBroker, scope: &LeaseScope, control: &Control) -> Result<()> {
        if request != &self.submission_probe_request("submission_resolution")? {
            return Err(fail("CHECKPOINT_CONFLICT", "Resolution request no longer matches the frozen checkpoint"));
        }
        validate_result(request, result).map_err(|_| invalid())?;
        match result["status"].as_str() {
            Some("unknown") => Ok(()),
            Some("rejected") => {
                let next = bgx_adapter_v2_ports::wire::submission_rejection::checkpoint_candidate(
                    &self.checkpoint, "service_rejected", "Original request durably fenced by its original service store", result,
                ).map_err(|_| invalid())?;
                self.commit(store, next)
            }
            Some("accepted") => {
                let accepted = &result["accepted"];
                let mut next = self.next()?;
                next["state"] = json!("accepted");
                next["vendorTaskId"] = accepted["vendorTaskId"].clone();
                next["resumeToken"] = accepted["resumeToken"].clone();
                next["result"] = accepted.clone();
                validate(&next)?;
                // Query construction revalidates the accepted token/result against the
                // exact original contract and rehydrates only retained original media.
                let original = std::mem::replace(&mut self.checkpoint, next.clone());
                let checked = self.query_context(media, scope, control);
                self.checkpoint = original;
                checked?;
                self.commit(store, next)
            }
            _ => Err(invalid()),
        }
    }
}
