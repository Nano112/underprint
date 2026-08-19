# Roadmap gates that cannot be completed from source alone

This file separates implementation work from production claims. These gates
remain open even when all local tests pass.

| Gate | Required external input | Completion evidence |
|---|---|---|
| TrustMark weight redistribution | Written terms or permission covering the exact encoder/decoder files | Recorded licence decision and approved release/image acquisition path |
| Historical Schematio corpus | Creator consent, retention rules, and a representative production export | Versioned manifest with hashes, consent basis, and no unapproved user data |
| False-positive threshold | Reviewed clean and transformed corpora | Published method, confidence interval, threshold, and signed results |
| Shadow/canary migration | Schematio production traffic and feature flags | Parity report, alerts, staged rollout record, and exercised rollback |
| Platform release matrix | Hosted Linux x86/ARM, macOS x86/ARM, and Windows runners | Successful tag workflow and downloadable attested artifacts |
| GPU provider | Actual target hardware and packaging/legal requirements | CPU/GPU parity, latency/RSS report, and vendor-notice review |
| Additional watermark family | Patent, source, model, redistribution, CPU, and robustness review | Accepted profile ADR and reproducible corpus results |
| Legal/trademark clearance | Qualified legal review | Written approval for name and redistribution claims |

Unchecked roadmap entries that depend on these gates are intentional. They must
not be checked based on synthetic tests or a developer workstation alone.
