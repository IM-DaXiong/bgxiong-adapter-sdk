# Operation Input v1

Set `inputProfile` to `operation-input-v1` on the capability operation and declare `operation-input-preflight-v2` in the package manifest's required features. The operation's JSON Schema owns field names, primitives, limits, conditional constraints, and legal enumerations. No field names are reserved for model parameters.

Controls address the same input by RFC 6901 field pointer. A display role such as `prompt`, `size`, `aspectRatio`, `duration`, `frameRate`, or `seed` guides presentation. Unknown display roles use the declared widget. Control hints never change the value, unit or range. Media slots supply managed IDs with the carrier forms declared by the package schema. Transport budgets come from the SDK schema policy.

Apply schema defaults only on creation of a draft. Schema constants can be instantiated when absent. Report conflicting values. Preserve absence, null, an empty string, zero, false and an empty array independently. Declare large integer identifiers as decimal strings when they exceed JSON safe-integer precision.

Send the frozen selection, non-secret configuration, environment revision and readiness, input, and content identities in the feature's `probe` payload. The SDK computes the request digest as lowercase SHA-256 of canonical UTF-8 JSON without `requestDigest`. Object keys sort by Unicode code point; arrays keep their order with the sole exception of the metadata table sorted by media ID. Safe integer-valued numbers omit the fraction. Other numbers use the SDK's JSON serialization. Use the SDK helper when making or checking a digest.

On probe, return `ready` with material or `rejected` with diagnostics, echoing the request digest. Use JSON Pointers relative to the input for field diagnostics. Use package namespaced diagnostic codes for business rejections. The sandbox grants logging only during this probe. The package must avoid credentials, uploads, submissions and checkpoints. Any random value used for the task belongs in the opaque preparation material with its effective value. Credential references and secrets stay with the host. Material must not carry arbitrary filesystem paths.

The SDK verifies the result and seals the material with a digest. The reference CLI writes the sealed value into `operation-preparation.json` in the run diagnostics. Submit the same unmodified input with the sealed `preparation` the current `environmentRevisionDigest`, and live managed media. Changed input, account binding, package identity, media facts or a different environment revision requires a fresh probe. An expired or absent preparation yields `PREPARATION_STALE` before a submission checkpoint opens. Continue query and cancellation against the original task; never re-submit an unknown acceptance.

The wire examples in `examples.json` show one draft, a probe, and both result branches. The protocol retains capabilities, probe, upload, submit, query, and cancel. The separate video speech profile has its own preparation contract.

## Independent verification

1. Run SDK `check` with the exact package and worker. Verify that capability discovery returns the operation profile.
2. In a fresh development workspace call `probe` with the exact selection and media facts. Replay the same probe with an intentionally illegal combination and inspect its field diagnostics. Prepare-only commands do not load the provider credential or issue HTTP.
3. Use the sealed preparation for `submit`. With a separately declared mock fixture, record the actual probe and submit calls and use the SDK `replay` and `conform` commands. Preserve the original reference for query and cancel. Keep real provider receipts separate from fixtures.
4. Try the preparation with modified input, material, environment revision, package or media. It must fail before submission. Run the same JSON against another package whose declaration or business rule differs and record its actual decision.
