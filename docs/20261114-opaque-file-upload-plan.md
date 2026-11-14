# Opaque file multipart names

The product's 3D editor conversion requires a basename such as `scene.glb`. The SDK correctly measures unrecognized binary inputs as `file`, `application/octet-stream`, `<digest>.bin`; its multipart extension invariant currently rejects the adapter's `.glb` upload before HTTP. Do not rename durable objects or claim a detected 3D format.

## Fixed scope and self-review

Add negotiated semantic feature `opaque-file-upload-name-v1`, without changing protocol 2, WIT, schemas, media descriptors, leases or durable identities. Only when the verified package and host both enable this feature may an opaque `file` with measured MIME `application/octet-stream` and measured extension `.bin` use a different valid multipart basename. All recognized formats retain exact measured extensions. MIME equality, basename validation, byte measurement, lease ownership, outbound authorization and no retry remain mandatory. The alias is transport metadata; the receiving adapter/platform validates its format. Old packages and retained tasks keep the old behavior.

Files: `protocol/host-feature-catalog.json`; `crates/reference-host/src/policy.rs` (verified negotiation); `http.rs` (measured kind and constrained alias); existing `tests/policy.rs` (actual multipart positive and negative cases); host integration documentation; component/release versions and generated allowlist hashes. No upstream/vendor-specific suffix whitelist in the SDK.

Acceptance: actual local HTTP upload with an opaque filename alias; feature absent rejected before HTTP; recognized PNG renamed GLB rejected; forged MIME, path basename and foreign handle rejected; existing streamed media checks retained. Then build the SDK CLI/worker from a fixed clean commit, consume it in the product, rebuild the adapter with a new version and repeat the original EXE conversion/retained-collect test. Local policy tests do not establish product acceptance.

Budget: two hours for this bounded contract correction, one targeted correction after the initial run. Preserve the failed product task and previous SDK/package artifacts. Do not replay an unknown submit; this fixture received zero uploads or conversions, and uses no inference service.

Self-review: this is an explicitly negotiated filename transport capability, not a relaxation of known media type validation or an arbitrary file execution path. It preserves the measured MIME and content identity and does not expose local paths. No model, GPU, platform API or production database changes are required.

## Local verification

2026-11-14: existing policy integration target ran 4 tests, all passed (actual HTTP, negotiated opaque basename, unchanged recognized-format rejection, MIME/path/lease rejection, streaming/cancellation). Contract and WIT generation checks passed; template mirrors regenerated after Cargo.lock component version changes. Two initial compile errors corrected the typed generated media-kind enum usage; no business assertion was removed. CLI/worker and product integration remain pending at this source checkpoint.
