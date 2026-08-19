#!/usr/bin/env bash
set -euo pipefail

library=${1:-}
if [[ -z "$library" || ! -f "$library" ]]; then
    echo "usage: $0 <shared-library>" >&2
    exit 2
fi

expected_exports() {
    printf '%s\n' \
        up_abi_version \
        up_context_capabilities \
        up_context_create \
        up_context_free \
        up_detect \
        up_embed \
        up_result_free \
        up_result_json \
        up_result_output \
        up_verify \
        up_version
}

actual_exports() {
    case "$(uname -s)" in
        Darwin)
            nm -gU "$library" | awk 'NF { print $NF }' | sed 's/^_//'
            ;;
        Linux)
            nm -D --defined-only "$library" | awk 'NF { print $NF }'
            ;;
        *)
            echo "unsupported host for export inspection" >&2
            exit 2
            ;;
    esac
}

if ! diff -u \
    <(expected_exports | LC_ALL=C sort) \
    <(actual_exports | LC_ALL=C sort); then
    echo "shared-library exports differ from the canonical ABI" >&2
    exit 1
fi

echo "Underprint shared-library exports match the canonical ABI"
