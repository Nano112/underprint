# Go binding

This package calls the canonical C ABI through cgo. It does not reimplement
media, profile, payload, or watermark policy. Input allocations remain alive
for each native call, result JSON and image bytes are copied before the native
result is freed, and closing a context is idempotent.

Build the shared library first, then expose it to the dynamic loader:

```bash
cargo build --profile minimal-release -p underprint-ffi
cd bindings/go
DYLD_LIBRARY_PATH=../../target/minimal-release go test ./... # macOS
LD_LIBRARY_PATH=../../target/minimal-release go test ./...   # Linux
```
