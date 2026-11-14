# Image and video projection

Protocol remains 2. Require `media-projection-v1`; unsupported hosts reject before execution. Optional negotiation and combining it with `image-projection-v1` are invalid. Existing packages and frozen tasks retain their original profile. No WIT, lease or execution-envelope changes.

Non-streaming image.generate/video.generate operations require outputProjection, a nonempty unique closed enum with a valid default: flat, equirectangular180, equirectangular360. At least one required output must match the capability kind.

Successful data.mediaArtifacts binds every image/video output exactly once by mediaId, role, kind and projection. Projection equals the original frozen request. Equal bytes may have distinct media identities; never merge receipts by digest. Audio/file outputs have no entries. Accepted receipts keep the existing shape; other non-success states cannot publish visual outputs or entries.

The media owner uses decoded/probed dimensions: 180 is square, 360 is exactly 2:1, dimensions are positive and arithmetic cannot overflow. Flat adds no aspect constraint. Video needs an explicitly configured ProbeTool; missing measured dimensions reject panorama output. Never fill missing facts from requested dimensions, resize silently or infer projection from filenames. Audio, frame rate, duration, color and pose remain in existing media facts and operation contracts.

Query/recovery/retained operations preserve the original task and accepted ID, negotiate the frozen profile and never resubmit generation. Vendor models, graph mapping and rendering belong to adapters.

The existing image_projection_sync.py fixture accepts `--media-projection <absolute-ffprobe> <absolute-2:1-video>` with a media-projection-feature guest. It runs loopback HTTP with real tiny images/video, checks retained export and independent equal-byte identities, and rejects wrong video aspect or missing probe dimensions. Callers supply explicit tools/media; no PATH discovery or model downloads.
