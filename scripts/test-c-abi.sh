#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
project_dir=$(cd "$script_dir/.." && pwd)
profile=${UNDERPRINT_BUILD_PROFILE:-minimal-release}
library_dir="$project_dir/target/$profile"
task_tmp=$(mktemp -d)
trap 'rm -rf "$task_tmp"' EXIT

cc \
    -I"$project_dir/include" \
    "$project_dir/tests/ffi_smoke.c" \
    -L"$library_dir" \
    -Wl,-rpath,"$library_dir" \
    -lunderprint \
    -o "$task_tmp/ffi-smoke"

"$task_tmp/ffi-smoke"
