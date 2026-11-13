//! Read-only public projection of a standalone SDK session; applications never parse its store layout.
use bgx_adapter_v2_reference_host::{checkpoint::{CheckpointStore, SnapshotPolicy, SqliteFixtureRows}, media::DevelopmentWorkspace};
use serde_json::{json, Value};
pub fn create(args: &[String]) -> crate::Result<Value> {
    let parsed = crate::Args::parse(args, &["--workspace"], &[])?;
    let root = parsed.path("--workspace")?;
    let workspace = DevelopmentWorkspace::create(&root).map_err(|e|
        crate::error("SESSION_UNAVAILABLE", "/workspace", e.message, "Select a new empty workspace path"))?;
    // Establish the sole checkpoint store before any submission can start. Later
    // read-only inspection distinguishes an absent row from a lost/corrupt store.
    let rows = SqliteFixtureRows::open(&workspace).map_err(|e|
        crate::error("SESSION_UNAVAILABLE", "/session", e.message, "Preserve the failed workspace for diagnosis"))?;
    drop(rows);
    Ok(json!({"schemaVersion":1,"workspace":root,"initialized":true}))
}
pub fn execute(args: &[String]) -> crate::Result<Value> {
    let parsed = crate::Args::parse(args, &["--workspace", "--request-id"], &[])?;
    let request = parsed.values.get("--request-id").filter(|v|!v.is_empty())
        .ok_or_else(||crate::usage("Original request-id is required"))?;
    let fail = |e: bgx_adapter_v2_reference_host::checkpoint::CheckpointError|
        crate::error("SESSION_UNAVAILABLE", "/session", e.message, "Preserve the original SDK workspace and request identity");
    let workspace = DevelopmentWorkspace::open(&parsed.path("--workspace")?).map_err(|e|
        crate::error("SESSION_UNAVAILABLE", "/workspace", e.message, "Select the original marked SDK workspace"))?;
    let rows = SqliteFixtureRows::open_read_only(&workspace).map_err(fail)?;
    let store = CheckpointStore::with_rows(Box::new(rows), SnapshotPolicy::new(vec![]).map_err(fail)?);
    let checkpoint = store.read(request, request).map_err(fail)?;
    Ok(json!({"schemaVersion":1,"requestId":request,"checkpoint":checkpoint}))
}
