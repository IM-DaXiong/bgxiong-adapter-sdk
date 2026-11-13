# Speech analysis profile v1

`speech.diarize` / `std/speech.diarize@1` uses the existing submit, query and cancel operations and the standard authorized `audio` media slot at `/audio`. It returns structured `data`; it does not generate a new voice or identify a real-world person.

The host supplies the original source fingerprint and duration. Adapters must retain those anchors and use original-source millisecond times; concatenating voiced regions must never shorten the timeline. Implementations independently fingerprint received media for artifact reuse. `speakerCount`, when omitted, requests the model's supported automatic count estimation. Unsupported counts must fail explicitly rather than silently clipping.

Speaker IDs are anonymous, stable across the complete source and retained on resume. A turn must have `0 <= startMs < endMs <= durationMs`, refer to one unique declared speaker, and retain any actually detected overlaps. Empty `speakers` and `turns` mean no speech. Hosts validate these relational invariants in addition to the JSON schema.

`overlapDetection: "unsupported"` is a capability limitation, not proof that the source has no overlapping speech. Clients must keep manual correction available. `available` may only be returned by an implementation that actually ran its declared overlap detector.

Asynchronous adapters persist and reuse accepted jobs and successful analysis artifacts under the existing checkpoint owner. The public resume token binds sourceFingerprint and jobId. Querying or resuming must not restart completed inference unnecessarily. Cancellation closes only resources owned by that job; completed source-time artifacts may be retained for an explicit resume.
