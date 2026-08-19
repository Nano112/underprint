# Apple Silicon optimization run — 2026-08-19

## Scope and method

This run measured the stripped `minimal-release` build through the production-
shaped PHP FFI benchmark on an Apple Silicon MacBook Pro running Darwin 24.6.0
and PHP 8.4.8. The public 240×240 UFO fixture and a 1024×1024 resize were used
with the fixed 61-bit compatibility token. Every iteration validated the exact
decoded token. `/usr/bin/time -l` supplied process peak RSS. Cold CLI results
used Hyperfine with five independent processes.

The machine was not isolated from desktop activity, so small latency
differences should be treated as directional. Memory, output hashes, exact
payload recovery, and the large performance gaps were repeatable. The
benchmark command and schema are documented in the parent directory.

## Final runtime profiles

| Policy | Inference threads | CPU arena | Memory pattern | Prepacking |
|---|---:|---:|---:|---:|
| Default / bounded memory | 6 | off | on | on |
| Throughput opt-in | 6 | on | off | on |

Both profiles lazily initialize encoder and decoder sessions. Context creation
still verifies both pinned model SHA-256 digests.

## 240×240 result

| Metric | Original native | Final default | Final throughput | Python worker |
|---|---:|---:|---:|---:|
| Warm detect median | 38.52 ms | 44.02 ms | 35.04 ms | 44.97 ms |
| Warm embed + verify median | 228.20 ms | 222.14 ms | 201.29 ms | 299.44 ms |
| RSS after context creation | 150.14 MiB | 23.48 MiB | 24.63 MiB | n/a |
| RSS after decoder use | 306.63 MiB | 144.44 MiB | 217.72 MiB | n/a |
| RSS after encoder + decoder | 442.02 MiB | 146.98 MiB | 299.58 MiB | 420.6 MiB after run |
| Peak RSS | ~444 MiB | 228.89 MiB | 299.86 MiB | n/a |
| Cold CLI detect mean | 307.3 ms | 273.4 ms | 269.7 ms | 2.611 s process |
| Protected PNG | 103,957 B | 103,957 B | 103,957 B | 99,886 B |

Against the original native implementation, the default cuts steady RSS after
both models by 66.7% and peak RSS by about 48%, while embed latency improves
about 2.7%; detection pays about 14% for the lower-memory allocator policy.
The throughput profile is about 9% faster for detection and 12% faster for
embed while still cutting steady RSS about 32%.

Against the Python worker, the default is roughly equal for warm detection,
26% faster for embed, 65% lower in steady RSS, and about 9.5× faster for a cold
one-shot detection process. The native PNG is 4.1% larger.

## 1024×1024 result

| Metric | Final default | Python worker |
|---|---:|---:|
| Warm detect median | 54.77 ms | 86.10 ms |
| Warm embed + verify median | 294.00 ms | 494.90 ms |
| RSS after encoder + decoder | 151.70 MiB | 424.3 MiB after run |
| Peak native RSS | 234.09 MiB | n/a |
| Protected PNG | 1,022,549 B | 937,929 B |

The final default is about 36% faster for detection, 41% faster for embed, and
64% lower in steady RSS than the current Python worker on this fixture. Its PNG
is 9.0% larger.

## Correctness and artifact results

- The optimized 240 px output remained byte-for-byte identical to the original
  native output: SHA-256
  `22501a77555716c0899f7be990df338caf31e108320bcd37812a37d81f6d061d`.
- Underprint recovered the exact token from the Python worker's PNG, and the
  Python TrustMark 0.9.1 worker recovered the exact token from Underprint's PNG.
- The RGB8 residual fast path has an exact quantization-equivalence unit test.
- The stripped shared library is 17,614,960 bytes; the stripped CLI is
  17,626,800 bytes. The two pinned Q models total 64,713,430 bytes.
- The shared library exports only the eleven intended `up_*` ABI symbols.

## Where the time and memory were won

1. ONNX Runtime sessions now load independently on first encoder/decoder use.
2. The default disables the CPU arena; the throughput profile retains it but
   disables memory-pattern retention.
3. Six inference threads outperformed the previous eight-thread default on the
   reference CPU; higher counts regressed from contention.
4. Encoder preprocessing is calculated once rather than duplicated to derive
   the residual.
5. Model-image conversion borrows source images, and the adapter no longer
   clones around every inference call.
6. RGB8 residual application mutates one final buffer instead of materializing
   duplicate RGBA32F source and output images.
7. PNG serialization avoids an RGB clone when the image is already RGB8.

## Remaining optimization targets

- PNG encoding is the clearest size/latency trade: native output is 4–9%
  larger than Pillow on these fixtures. A diagnostic compression sweep reduced
  the 1024 px file from 1,022,549 to 940,657 bytes with balanced compression,
  but consumed about 4× the encoder CPU in an unoptimized build. Because each
  adaptive candidate is serialized and verified, the fast encoder remains the
  production default until a release-build corpus sweep establishes a useful
  end-to-end trade-off.
- Model digest verification accounts for much of context-creation time. A
  securely keyed file-identity cache could help processes that create many
  contexts, but must never allow changed artifacts to bypass verification.
- Concurrency and queue-level throughput remain unmeasured; additional ONNX
  sessions may improve aggregate throughput at a significant memory cost.
- x86-64 Linux and production container measurements are still required before
  sizing Schematio workers.
- The current quality proof is exact output preservation on reference vectors;
  corpus-level PSNR/SSIM and robustness gates belong in the robustness lab.
