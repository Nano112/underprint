# ADR 0001: initial platform boundaries

Status: accepted for the first compatibility release.

## Portable evidence

Underprint will use a DSSE envelope with Ed25519 signatures. The payload type
will identify a versioned, canonical Underprint evidence document; signing will
use DSSE pre-authentication encoding. DSSE was selected because the envelope is
small, language-neutral, explicit about payload type, and does not make JSON
canonicalization part of the signature primitive. Evidence verification remains
separate from watermark detection and from an application's trust conclusion.

This decision selects the envelope; it does not claim the evidence feature is
implemented. The implementation still needs deterministic vectors, key and
trust-store policy, rotation/revocation handling, and a threat-model review.

## Batch persistence

The first standalone batch implementation will use SQLite for transactional job
metadata and operator-configured filesystem/object storage for media and result
manifests. PostgreSQL and remote object stores belong in a later distributed
worker adapter. Core detection and embedding reports must remain identical
regardless of the persistence adapter.

## Service authentication

Loopback development may run without a token. Binding to a non-loopback address
requires a non-empty bearer token at startup. Health and readiness remain
unauthenticated for orchestrators; algorithms and operation routes require the
token. A reverse proxy may add stronger identity, but must not silently disable
the service's configured authentication.

## PHP transport

There is no implicit `auto` transport. PHP FFI is selected explicitly and is
supported for bounded, long-lived Laravel queue or Octane task workers. A web
application that wants an HTTP service must configure that transport explicitly.
Underprint never changes process topology based on environment heuristics.

## Compatibility retention

Published profile IDs are immutable. A profile's preprocessing, model digests,
payload codec, thresholds, and result interpretation never change in place.
Underprint retains the decoder contract, schemas, and artifact manifest for a
published profile for at least the support lifetime of the last release that can
write it. Removal requires a major release, a migration tool, and two minor
releases of advance deprecation notice. Model weights remain subject to their
own redistribution rights, so operators must archive legally acquired copies.

## Initial execution providers

The first production matrix is CPU-only ONNX Runtime. No GPU provider is
selected until measurements on an actual deployment justify its memory,
packaging, and vendor obligations. Provider selection will be explicit and must
not change an existing profile ID or result threshold.

## Model delivery

Model weights are mounted separately and read-only. They are not embedded in
source, release archives, or container images until redistribution permission is
recorded. Startup verifies pinned size and SHA-256 identity before readiness.
