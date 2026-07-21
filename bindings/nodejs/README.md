# `@model-asset-guard/node`

Node bindings over the Rust `guardd` C ABI using [koffi](https://koffi.dev/)
(prebuilt N-API; no `node-gyp` / `ffi-napi` toolchain required).

## Install (from repo)

```bash
cargo build --release --manifest-path ../../src/rust/guardd/Cargo.toml
bash ../../scripts/stage_guardd_native.sh
cd bindings/nodejs
npm install
npm test
```

Or fetch a CI/release-built library (needs `gh` CLI; does **not** embed prebuilts):

```bash
cd bindings/nodejs
npm run fetch-native
# optional: npm run fetch-native -- --tag v0.1.0
```

### Pointing at `libguardd`

Search order:

1. Constructor argument / `createGuard(libPath)`
2. Environment variable `GUARDD_LIB` (absolute path to `.dll` / `.so` / `.dylib`)
3. `bindings/nodejs/native/` (staged by `scripts/stage_guardd_native.sh`)
4. `dist/native/` at the repo root
5. `src/rust/guardd/target/{release,debug}/` relative to the repo root
6. This package directory

On Windows the artifact is typically `guardd.dll`; on Linux/macOS,
`libguardd.so` / `libguardd.dylib`.

Prefer GitHub Release assets `guardd-native-<os>.zip` (via `npm run fetch-native`);
CI artifacts remain a fallback for unreleased commits. Or set `GUARDD_LIB`.
See [`docs/native-packaging.md`](../../docs/native-packaging.md).

**Honesty:** this npm package does **not** ship platform prebuilts. A local
`cargo build` or `npm run fetch-native` (release/CI download) is required.

## Scripts

| Script | Purpose |
| --- | --- |
| `npm test` / `npm run smoke` | Digest + `checkedLoad` smoke (`exit 2` = skip if lib/koffi missing) |
| `npm run selfcheck` | Inline self-check via `node_guard.js` |
| `npm run fetch-native` | Download `libguardd` via repo helper (release-first; needs `gh`) |

## Honesty notes

- `checkedLoad` sets `valid` from a real digest comparison when an expected
  digest is supplied; it does not hardcode success.
- Bit-flip helpers default to small corpora; large sizes require opt-in on the
  Rust side (`GUARDD_BITFLIP_ALLOW_LARGE=1` or CLI `--allow-large`).
- Hash-vocab encode/decode uses CHD MPH by default (`kind: "chd_mph"`) with
  string verify; legacy open-address JSON still loads. Lean proves structural
  placement / assoc-list round-trip properties, not the Rust displace search.
- `spEncode` / `spDecode` mirror the Rust SentencePiece gate (CHAR/BPE/UNIGRAM/WORD
  + charsmap / byte_fallback / n-best / sample). Options include `reverse`,
  `addBos` / `addEos`, and n-best `nbestStockCompat`. Smoke tests remain
  skippable when the shared library or `koffi` is missing (`exit 2`).
