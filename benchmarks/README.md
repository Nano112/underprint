# Underprint benchmarks

`ffi.php` measures the production-shaped native path through PHP FFI. It keeps
one model context warm, validates every recovered payload, and emits versioned
JSON containing latency and steady resident-memory checkpoints.

Build the minimal library, provide one source image and an already protected
image containing the configured token, then run:

```bash
cargo build --profile minimal-release -p underprint-ffi

UNDERPRINT_LIBRARY_PATH="$PWD/target/minimal-release/libunderprint.dylib" \
UNDERPRINT_MODELS_DIR="$PWD/models" \
UNDERPRINT_BENCH_INPUT=/path/to/source.jpg \
UNDERPRINT_BENCH_PROTECTED=/path/to/protected.png \
php -d ffi.enable=1 benchmarks/ffi.php
```

The default is a bounded-memory CPU policy: up to six inference threads,
disabled CPU arenas, enabled memory patterns, and enabled prepacking. A
lower-latency throughput policy can be measured without rebuilding:

```bash
UNDERPRINT_BENCH_RUNTIME='{"intra_threads":6,"cpu_arena":true,"memory_pattern":false,"prepacking":true}' \
  php -d ffi.enable=1 benchmarks/ffi.php
```

Use `/usr/bin/time -l` on macOS or `/usr/bin/time -v` on Linux around the PHP
command to capture transient peak RSS. Compare results only when the image,
payload, machine, build profile, warmups, iteration counts, runtime policy, and
power/thermal conditions are recorded. Do not compare a warm result with a
cold process result.

The current named reference run and pre-optimization comparison are recorded
in [`results/apple-silicon-2026-08-19.md`](results/apple-silicon-2026-08-19.md).
