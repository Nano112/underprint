# CLI contract

The command tree and exit codes are part of the compatibility surface. JSON
documents are written to stdout; diagnostics and JSON error documents are
written to stderr. Underprint never emits ANSI escapes, so redirected streams,
`--json`, `NO_COLOR`, `TERM=dumb`, and CI output are stable by construction.

Use `-` as the input path to read at most 10 MiB from stdin. Use `--output -`
to write PNG bytes to stdout. Binary output refuses a terminal unless `--force`
is explicit, and cannot share stdout with `--json`.

Output files are created atomically and never replaced by default. Pass
`--overwrite` to replace a separate destination or `--in-place` to atomically
replace the input after embedding and exact self-verification. `--in-place`
cannot be used with stdin or `--output`.

| Exit | Meaning |
|---:|---|
| 0 | Success or at least one qualifying detection |
| 1 | Valid input with no qualifying detection |
| 2 | Invalid arguments or configuration |
| 3 | Invalid, unsupported, or unsafe input |
| 4 | Profile, model, or runtime unavailable |
| 5 | Evidence exists but is invalid or untrusted |
| 6 | Resource limit or timeout |
| 7 | Partial batch success (reserved until batch v1) |
| 10 | Algorithm or internal failure |

The versioned output contracts are published in [`schemas/`](../schemas/).
