# Underprint roadmap

This is the executable roadmap for Underprint. Product invariants and normative
requirements live in [`docs/requirements.md`](docs/requirements.md); this file
tracks delivery. A checked item means its implementation and stated validation
exist in this repository, not merely that code was started.

Production, corpus, redistribution, hardware, and legal dependencies that
cannot be satisfied by source changes are listed in
[`docs/roadmap-blockers.md`](docs/roadmap-blockers.md).

## Release definitions

- **Compatibility build:** the smallest native CPU build required by Schematio.
- **Full CPU build:** multi-signal detection, provenance, evidence, service,
  batch, and robustness tools.
- **Full GPU build:** the full build plus explicitly selected execution
  providers. GPU support never changes a profile's meaning silently.
- **Production-ready:** every gate in the applicable milestone is checked,
  compatibility and rollback are demonstrated, and release artifacts are
  reproducible, signed, documented, and legally redistributable.

## M0 — Native compatibility foundation

Goal: replace the Python execution dependency with one native implementation
shared by Rust, the CLI, C callers, and PHP.

### Core and TrustMark

- [x] Create the Rust workspace with separate core, TrustMark, CLI, and FFI
  crates.
- [x] Define the immutable `trustmark-q-bch5@1` compatibility profile.
- [x] Implement PNG, JPEG, and still-WebP decoding with compressed-byte,
  dimension, pixel-allocation, and output limits.
- [x] Normalize input to RGB and reproduce the legacy small-image enlargement
  and padding policy.
- [x] Validate exactly 61 ASCII binary digits for the BCH-5 payload.
- [x] Run TrustMark Q encode/decode natively through ONNX Runtime without
  Python or subprocesses.
- [x] Pin encoder and decoder sizes and SHA-256 digests in a model manifest.
- [x] Reject missing, altered, or unexpected model artifacts before loading.
- [x] Implement adaptive strength escalation at `0.6, 0.7, 0.8, 0.9, 1.0`.
- [x] Serialize every candidate to final 8-bit RGB PNG and recover the exact
  payload before accepting it.
- [x] Fail closed when no strength produces an exactly recoverable payload.
- [x] Return versioned reports with profile, strength, input/output hashes,
  artifact identities, timing, and self-verification state.

### Public surfaces

- [x] Expose the application core as a native Rust API.
- [x] Implement scriptable `algorithms`, `doctor`, `embed`, `detect`, and
  `version` CLI commands with JSON output.
- [x] Define a versioned C ABI using opaque validated handles, fixed statuses,
  byte views, and matching free functions.
- [x] Catch panics at every exported ABI boundary.
- [x] Reject null, stale, wrong-type, and repeatedly freed handles without
  dereferencing foreign memory.
- [x] Implement a direct PHP FFI binding with binary buffers and one warm
  context per process.
- [x] Keep model inference in the shared library; do not invoke Python, a CLI,
  or an HTTP sidecar from PHP.

### M0 validation

- [x] Unit-test media limits, payload policy, strength scheduling, fail-closed
  embedding, capabilities, and handle safety.
- [x] Run native CLI embed/detect against the real TrustMark Q artifacts.
- [x] Verify Underprint output with Schematio's TrustMark 0.9.1/PyTorch worker.
- [x] Verify Schematio worker output with Underprint.
- [x] Exercise the compiled shared library through both C and PHP.
- [x] Verify stripped compatibility artifacts and exported-symbol allowlisting.
- [x] Commit synthetic golden vectors and their generation/verification
  procedure without production user data.
- [x] Record reproducible cold/warm latency, peak RSS, output size, and quality
  baselines on named reference hardware.

## M1 — Schematio-compatible production surface

Goal: prove read/write parity in a production-shaped deployment and migrate
Schematio with an independent rollback path.

### Contracts and CLI completion

- [x] Freeze detection, embedding, capabilities, and error JSON schemas for the
  compatibility release.
- [x] Document ABI thread-safety, blocking calls, context sharing, model load,
  buffer lifetime, and shutdown behavior.
- [x] Add ABI/build/schema/profile/artifact introspection to every public
  surface.
- [ ] Complete stable exit-code coverage: not present, invalid arguments,
  invalid input, unavailable, untrusted evidence, limits, partial batch, and
  internal failure.
- [x] Add explicit stdin/stdout support and refuse binary output to a terminal
  without `--force`.
- [x] Add atomic output writes, overwrite protection, and an explicit atomic
  in-place mode.
- [x] Guarantee ANSI-free output for JSON, redirected streams, `NO_COLOR`,
  `TERM=dumb`, and CI.
- [x] Add CLI JSON-schema fixtures and snapshot/contract tests.

### HTTP compatibility service

- [ ] Implement `GET /health`, `POST /embed`, and `POST /decode` with exact
  Schematio-compatible success and failure semantics.
- [ ] Implement `GET /health/live`, `GET /health/ready`,
  `GET /v1/algorithms`, `POST /v1/embeddings`, `POST /v1/detections`, and
  `POST /v1/verifications`.
- [x] Publish and contract-test an OpenAPI 3.1 document.
- [x] Stream and bound uploads without accepting remote URLs by default.
- [ ] Add request IDs, safe structured errors, authentication hooks,
  idempotency keys for writes, and rate/concurrency limits.
- [x] Make readiness depend on required artifact digests and successfully
  initialized profiles.
- [x] Document media retention and ensure uploads are not retained by default.
- [ ] Add graceful shutdown, bounded queues, deadlines, cancellation, and
  backpressure.

### Container and operations

- [x] Build a minimal non-root container with no compiler, Python runtime,
  model downloader, or writable application directory.
- [x] Decide whether model artifacts are separately mounted, attached to a
  release, or included in an image after redistribution review.
- [x] Add structured logging and metrics for duration, outcome, profile,
  strength, queue time, resource rejection, and model readiness.
- [x] Ensure logs exclude media bytes, private payloads/metadata, credentials,
  keys, filesystem paths, and stack traces by default.
- [x] Add health/readiness probes, resource recommendations, deployment
  examples, and rollback documentation.
- [x] Test CPU/memory limits and concurrent overload behavior in the container.

### Compatibility, security, and migration

- [ ] Build a consented historical Schematio read corpus plus synthetic clean,
  malformed, and transformed corpora.
- [ ] Measure clean-corpus false-positive behavior and define a release
  threshold before enabling production conclusions.
- [ ] Add golden cross-runtime tests to CI with pinned legacy and native
  versions.
- [ ] Fuzz image metadata/decoders, JSON options, manifests, and all ABI handle
  operations.
- [ ] Add timeout, cancellation, memory exhaustion, decompression-bomb, and
  oversized-output tests.
- [ ] Add long-running PHP FFI memory/handle/concurrency soak tests, including
  Laravel queue and Octane lifecycle cases.
- [x] Add at least one non-PHP foreign-language binding and ownership test.
- [ ] Run shadow detection in Schematio and compare every result without
  changing writes or user-visible conclusions.
- [ ] Define parity tolerances, alerting, rollout stages, feature flags, and an
  independent rollback path.
- [ ] Enable native reads, then canary writes, then full writes only after the
  recorded gates pass.
- [ ] Retain the historical profile, artifacts, and decoder needed to read old
  protected content.

## M2 — Multi-signal detection platform

Goal: detect and report several independent watermark and provenance signals
without conflating them.

### Orchestration

- [ ] Generalize the engine registry for multiple profiles, codecs, media
  capabilities, runtimes, and resource classes.
- [ ] Implement `fast`, `balanced`, and `deep` detection plans.
- [ ] Add bounded transform and region-search plans with overall and per-engine
  deadlines, memory budgets, and concurrency budgets.
- [ ] Preserve successful engine results when another engine is unavailable,
  times out, or fails; report partial execution explicitly.
- [ ] Retain conflicting payloads and signals rather than selecting one hidden
  winner.
- [ ] Keep raw detector scores distinct from calibrated confidence and policy
  conclusions.
- [ ] Add profile discovery and selection consistently to Rust, CLI, ABI, PHP,
  and HTTP.

### Signals and profiles

- [ ] Add reviewed TrustMark variants and BCH profiles with immutable IDs and
  pinned artifacts.
- [ ] Add C2PA manifest discovery, signature/claim validation, ingredients,
  trust status, and failure detail through `c2pa-rs`.
- [ ] Inspect EXIF, IPTC, and XMP rights/provenance metadata without treating
  unsigned metadata as proof.
- [ ] Add configurable perceptual hashes labelled strictly as similarity
  evidence.
- [ ] Select one additional invisible-watermark family only after source,
  model, patent, licence, redistribution, CPU, and robustness review.
- [ ] Implement the selected detector natively and publish its profile-specific
  false-positive and transform results.
- [ ] Evaluate later DWT/DCT, StegaStamp-compatible, RivaGAN-compatible, Stable
  Signature, and private organization profiles without promising support
  before legal and technical review.

### Multi-signal acceptance

- [ ] Version and publish a calibration corpus and method before returning any
  value named `confidence`.
- [ ] Report state, engine, profile, payload/raw bytes, artifacts, transform,
  timing, measurements, and errors for every attempted detector.
- [ ] Test coexistence, contradictions, false positives, partial failures, and
  deterministic bounded search behavior.

## M3 — Portable evidence and provenance

Goal: turn detected signals into independently verifiable evidence without
claiming that detection alone proves ownership or authenticity.

### Payload model

- [ ] Separate logical payloads from algorithm-specific encodings.
- [ ] Support raw bytes, bitstrings, UTF-8 where capacity permits, UUID/ULID,
  URI, the Schematio 61-bit token, and compact provenance references.
- [ ] Define stable canonicalization and size/capacity failure rules.

### Signed evidence

- [ ] Record an architecture decision choosing COSE, DSSE, or JWS for a
  deterministic Ed25519 evidence envelope.
- [ ] Include issuer, key ID, profile, payload, subject and output hashes,
  artifact digests, effective configuration, timestamp, and optional subject
  URI.
- [ ] Implement `verify`, `evidence inspect`, `evidence sign`, and
  `evidence verify` across the relevant public surfaces.
- [ ] Support offline public-key trust stores, issuer policies, key rotation,
  expiry, and revocation without requiring Underprint's service to exist.
- [ ] Load private keys from restricted files, OS key stores, PKCS#11, or KMS;
  never return or log secret material.
- [ ] Add resolvers with bounded network behavior, cache policy, SSRF defense,
  and explicit online/offline modes.
- [ ] Keep detection, evidence resolution, cryptographic verification, and the
  caller's policy conclusion as separate fields.
- [ ] Add Schematio's historical HMAC evidence adapter without presenting
  symmetric legacy evidence as portable third-party proof.
- [ ] Link signed evidence to C2PA claims where useful without duplicating or
  contradicting the C2PA trust result.

### Evidence acceptance

- [ ] Add deterministic vectors, tampering cases, wrong-key cases, rotation and
  revocation cases, clock handling, schema upgrades, and offline verification
  tests.
- [ ] Write a threat model covering issuer compromise, replay, substitution,
  downgrade, malicious metadata, resolver attacks, and lost keys.

## M4 — Batch, robustness, performance, and GPU

Goal: operate Underprint at scale and make robustness/performance claims from
repeatable evidence.

### Batch and distributed work

- [ ] Implement `batch embed`, `batch detect`, and `POST /v1/batches` with
  bounded concurrency and partial-success semantics.
- [ ] Choose SQLite/files or PostgreSQL/object storage for resumable batch
  state and record the architecture decision.
- [ ] Add idempotent jobs, checkpoints, retry policy, cancellation, progress,
  result manifests, retention, and safe cleanup.
- [ ] Sign webhook deliveries and implement replay protection and retry/dead
  letter behavior.
- [ ] Support distributed service workers without changing core result
  semantics or profile identity.

### Robustness laboratory

- [ ] Implement versioned transforms for JPEG/WebP compression, PNG
  re-encoding, resizing, crop/pad/translation, rotation/orientation,
  color/gamma/contrast, blur/sharpen/denoise/noise, overlays, occlusion,
  collage, screenshot/camera capture, and social-platform presets.
- [ ] Track corpus consent/licensing, hashes, splits, versions, and provenance.
- [ ] Report detection rate, bit accuracy, false positives, latency, memory,
  output size, visual-quality metrics, profile/corpus versions, and confidence
  intervals where meaningful.
- [ ] Add `robustness run` and `benchmark` commands with machine-readable,
  reproducible reports.
- [ ] Define quality and robustness gates separately for every immutable
  profile.

### Scheduling and acceleration

- [x] Add lazy one-shot model loading and reusable warm contexts with explicit
  memory accounting.
- [ ] Build a scheduler that caps CPU, memory, GPU, model sessions, queue depth,
  and each engine's resource class.
- [ ] Benchmark CPU cold/warm paths and concurrency on named x86-64 and ARM64
  reference systems.
- [ ] Select the initial GPU provider matrix based on actual deployment needs.
- [ ] Add CUDA, TensorRT, DirectML, CoreML, or other ONNX Runtime providers only
  behind independent features with documented vendor obligations.
- [ ] Verify CPU/GPU interpretation parity and prevent provider selection from
  silently changing a profile ID or threshold.
- [ ] Ensure health and lightweight jobs cannot be starved by deep detection or
  batch workloads.

## M5 — Developer experience and ecosystem

Goal: make the full platform easy to integrate, inspect, package, and maintain.

- [ ] Add optional Ratatui views for detection inspection, batch progress,
  robustness comparisons, and artifact diagnostics while keeping every action
  fully scriptable.
- [ ] Add bindings/examples for C, C++, PHP, Go, Python, Node.js, and another
  JVM or .NET language without reimplementing algorithm policy.
- [ ] Publish API, ABI, schema, profile-authoring, deployment, migration,
  security, and troubleshooting documentation.
- [ ] Generate bindings where practical from one canonical ABI definition and
  test binary ownership in every supported language.
- [ ] Define a memory-safe future extension mechanism, preferring a Rust API or
  WebAssembly component; do not accept arbitrary in-process native plugins.
- [ ] Provide sample integrations for Laravel queues/Octane and a standalone
  service deployment.

## Release engineering and governance

- [ ] Confirm the public repository URL, package ownership, crate publication
  rights, and `underprint` naming/trademark clearance before publication.
- [ ] Resolve TrustMark model-weight redistribution permission and document the
  approved acquisition mechanism.
- [ ] Audit every source dependency, native runtime, model, test corpus, and
  bundled asset; keep `NOTICE` and the licence inventory current.
- [x] Pin dependencies and artifacts, run dependency/vulnerability/licence
  checks, and define supported upgrade windows.
- [ ] Build and test Linux x86-64/ARM64, macOS x86-64/ARM64, and Windows x86-64
  CLI and shared-library artifacts.
- [ ] Produce deterministic archives with checksums, SBOMs, signed release
  manifests, and build provenance attestations.
- [x] Enforce exported-symbol allowlists and ABI compatibility checks in CI.
- [x] Add sanitizers and platform-appropriate memory/error tooling to CI.
- [x] Establish supported-version, vulnerability disclosure, release,
  deprecation, and historical-profile retention policies.
- [ ] Publish benchmark and robustness reports beside each production profile,
  with hardware, flags, corpus, warm/cold state, concurrency, and percentiles.

## Open decisions

- [x] Choose the portable evidence envelope: COSE, DSSE, or JWS.
- [ ] Choose the first additional invisible-watermark family.
- [ ] Choose the first GPU execution provider(s).
- [x] Choose initial batch persistence and object storage.
- [x] Choose service authentication defaults.
- [x] Decide whether PHP `auto` transport may select FFI only for explicitly
  configured queue/Octane workers.
- [x] Define the compatibility and retention promise for old models, profiles,
  schemas, and evidence.
