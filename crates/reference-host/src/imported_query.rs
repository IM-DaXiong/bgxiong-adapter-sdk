//! Query authority for an explicitly reviewed task recovered from an immutable
//! historical request. The reviewed task does not carry a submit permit.
use crate::{
    checkpoint::{fail, validate, Result},
    media::{LeaseScope, MediaBroker},
};
use bgx_adapter_v2_ports::{
    resource_policy,
    wire::{CapabilityContractV2, OperationTaskRefV2},
};
use bgx_adapter_v2_validator::{
    capabilities::{Binding, CapabilityRegistry},
    identity::PackageIdentity,
    ValidatedOperationInput,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewEvidence {
    pub reference: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum ReviewAuthority {
    OperatorReviewed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OriginalTaskReview {
    pub schema_version: u32,
    pub authority: ReviewAuthority,
    pub reviewed_by: String,
    pub original_request_id: String,
    pub original_request_digest: String,
    pub account_id: String,
    pub vendor_task_id: String,
    pub resume_token: Value,
    pub evidence: Vec<ReviewEvidence>,
}

/// The embedding host must read the account from its persisted request and
/// authorize the reviewer before supplying this input. Credentials stay in the vault.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportedQuery {
    pub original_checkpoint: Value,
    pub original_frozen: Value,
    pub original_account_id: String,
    pub current_account_id: String,
    pub review: OriginalTaskReview,
}

fn denied() -> crate::checkpoint::CheckpointError {
    fail(
        "PERMISSION_DENIED",
        "Reviewed query does not match the immutable original request, account or operation",
    )
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

impl ImportedQuery {
    /// Offline validation. Certifying that the evidence uniquely identifies a
    /// remote acceptance is the explicitly authorized reviewer's responsibility.
    pub fn validate_original(&self, package: &PackageIdentity, binding: &Binding) -> Result<()> {
        let limit = resource_policy()["limits"]["maxStateBytes"]
            .as_u64()
            .unwrap() as usize;
        let checkpoint = &self.original_checkpoint;
        let frozen = &self.original_frozen;
        let review = &self.review;
        if [&self.original_checkpoint, &self.original_frozen]
            .iter()
            .any(|v| serde_json::to_vec(v).map_or(true, |bytes| bytes.len() > limit))
            || serde_json::to_vec(review).map_or(true, |bytes| bytes.len() > limit)
        {
            return Err(fail(
                "BUDGET_EXCEEDED",
                "Reviewed original task exceeds its evidence budget",
            ));
        }
        validate(checkpoint)?;
        crate::session::validate_frozen_checkpoint(checkpoint, frozen)?;
        if review.schema_version != 1
            || review.reviewed_by.trim().is_empty()
            || review.vendor_task_id.trim().is_empty()
            || review.vendor_task_id.chars().any(char::is_control)
            || review.evidence.is_empty()
            || review
                .evidence
                .iter()
                .any(|item| item.reference.trim().is_empty() || !is_sha256(&item.sha256))
            || checkpoint["state"] != "acceptance_unknown"
            || !checkpoint["vendorTaskId"].is_null()
            || checkpoint["requestId"] != review.original_request_id
            || checkpoint["requestDigest"] != review.original_request_digest
            || self.original_account_id.trim().is_empty()
            || self.original_account_id != review.account_id
            || self.current_account_id != review.account_id
            || checkpoint["package"]["pluginId"] != package.plugin_id
            || binding.package() != package
            || frozen["request"]["configuration"] != *binding.configuration()
        {
            return Err(denied());
        }
        Ok(())
    }

    pub(crate) fn prepare(
        &self,
        package: &bgx_adapter_v2_validator::package::VerifiedPackage,
        binding: &Binding,
        catalog: &CapabilityRegistry,
        root: &str,
        transport_slot: &Value,
        payload: &Value,
    ) -> Result<ValidatedOperationInput> {
        let checkpoint = &self.original_checkpoint;
        let frozen = &self.original_frozen;
        if root != self.review.original_request_id
            || payload["vendorTaskId"] != self.review.vendor_task_id
            || payload["resumeToken"] != self.review.resume_token
        {
            return Err(denied());
        }
        self.validate_original(package.identity(), binding)?;
        let selected = catalog
            .list(None)
            .into_iter()
            .filter(|row| {
                let r = &row.reference;
                payload["modelId"] == r.model_id
                    && payload["capabilityKey"] == r.capability_key
                    && payload["operationId"] == r.operation_id
                    && payload["operationVersion"] == r.operation_version
            })
            .map(|row| row.reference)
            .collect::<Vec<_>>();
        let [selected] = selected.as_slice() else {
            return Err(denied());
        };
        let contract = catalog
            .contract_for(selected, binding)
            .map_err(|_| denied())?;
        let next = serde_json::to_value(contract).map_err(|_| denied())?;
        let operation = next["operations"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|op| {
                op["id"] == selected.operation_id && op["version"] == selected.operation_version
            })
            .ok_or_else(denied)?;
        let original = frozen["contract"]["operations"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|op| {
                op["id"] == checkpoint["operationId"]
                    && op["version"] == checkpoint["operationVersion"]
            })
            .ok_or_else(denied)?;
        if payload["modelId"] != checkpoint["modelId"]
            || payload["capabilityKey"] != checkpoint["capabilityKey"]
            || payload["operationId"] != checkpoint["operationId"]
            || payload["configuration"] != *binding.configuration()
            || &next["transportSlot"] != transport_slot
            || next["transportSlot"] != frozen["contract"]["transportSlot"]
            || operation["execution"]["mode"] != "asynchronous"
            || operation["execution"]["resumable"] != true
            || operation["outputSchema"]["digest"] != checkpoint["outputSchemaDigest"]
            || operation["resumeSchema"]
                .get("digest")
                .unwrap_or(&Value::Null)
                != &checkpoint["resumeSchemaDigest"]
            || operation["outputSlots"] != original["outputSlots"]
        {
            return Err(denied());
        }
        let declaration: CapabilityContractV2 =
            serde_json::from_value(frozen["contract"].clone()).map_err(|_| denied())?;
        let mut original_reference = payload.clone();
        original_reference["operationVersion"] = checkpoint["operationVersion"].clone();
        let task: OperationTaskRefV2 =
            serde_json::from_value(original_reference).map_err(|_| denied())?;
        bgx_adapter_v2_validator::result::validate_retained_query(
            &declaration,
            &frozen["request"],
            &task,
            &self.review.vendor_task_id,
            package.enabled_features(),
        )
        .map_err(|_| denied())
    }

    pub(crate) fn validate_outputs(
        &self,
        data: &Value,
        context: &ValidatedOperationInput,
        media: &MediaBroker,
        scope: &LeaseScope,
    ) -> Result<()> {
        if data["resumeToken"] != self.review.resume_token {
            return Err(denied());
        }
        let leases = media.validation_context(scope);
        for output in data["outputs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|v| v["kind"] == "media")
        {
            let facts =
                bgx_adapter_v2_validator::media::validate_leased_media(&output["media"], &leases)
                    .map_err(|_| denied())?;
            if facts.kind == "image" {
                bgx_adapter_v2_validator::image_projection::validate_geometry(
                    context,
                    facts.width,
                    facts.height,
                )
                .map_err(|_| denied())?;
            }
            bgx_adapter_v2_validator::media_projection::validate_geometry(
                context,
                &facts.kind,
                facts.width,
                facts.height,
            )
            .map_err(|_| denied())?;
        }
        Ok(())
    }
}
