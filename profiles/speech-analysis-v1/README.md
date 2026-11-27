# Speech analysis profile v1

`speech.diarize` / `std/speech.diarize@1` uses the existing submit, query and cancel operations and the standard authorized `audio` media slot at `/audio`. It returns structured `data`; it does not generate a new voice or identify a real-world person.

`speech.subtitle` / `std/speech.subtitle@1` requests `segmentTimed` for the authorized audio slot. Each cue uses half-open sample offsets relative to the original upload, before provider-side silence removal. Successful silence returns `no_speech` and an empty cue array. Hosts must check cue order and bounds against the reported original frame count before writing a timeline.

The host supplies the original source fingerprint and duration. Adapters must retain those anchors and use original-source millisecond times; concatenating voiced regions must never shorten the timeline. Implementations independently fingerprint received media for artifact reuse. `speakerCount`, when omitted, requests the model's supported automatic count estimation. Unsupported counts must fail explicitly rather than silently clipping.

Speaker IDs are anonymous, stable across the complete source and retained on resume. A turn must have `0 <= startMs < endMs <= durationMs`, refer to one unique declared speaker, and retain any actually detected overlaps. Empty `speakers` and `turns` mean no speech. Hosts validate these relational invariants in addition to the JSON schema.

`overlapDetection: "unsupported"` is a capability limitation, not proof that the source has no overlapping speech. Clients must keep manual correction available. `available` may only be returned by an implementation that actually ran its declared overlap detector.

Asynchronous adapters persist and reuse accepted jobs and successful analysis artifacts under the existing checkpoint owner. The public resume token binds sourceFingerprint and jobId. Querying or resuming must not restart completed inference unnecessarily. Cancellation closes only resources owned by that job; completed source-time artifacts may be retained for an explicit resume.

## Transcription

The `speech.transcribe` capability exposes `std/speech.transcribe` version 1. Send an authorized `audio` media slot and request `textOnly`. A successful result reports the duration of the submitted audio. Use `no_speech` with an empty string when recognition finds no speech. A failed task uses the standard error result. Resumable adapters persist the opaque job ID and query it through the standard checkpoint flow.

### Transcription policy

Call `speech.transcribe.policy` / `std/speech.transcribe.policy@1` with
`{"schemaVersion":1,"usage":"chatMessage"}` or
`{"schemaVersion":1,"usage":"continuousDictation"}`. Use the same package,
model, and account binding as the transcription operation. This is a local,
synchronous operation with no media, network, or billable request.

A chat result declares `wholeFile` and `maxBatchSamples`. An incremental
dictation result declares `progressiveFile` and also
`minPauseFlushSamples` and `maxBufferedMs`. Sample limits are counts of
16 kHz mono PCM samples. The host applies the returned policy with the
SDK planner and sends one standard file-transcription request per batch.
A batch includes a half-open, non-overlapping range of the supplied samples.
A producer may deliver a natural pause at the preceding input's end.
Only a declared policy authorizes this kind of batching.
