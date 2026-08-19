# Synthetic golden vectors

Golden vectors in this directory contain generated pixels only. They contain no
Schematio uploads, user metadata, filenames, or production tokens.

Regenerate the TrustMark Q/BCH-5 vector with the pinned artifacts in `models/`:

```bash
cargo run -p underprint-cli --example generate-golden
cargo run -p underprint-cli -- detect \
  tests/golden/trustmark-q-bch5-v1/protected.png \
  --models models --json
```

The generator refuses to finish unless the final serialized PNG recovers the
exact 61-bit payload. The committed manifest pins the input, output, payload,
profile, strength, and model digests. Output bytes may only be replaced when a
profile version is intentionally superseded; a changed model or policy requires
a new immutable profile and vector directory.
