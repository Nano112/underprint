# Architecture

Underprint keeps one policy implementation behind every surface.

```text
                        +-----------------------+
                        |   underprint (library)  |
                        | limits, profiles,     |
                        | orchestration, schema |
                        +-----------+-----------+
                                    |
                 +------------------+------------------+
                 |                                     |
       +---------v-----------+               +---------v----------+
       | underprint-trustmark  |               | future native      |
       | Rust + ONNX Runtime |               | signal engines     |
       +---------+-----------+               +--------------------+
                 |
       +---------+-----------+--------------------------+
       |                     |                          |
+------v------+      +-------v-------+          +-------v-------+
| CLI         |      | C ABI / PHP   |          | HTTP service  |
| underprint    |      | libunderprint   |          | (full build)  |
+-------------+      +---------------+          +---------------+
```

The core library owns limits, preprocessing, adaptive policy, result schemas,
and profile selection. Engines receive already validated media and implement
only algorithm-specific inference. This prevents the CLI, PHP adapter, and
future server from drifting into different interpretations of the same model.

## ABI handle model

Opaque C handles are non-dereferenced numeric tokens. A process-local registry
maps them to Rust-owned contexts and results. This lets the library reject
random, stale, wrong-type, and double-freed handles without trusting a foreign
pointer's memory layout. Result buffers remain owned by the result entry and
are borrowed only until `up_result_free`.

## Model lifecycle

Context creation reads configuration and may remain lightweight when no model
directory is provided. When a directory is configured it verifies both pinned
artifact digests, but encoder and decoder sessions are loaded independently on
first use and then reused. Capability inspection therefore does not initialize
neural inference. A detect-only worker pays only for the decoder; the
compatibility embed flow eventually requires both sessions because every
serialized candidate is decoded before it can be returned. The effective
thread, arena, memory-pattern, and prepacking policy is reported by the C ABI
and PHP capabilities document.

## Feature boundaries

Terminal, HTTP, TUI, and binding dependencies do not enter the shared library.
Model families and execution providers are independent Cargo features. Release
automation checks exported symbols so only the canonical `up_*` ABI is public.
