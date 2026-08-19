# Release, support, and disclosure policy

Underprint is pre-1.0. Until 1.0, only the latest tagged minor release receives
security and correctness fixes. After 1.0, the current major release and the
immediately preceding major release will receive critical security fixes for at
least twelve months after supersession. Exact supported versions are listed in
each release note.

Security reports go privately to `contact@schem.at`; see `SECURITY.md`. We aim
to acknowledge a report within five business days, establish severity and a
remediation plan within fourteen days, and coordinate disclosure after a fix is
available. These targets are not a warranty or service-level agreement.

## Release requirements

A production tag must:

- match the Cargo workspace version;
- pass formatting, lint, unit, contract, ABI, foreign-language, fuzz-build,
  dependency, licence, vulnerability, and container gates;
- build the documented platform matrix from a locked dependency graph;
- publish deterministic archives, SHA-256 checksums, an SPDX SBOM, and GitHub
  build-provenance/SBOM attestations;
- identify every included model, native runtime, corpus, and asset and confirm
  its redistribution status; and
- link the applicable benchmark/robustness report and known limitations.

Release tags are immutable. A broken release is superseded by a new version,
not rebuilt in place. Operators roll back by digest to the retained prior
artifact.

## Upgrades and deprecation

Security dependency updates are reviewed continuously and normal dependency
updates at least monthly. Runtime/model upgrades require profile-parity tests
and never mutate an immutable profile. Public API or schema removals require a
major release. Non-security deprecations receive at least two tagged minor
releases of notice.

Historical profiles follow the retention promise in
`docs/decisions/0001-product-boundaries.md`. A release cannot claim production
readiness while a required artifact lacks redistribution permission, while the
clean-corpus false-positive gate is absent, or while its target platform has not
passed the release workflow.
