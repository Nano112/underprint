# Fuzzing

The libFuzzer package covers bounded image decoding. Opaque-handle ABI mutation
is exercised by the native property/stress harness because linking a `cdylib`
into libFuzzer is not portable across the supported macOS and Linux toolchains.

```bash
cargo install cargo-fuzz --locked
cargo +nightly fuzz run media
```

Corpus and crash artifacts must contain synthetic or public data only. Minimized
regressions belong in deterministic unit tests before a fix is considered done.
