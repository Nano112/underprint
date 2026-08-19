# C ABI and runtime contract

ABI version 1 is declared by `include/underprint.h`. The header is canonical;
the PHP declaration is mechanically checked against it in CI.

## Thread safety and blocking

All exported functions are safe to call from multiple native threads. Context
and result handles are process-local opaque tokens backed by synchronized Rust
registries. Separate calls may use the same context concurrently, subject to
the configured ONNX Runtime session and host resource limits.

`up_detect`, `up_embed`, and `up_verify` are blocking calls. Version and result
view accessors are non-blocking apart from a short registry lock. Context
creation verifies configured model files but does not initialize neural
sessions. The decoder loads on first detection; embedding lazily loads the
encoder and then the decoder for exact serialized self-verification. Sessions
remain warm until the last owning context is freed or the process exits.

Cancellation is not available in ABI v1. Callers that need hard deadlines must
bound work in a worker process until a future ABI introduces cooperative
cancellation. Killing a thread inside ONNX Runtime is unsupported.

## Handles, buffers, and ownership

- A successful `up_context_create` returns one context handle. Free it with
  `up_context_free` after all calls using it have finished.
- Operations return one result handle for both successful documents and safe
  structured errors. Free it with `up_result_free`.
- `up_result_json` and `up_result_output` return borrowed byte views. They
  remain valid only until that result is freed. Copy them before freeing it.
- Input views are borrowed for the duration of the call. The caller retains
  ownership and must keep their backing allocation alive and unchanged.
- Empty views use `data = NULL, len = 0`. A null pointer with non-zero length is
  rejected. Underprint never frees caller memory.
- Free functions tolerate null, stale, wrong-type, and repeated handles. A
  handle must not be freed concurrently with a call that is still using it.

Rust panics are caught at every exported boundary and map to `UP_INTERNAL`.
They never cross into foreign code. Allocation and deallocation always happen
inside the same Underprint shared library.

## Introspection and compatibility

`up_abi_version` returns the integer ABI version. `up_version` returns the
library package version as a static borrowed view. `up_context_capabilities`
returns the frozen `underprint.capabilities/v1` document containing build,
schema, effective runtime, profile, and artifact identities.

ABI version 1 status values are stable:

| Status | Value | Meaning |
|---|---:|---|
| `UP_OK` | 0 | Successful operation / qualifying detection |
| `UP_NOT_DETECTED` | 1 | Valid input without a qualifying signal |
| `UP_INVALID_ARGUMENT` | 2 | Invalid options or configuration |
| `UP_INVALID_INPUT` | 3 | Unsupported, malformed, or unsafe media |
| `UP_UNAVAILABLE` | 4 | Required profile, model, or runtime unavailable |
| `UP_UNTRUSTED_EVIDENCE` | 5 | Evidence present but invalid or untrusted |
| `UP_RESOURCE_LIMIT` | 6 | Byte, pixel, output, memory, or time limit |
| `UP_INTERNAL` | 10 | Algorithm or unexpected internal failure |

The numeric ABI can remain stable while versioned JSON documents evolve. A
breaking JSON change receives a new schema identifier and file in `schemas/`.

## Shutdown

Stop submitting work, wait for in-flight calls, copy any needed result views,
free results, and finally free contexts. PHP's adapter performs this sequence
from `Native::close` and its destructor. Normal process termination may rely on
the operating system, but explicit shutdown is preferred in leak and lifecycle
tests.
