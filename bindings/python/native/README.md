# Python native library staging

Place `libguardd.so` / `libguardd.dylib` / `guardd.dll` here after:

```bash
cargo build --release --manifest-path ../../../src/rust/guardd/Cargo.toml
bash ../../../scripts/stage_guardd_native.sh
```

Or fetch a release/CI library (see `docs/native-packaging.md`):

```bash
model-asset-guard-fetch-native
# or: python -m fetch_native
# or: bash ../../../scripts/fetch_guardd_native.sh
```

This directory is empty in git by design. PyPI wheels do **not** currently
embed `libguardd`. Fetch prefers GitHub Release `guardd-native-<os>.zip`, then
CI artifacts for unreleased commits. Or set `GUARDD_LIB`.
