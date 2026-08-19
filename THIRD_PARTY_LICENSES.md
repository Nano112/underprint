# Third-party components

Underprint's original source is MIT licensed. Release bundles must also preserve
the notices belonging to their dependencies and model artifacts.

| Component | Purpose | Licence | Ownership treatment |
|---|---|---|---|
| Adobe TrustMark `0.2.2` | TrustMark encode/decode and BCH codecs | MIT | Vendored from Adobe commit `0b49d6d523e6756f872a6b57bc543cedea58a616`, then modified for Underprint; Adobe copyright and MIT licence are preserved in `vendor/trustmark` |
| ONNX Runtime | Native neural inference | MIT | Microsoft notices and `ThirdPartyNotices.txt` ship in binary distributions |
| `image` and codec dependencies | Bounded image decoding/encoding | Mixed permissive | Cargo licence inventory ships with releases |
| TrustMark Q ONNX models | Neural weights | Separately hosted by Adobe | Not committed; redistribution requires a recorded model-distribution review |

Release automation must generate a complete dependency licence inventory and
SBOM. This document records the high-level obligations; it is not a substitute
for the generated inventory.
