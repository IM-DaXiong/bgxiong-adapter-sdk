//! Validation for the negotiated frame selection and trajectory correction profile.
use crate::{ContractError, SchemaRegistry};
use serde_json::Value;
use std::collections::BTreeSet;

pub const FEATURE: &str = "media-target-range-repair-v1";

#[derive(Clone, Copy)]
pub enum Message {
    Declaration,
    Seed,
    Trajectory,
    Patch,
    Apply,
    Freeze,
}

impl Message {
    fn name(self) -> &'static str {
        match self {
            Self::Declaration => "declaration",
            Self::Seed => "seed",
            Self::Trajectory => "trajectory",
            Self::Patch => "patch",
            Self::Apply => "apply",
            Self::Freeze => "freeze",
        }
    }
}

fn invalid(path: &str, reason: &str) -> ContractError {
    ContractError::new("INVALID_INPUT", path, reason)
}

fn box_inside(value: &Value, path: &str) -> Result<(), ContractError> {
    let x = value["x"].as_f64().unwrap_or(f64::INFINITY);
    let y = value["y"].as_f64().unwrap_or(f64::INFINITY);
    let w = value["width"].as_f64().unwrap_or(0.0);
    let h = value["height"].as_f64().unwrap_or(0.0);
    if x < 0.0 || y < 0.0 || w <= 0.0 || h <= 0.0 || x + w > 1.0 || y + h > 1.0 {
        return Err(invalid(path, "box must lie inside the displayed frame"));
    }
    Ok(())
}

pub fn validate(message: Message, value: &Value) -> Result<(), ContractError> {
    let schema = format!(
        "https://bgxiong.local/vendor-adapter/v2/profiles/{FEATURE}/{}.schema.json",
        message.name()
    );
    SchemaRegistry::public()?.validate(&schema, value)?;
    if value["kind"] == "seedRequest" {
        let start = value["startMs"].as_u64().unwrap();
        let end = value["endMs"].as_u64().unwrap();
        let time = value["timeUs"].as_u64().unwrap() / 1000;
        if !(start..end).contains(&time) {
            return Err(invalid("/timeUs", "playhead must be inside the selection"));
        }
    }
    if let Some(candidates) = value["candidates"].as_array() {
        let mut candidate_ids = BTreeSet::new();
        for candidate in candidates {
            box_inside(&candidate["box"], "/candidates/box")?;
            if !candidate_ids.insert(candidate["candidateId"].as_str().unwrap()) {
                return Err(invalid("/candidates/candidateId", "duplicate candidate"));
            }
        }
    }
    if let Some(samples) = value["samples"].as_array() {
        let mut media_ids = BTreeSet::new();
        for sample in samples {
            if !media_ids.insert(sample["mediaId"].as_str().unwrap()) {
                return Err(invalid("/samples/mediaId", "duplicate sample"));
            }
        }
    }
    if let Some(anchor) = value.get("anchor") {
        box_inside(&anchor["box"], "/anchor/box")?;
    }
    if let Some(range) = value.get("range") {
        let start = range["startFrame"].as_u64().unwrap();
        let end = range["endFrame"].as_u64().unwrap();
        if start >= end {
            return Err(invalid("/range", "range must contain at least one frame"));
        }
        if let Some(anchor) = value["anchorFrame"].as_u64() {
            if !(start..end).contains(&anchor) {
                return Err(invalid(
                    "/anchorFrame",
                    "repair anchor outside repair range",
                ));
            }
        }
        if let Some(runs) = value["runs"].as_array() {
            let mut next = start;
            for run in runs {
                let run_start = run["startFrame"].as_u64().unwrap();
                let run_end = run["endFrame"].as_u64().unwrap();
                if next != run_start || run_start >= run_end || run_end > end {
                    return Err(invalid("/runs", "runs must partition the requested range"));
                }
                next = run_end;
            }
            if next != end {
                return Err(invalid("/runs", "tracking coverage is incomplete"));
            }
        }
    }
    Ok(())
}

pub fn validate_anchor(seed: &Value, anchor: &Value) -> Result<(), ContractError> {
    validate(Message::Seed, seed)?;
    if seed["kind"] != "seedReceipt"
        || anchor.get("frame") != seed.get("frame")
        || anchor.get("imageDigest") != seed.get("imageDigest")
    {
        return Err(invalid(
            "/anchor",
            "anchor does not bind the presented frame",
        ));
    }
    box_inside(&anchor["box"], "/anchor/box")?;
    if let Some(id) = anchor.get("candidateId") {
        if !seed["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|candidate| candidate.get("candidateId") == Some(id))
        {
            return Err(invalid("/anchor/candidateId", "unknown candidate"));
        }
    }
    Ok(())
}

pub fn validate_patch_parent(parent: &Value, patch: &Value) -> Result<(), ContractError> {
    validate(Message::Trajectory, parent)?;
    validate(Message::Patch, patch)?;
    if parent["kind"] != "trajectoryReceipt"
        || patch["binding"] != parent["binding"]
        || patch["parent"] != parent["revision"]
        || patch["range"]["startFrame"].as_u64() < parent["range"]["startFrame"].as_u64()
        || patch["range"]["endFrame"].as_u64() > parent["range"]["endFrame"].as_u64()
    {
        return Err(invalid(
            "/parent",
            "patch does not bind the exact confirmed trajectory",
        ));
    }
    Ok(())
}
