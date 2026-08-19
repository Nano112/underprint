# Container validation — Apple Silicon — 2026-08-20

The `underprint:validation` image was built from the locked workspace on an
Apple M2 Max using Docker Desktop 28.0.4 (Linux ARM64 VM), then run with a
read-only root filesystem, all Linux capabilities dropped, no-new-privileges,
two CPUs, 512 MiB memory, one operation permit, and the pinned models mounted
read-only.

Results:

- image digest: `sha256:732e7bb6b4070d1d26ca415d239af7a959151e75e23a6ef590f2b2797a992a70`
- image size: 36,992,677 bytes
- runtime identity: numeric `65532:65532`
- readiness: `200`, after both artifact digests and sessions initialized
- unauthenticated operation on a public bind: `401`
- 20 simultaneous synthetic-golden detections: one `200`, nineteen immediate
  `429` responses, matching the configured concurrency of one
- metrics: one successful detection, nineteen concurrency rejections, zero
  queued microseconds, model readiness `1`
- SIGTERM: emitted the structured `shutdown requested` event and exited cleanly

The test used only
`tests/golden/trustmark-q-bch5-v1/protected.png`. It demonstrates resource-limit
and overload behavior for the compatibility profile; it is not a production
capacity recommendation or a replacement for Linux x86-64 measurements.
