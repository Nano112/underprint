# Underprint product and engineering requirements

Status: implementation draft
Primary language: Rust
Initial consumer: Schematio
Licence: MIT for original Underprint source
Document date: 19 August 2026

## 1. Purpose

Underprint is a standalone, production-grade platform for embedding, detecting,
decoding, and verifying invisible watermarks and provenance signals. It must be
a strict functional superset of Schematio's current preview-watermark worker,
while remaining useful as an independent CLI, native library, and service.

Underprint is implemented natively in Rust. It must not call Python, spawn an
algorithm implementation in another language, or hide a second implementation
behind a subprocess. Native C/C++ dependencies such as ONNX Runtime may be
linked through their supported ABI when they are redistributable and audited.

The project owns its orchestration, schemas, policy, evidence model, ABI, CLI,
resource boundaries, and integrations. Upstream algorithms and model artifacts
remain clearly attributed to their owners.

`MUST` is release blocking. `SHOULD` is a strong default that may be deferred
only with a recorded reason. `MAY` is optional.

## 2. Product principles

1. **Evidence over claims.** Detection, resolution, cryptographic verification,
   and a policy conclusion remain separate facts.
2. **Native, not wrapped.** Supported engines execute through Rust-native code
   and linked native runtimes, never Python or CLI subprocesses.
3. **Compatibility before novelty.** Historical Schematio payloads and evidence
   remain readable.
4. **Bounded by default.** Bytes, pixels, dimensions, frames, memory, time,
   concurrency, queues, and transform search all have limits.
5. **Immutable profiles.** A model, threshold, codec, artifact, or policy change
   creates a new versioned profile.
6. **Multiple signals may coexist.** One detector cannot erase another result.
7. **Offline operation matters.** Models are pinned locally and verification
   must not require the issuer to remain online.
8. **Robustness is measured.** Marketing claims require a reproducible corpus
   and transformation suite.

## 3. Schematio compatibility profile

The first production profile is `trustmark-q-bch5@1`:

- Adobe TrustMark variant Q;
- binary BCH-5 encoding;
- exactly 61 ASCII binary digits;
- CPU operation;
- PNG, JPEG, and still WebP input;
- PNG output;
- TrustMark encoder and decoder ONNX artifacts pinned by SHA-256;
- native Rust inference through ONNX Runtime;
- no Python runtime or sidecar.

### 3.1 Input policy

- Compressed input MUST not exceed 10 MiB.
- Width and height MUST each be within 1–4096 pixels.
- Decoder limits MUST be applied before unbounded pixel allocation.
- Zero-sized, unreadable, truncated, unsupported, and decompression-bomb inputs
  MUST be rejected as invalid input.
- Algorithm input MUST be normalized to RGB.
- If either dimension is below 64 pixels, the image MUST be enlarged with
  aspect ratio preserved when that remains within 4096×4096. Otherwise it MUST
  be centered on a minimally padded RGB canvas without exceeding those limits.

### 3.2 Embedding policy

- The default strength schedule MUST be `0.6, 0.7, 0.8, 0.9, 1.0`.
- Strength start, ceiling, and step MAY be configured within a profile policy.
- Every candidate MUST be serialized to the final output format and decoded.
- Only the first candidate that decodes to the exact requested payload may be
  returned.
- Embedding MUST fail closed after exhausting the schedule.
- PNG output MUST not exceed 64 MiB.
- Result metadata MUST include profile, selected strength, input/output hashes,
  model digests, and self-verification state.

### 3.3 Detection policy

- A payload is present only if BCH decoding succeeds and returns exactly 61
  binary digits.
- `not_present`, invalid input, unavailable model, resource exhaustion, and
  internal failure MUST remain distinct.
- Detection alone MUST never be labelled proof of authorship, ownership, or
  authenticity.

### 3.4 Compatibility gate

Before Schematio write migration:

1. Historical protected previews decode to their existing 61-bit tokens.
2. Underprint outputs decode through TrustMark 0.9.1 and Underprint.
3. Adaptive strengths are attempted in exact 0.1 increments and stop at the
   first verified output.
4. Public failure semantics match the existing worker.
5. Clean-corpus false-positive behavior is measured.
6. Golden vectors contain only synthetic, public, or explicitly opted-in data.

## 4. Build editions

Underprint uses one Cargo workspace and compile-time feature sets, not separate
implementations.

### 4.1 Compatibility edition

The smallest production build includes:

- core schemas and orchestration;
- bounded still-image decoding;
- TrustMark Q/BCH-5 embed and detect;
- CPU ONNX Runtime;
- C ABI and PHP binding;
- CLI commands required by Schematio.

It excludes C2PA, other model variants, GPU providers, service, TUI, batch,
evidence signing, plugins, removal, bounding-box detection, and robustness lab.

### 4.2 Full CPU edition

Adds multi-profile TrustMark, C2PA, metadata, perceptual hashes, portable
evidence, HTTP service, batches, robustness tooling, and optional TUI.

### 4.3 Full GPU edition

Adds supported ONNX Runtime execution providers. CUDA, TensorRT, DirectML,
CoreML, or other providers MUST be independent feature flags and retain their
vendor licence obligations. GPU availability must never alter profile identity
or output interpretation silently.

### 4.4 Release profiles

Release builds use LTO, one codegen unit, symbol stripping, and explicit symbol
allowlists. The FFI build MUST use `panic = "unwind"`; every exported function
catches panics and maps them to a stable status. Panics and foreign exceptions
must never cross the ABI boundary.

## 5. Supported surfaces

All surfaces call the same Rust application core.

### 5.1 Rust API

- Algorithm traits MUST be independent of CLI, HTTP, and FFI concerns.
- Callers MAY provide bytes or readers without filesystem paths.
- Public types MUST document thread-safety and blocking behavior.
- Profiles expose capabilities without eagerly loading every model.
- API stability is not promised before `1.0`; schemas and ABI may stabilize
  earlier under their own versions.

### 5.2 CLI

Initial commands:

```text
underprint algorithms list
underprint algorithms inspect <profile>
underprint doctor [--models <path>]
underprint embed <input> --output <path> --profile <id> --payload <bits>
underprint detect <input> [--profile <id>...] [--mode fast|balanced|deep]
underprint version
```

Full roadmap:

```text
underprint verify <input> --evidence <path-or-url>
underprint batch embed --manifest <file>
underprint batch detect <path...>
underprint evidence inspect|sign|verify ...
underprint robustness run <corpus> --profile <suite>
underprint benchmark <corpus>
underprint serve
```

CLI requirements:

- human and `--json` output;
- stable exit codes;
- stdout contains only requested data, with diagnostics on stderr;
- binary output is never written to a TTY without explicit force;
- redirected output, `--json`, `NO_COLOR`, `TERM=dumb`, and CI disable ANSI;
- stdin/stdout support is explicit where binary ambiguity exists;
- input files are never overwritten without an explicit atomic in-place mode;
- Clap defines the stable command tree;
- Ratatui MAY provide opt-in interactive views, but every operation remains
  fully scriptable.

Exit codes:

| Code | Meaning |
|---:|---|
| 0 | Success; for detection, at least one qualifying signal |
| 1 | Valid input but no qualifying signal |
| 2 | Invalid arguments or configuration |
| 3 | Invalid, unsupported, or unsafe input |
| 4 | Algorithm/model unavailable |
| 5 | Evidence exists but is invalid or untrusted |
| 6 | Resource limit or timeout |
| 7 | Partial batch success |
| 10 | Internal failure |

### 5.3 Stable C ABI

Release artifacts SHOULD include:

- `libunderprint-linux-x64.so`;
- `libunderprint-linux-arm64.so`;
- `libunderprint-macos-x64.dylib`;
- `libunderprint-macos-arm64.dylib`;
- `underprint-windows-x64.dll`;
- canonical `include/underprint.h`.

The ABI MUST:

- use opaque handles, fixed-width integers, pointer-plus-length views, and
  explicit status codes;
- never expose Rust strings, vectors, traits, panics, or layouts;
- provide matching result/context free functions;
- keep allocation and deallocation within the same library;
- reject null/empty/overflowing inputs before creating Rust slices;
- tolerate invalid, stale, wrong-type, and double-freed handles without
  dereferencing foreign pointers;
- make result byte views valid until their result handle is freed;
- expose ABI/build/schema versions, capabilities, and artifact identities;
- transfer image bytes directly rather than through base64;
- define thread-safety, blocking, and model-loading behavior.

Versioned JSON is used for evolving options and result schemas while the C
function surface remains small.

### 5.4 PHP FFI

PHP is a first-class target. Its binding MUST:

- detect the FFI extension and `ffi.enable` availability;
- support an explicit `UNDERPRINT_LIBRARY_PATH`;
- load and capability-check once per PHP process;
- use declarations mechanically synchronized with `underprint.h`;
- use `FFI::memcpy` into contiguous `uint8_t[]` buffers;
- keep borrowed input alive for the call;
- copy output/JSON before freeing the native result;
- map statuses into typed exceptions or detection results;
- expose availability, version, capabilities, detect, embed, and verify;
- work in long-lived Laravel queue and Octane processes;
- keep native paths and internal diagnostics out of public HTTP errors;
- include memory-soak and ABI-mismatch tests.

FFI is recommended for a bounded number of persistent workers. Model-heavy
contexts MUST NOT be loaded into every PHP-FPM worker by default.

### 5.5 HTTP service

The full build will expose legacy Schematio routes and a versioned API:

```text
GET  /health
POST /embed
POST /decode
GET  /health/live
GET  /health/ready
GET  /v1/algorithms
POST /v1/embeddings
POST /v1/detections
POST /v1/verifications
POST /v1/batches
```

The service MUST publish OpenAPI 3.1, bound streaming uploads, request IDs,
idempotency for writes, structured safe errors, authentication hooks, rate and
concurrency limits, readiness based on required artifact digests, and explicit
retention policy. Remote URL fetching is disabled by default.

## 6. Algorithm architecture

An **engine** is a native Rust implementation backed by optional linked native
runtimes. A **profile** is an immutable versioned combination of engine, model,
payload codec, threshold, preprocessing, and resource limits.

Every profile declares:

- media formats and operations;
- payload capacity and codec;
- CPU/GPU/runtime requirements;
- artifact names and SHA-256 digests;
- licence and attribution;
- confidence and threshold semantics;
- expected timeout and memory class;
- transformation/region-search support.

External executable plugins are not part of the supported architecture. Future
plugins MUST use a memory-safe in-process interface, WebAssembly component, or
an explicitly isolated service deployment without changing result semantics.

## 7. Algorithm roadmap

Compatibility release:

- TrustMark Q/BCH-5 binary embedding and detection;
- SHA-256 exact-copy classification;
- optional verification of Schematio's historical HMAC evidence.

First multi-signal release:

- supported TrustMark variants and BCH profiles;
- C2PA discovery, signature validation, claims, and ingredients using `c2pa-rs`;
- EXIF/IPTC/XMP rights and provenance inspection;
- configurable perceptual hashes labelled as similarity evidence;
- one additional open and redistributable invisible-watermark detector after
  model/licence/robustness review.

Candidate later engines include DWT/DCT families, StegaStamp-compatible
detection, RivaGAN-compatible profiles, Stable Signature where licensing
permits, and organization-specific private profiles. Closed vendor signals may
only use authorized SDKs or APIs.

## 8. Multi-algorithm detection

Modes:

- `fast`: metadata and cheap detectors;
- `balanced`: default profiles and bounded normal transform search;
- `deep`: expensive models and bounded region/transform search.

The orchestrator MUST enforce overall and per-engine deadlines, memory and
concurrency budgets, and bounded transform plans. Failures become partial engine
results and never suppress successful detections.

Each detection includes state, engine, profile version, payload/raw bytes,
artifact digests, transformation, duration, and detector-specific measurements.
Raw scores are not universal probabilities. Calibrated confidence requires a
versioned calibration corpus and method. Conflicting payloads are retained and
flagged rather than collapsed.

## 9. Payloads and evidence

Logical payload and algorithm encoding remain separate. Supported logical
payloads SHOULD include raw bytes, bitstrings, UTF-8 where capacity permits,
UUID/ULID, URI, Schematio 61-bit token, and a compact provenance reference.

Portable evidence defaults to asymmetric signatures, preferably Ed25519 in a
deterministic COSE/DSSE/JWS envelope selected by an architecture decision.
Evidence includes issuer, key ID, profile, payload, input/output hashes,
artifact digests, effective configuration, timestamp, and optional canonical
subject URI. Verification supports offline public-key trust stores, rotation,
and revocation. Secrets are loadable from files, OS key stores, PKCS#11, or KMS
without entering logs or result data.

Signal detection, evidence resolution, signature verification, and policy
conclusion MUST remain separate fields. Underprint never reduces them to a single
unqualified “authentic” boolean.

## 10. Security and privacy

- Uploaded media is not retained by default.
- Logs never include image bytes, full private metadata/payloads, credentials,
  signing keys, model paths, or stack traces by default.
- Decoders run with strict byte, dimension, pixel, frame, output, time, and
  memory limits.
- Filenames and metadata are untrusted display data.
- Temporary files are atomic, permission restricted, bounded, and promptly
  removed.
- No request causes model download or shell command construction.
- Model/runtime artifacts are digest verified before readiness.
- Remote ingestion, if enabled, uses scheme/host allowlists and SSRF defenses.
- Dependencies/models are pinned; releases ship SBOMs, signatures, provenance
  attestations, licence inventories, and checksums.

## 11. Robustness laboratory

The full project includes reproducible transformations for JPEG/WebP
compression, PNG re-encoding, resizing, crop/pad/translation, rotation,
orientation, color/gamma/contrast, blur/sharpen/denoise/noise, screenshot and
camera samples, overlays/occlusion/collage, and social-platform presets.

Reports include detection rate, bit accuracy, false positives, latency, memory,
visual-quality metrics, corpus/profile versions, and confidence intervals where
meaningful. Results are profile-specific.

## 12. Performance and deployment

Models load lazily for one-shot CLI use and remain warm in service/FFI contexts.
The scheduler caps work by CPU, memory, GPU, and engine resource class. Health
and lightweight requests cannot be starved by deep jobs.

Reference measurements from the implementation spike on Apple Silicon:

- Q models: approximately 61.7 MiB combined;
- native Rust CLI binary with statically bundled runtime: approximately 27 MiB;
- both native sessions loaded: approximately 131 MiB RSS contribution;
- encode peak: approximately 202 MiB RSS;
- cold decode process: approximately 0.56 s;
- current Python worker embed/self-verify: approximately 1.44 s;
- current warm Python decode: approximately 76–89 ms;
- current Python worker: approximately 404 MiB RSS.

These are planning observations, not universal performance promises. Releases
publish named hardware, build flags, corpus, concurrency, warm/cold state, and
percentiles.

## 13. Testing and acceptance

Required layers:

- unit tests for limits, payloads, policy, schedules, schemas, and ABI handles;
- golden TrustMark vectors and cross-runtime compatibility;
- CLI exit-code and JSON contract tests;
- C ABI and PHP binary-buffer/ownership tests;
- fuzzing for image metadata, JSON schemas, handles, evidence, and manifests;
- cancellation, timeout, and resource-exhaustion tests;
- false-positive and robustness corpora;
- schema/profile/evidence upgrade tests;
- long-lived FFI memory-soak tests.

The first production release requires compatibility gates, digest verification,
recorded false-positive and transformed-corpus results, malformed-input and
resource-bound tests, PHP ABI tests, at least one additional language binding,
documented schemas, a non-root/no-runtime-download container, rollback, retained
historical profiles, and complete legal notices.

## 14. Milestones

### M0 — Native compatibility foundation

- Rust workspace, profile/schema drafts, native TrustMark Q engine;
- adaptive embed and exact self-verification;
- CLI, C ABI, PHP FFI spike;
- model manifest/digest validation;
- synthetic compatibility vectors and baseline benchmarks.

Exit: CLI and PHP call the same native engine and recover expected tokens.

### M1 — Schematio-compatible production surface

- complete CLI and stable ABI semantics;
- legacy HTTP routes and container;
- golden/fuzz/resource tests and observability;
- Schematio shadow detection.

Exit: read parity meets policy and rollback remains independent.

### M2 — Multi-signal platform

- detector registry/orchestration;
- C2PA and metadata;
- additional reviewed detector;
- fast/balanced/deep modes and partial results.

### M3 — Portable evidence

- Ed25519 evidence, trust stores, rotation/revocation, resolvers;
- legacy Schematio adapter and C2PA linkage.

### M4 — Batch, robustness, and GPU

- resumable batches, robustness/calibration reports, signed webhooks;
- optional execution providers and distributed service workers.

## 15. Open decisions

1. Repository organization and final public URL.
2. Model-weight redistribution permission and release download mechanism.
3. Portable evidence envelope: COSE, DSSE, or JWS.
4. First additional invisible-watermark family.
5. Initial GPU provider matrix.
6. Batch storage: SQLite/files first or PostgreSQL/object storage.
7. Authentication defaults for public service deployment.
8. Whether PHP `auto` transport prefers FFI only in queue workers.
9. The compatibility promise and retention policy for historical artifacts.
