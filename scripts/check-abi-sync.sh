#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
project_dir=$(cd "$script_dir/.." && pwd)
canonical="$project_dir/include/underprint.h"
php_header="$project_dir/bindings/php/underprint.ffi.h"
task_tmp=$(mktemp -d)
trap 'rm -rf "$task_tmp"' EXIT

extract_functions() {
    rg -o 'up_[a-z_]+\(' "$1" | sed 's/($//' | sort -u
}

extract_functions "$canonical" > "$task_tmp/canonical"
extract_functions "$php_header" > "$task_tmp/php"

if ! diff -u "$task_tmp/canonical" "$task_tmp/php"; then
    echo "PHP FFI declarations have drifted from include/underprint.h" >&2
    exit 1
fi

echo "Underprint ABI declarations are synchronized"
