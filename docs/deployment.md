# Service deployment and rollback

The service is an optional full-build surface. The compatibility CLI and shared
library do not depend on Axum, Tokio, or container tooling.

## Artifact policy

TrustMark model redistribution has not been approved. Production images do not
contain model weights or a downloader. Operators acquire the pinned artifacts
using the documented source and mount a read-only directory at `/models`.
Startup verifies exact sizes and SHA-256 digests before binding the HTTP socket,
then initializes required sessions before readiness can pass.

Uploaded media is held only for the bounded request and native operation. The
service has no upload, result, or metadata persistence. It does not fetch remote
URLs. Request logs exclude bodies, payload fields, authorization/cookie headers,
model paths, stack traces, and internal error text. Public failures use the
versioned safe error document.

## Container

```bash
docker build -t underprint:local .
UNDERPRINT_API_TOKEN="$(openssl rand -hex 32)" \
  docker compose -f deploy/compose.yaml up
```

The final image contains the stripped server and Debian runtime only: no Rust
compiler, Python, downloader, model files, or writable application directory.
It runs as numeric user `65532`, drops capabilities, and is intended for a
read-only root filesystem. Models are a separate read-only mount.

The defaults assume two CPU cores and 512 MiB. Start with concurrency `1` for a
256–384 MiB limit; use concurrency `2` only after measuring representative
images. `UNDERPRINT_REQUESTS_PER_SECOND` controls a process-local token bucket.
Work beyond the concurrency limit is rejected immediately with `429`, keeping
health probes responsive instead of building an unbounded queue.

Loopback development may omit authentication. The server refuses to bind a
non-loopback address unless `UNDERPRINT_API_TOKEN` is non-empty. The Compose and
Kubernetes examples therefore require a secret rather than falling back to an
accidentally public unauthenticated service.

## Probes and shutdown

- `/health/live` checks only the process and event loop.
- `/health/ready` requires pinned artifacts and initialized profiles.
- `/health` is the legacy aggregate probe.

SIGTERM and Ctrl-C stop accepting new connections and let Axum drain in-flight
requests. The orchestrator currently has no cooperative ONNX cancellation; set
the platform termination grace period above the request deadline. Do not kill a
native inference thread in place.

## Rollout

1. Deploy with zero external traffic and require readiness.
2. Mirror consented detection requests and compare reports without affecting
   writes or user-visible conclusions.
3. Enable native reads behind a feature flag for a small canary.
4. Enable writes only after clean-corpus, parity, and rollback gates pass.
5. Increase concurrency from measured queue time, RSS, and rejection metrics.

Keep the Python worker and its independently configured model path available
during migration. Do not make the native service the only decoder for historical
content until the retained-profile and corpus gates are recorded.

## Rollback

Route traffic back to the previous service deployment; do not rebuild or mutate
the failed image. Deployment references must use image digests. Retain the
previous binary, profile definition, schemas, model digests, and models. Because
the compatibility profile and payload are unchanged, rollback does not require
rewriting protected images. Disable native writes first, then reads, and preserve
failed request IDs and aggregate metrics without retaining media or payloads.
