#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
project_dir=$(cd "$script_dir/.." && pwd)
model_dir="$project_dir/models"
model_root="https://cai-watermark.adobe.net/watermarking/trustmark-models"
task_tmp=$(mktemp -d)
trap 'rm -rf "$task_tmp"' EXIT

fetch_model() {
    local name=$1
    local expected=$2
    local expected_bytes=$3
    local staged="$task_tmp/$name"
    local destination="$model_dir/$name"

    if [[ -f "$destination" ]]; then
        local existing existing_bytes
        if command -v sha256sum >/dev/null 2>&1; then
            existing=$(sha256sum "$destination" | cut -d' ' -f1)
        else
            existing=$(shasum -a 256 "$destination" | cut -d' ' -f1)
        fi
        existing_bytes=$(wc -c < "$destination" | tr -d ' ')
        if [[ "$existing" == "$expected" && "$existing_bytes" == "$expected_bytes" ]]; then
            echo "already verified $name"
            return
        fi
    fi

    curl --fail --location --proto '=https' --tlsv1.2 \
        --max-filesize 100000000 \
        --output "$staged" \
        "$model_root/$name"

    local actual
    if command -v sha256sum >/dev/null 2>&1; then
        actual=$(sha256sum "$staged" | cut -d' ' -f1)
    else
        actual=$(shasum -a 256 "$staged" | cut -d' ' -f1)
    fi
    if [[ "$actual" != "$expected" ]]; then
        echo "$name failed SHA-256 verification" >&2
        exit 1
    fi

    local actual_bytes
    actual_bytes=$(wc -c < "$staged" | tr -d ' ')
    if [[ "$actual_bytes" != "$expected_bytes" ]]; then
        echo "$name has unexpected byte length" >&2
        exit 1
    fi

    mv "$staged" "$destination"
    echo "verified $name"
}

fetch_model \
    "encoder_Q.onnx" \
    "19b3d1b25836130ffd78775a8f61539f993375d1823ef0e59ba5b8dffb4f892d" \
    "17312208"
fetch_model \
    "decoder_Q.onnx" \
    "ee3268f057c9dabef680e169302f5973d0589feea86189ed229a896cc3aa88df" \
    "47401222"
