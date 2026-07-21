# Native packaging (guardd)

Honest status for Model Asset Guard. This is **not** a claim that npm / PyPI
ship platform binaries today.

## What works today

| Path | Status |
| --- | --- |
| Local `cargo build` of `libguardd` / `guardd.dll` | **Yes** |
| `scripts/stage_guardd_native.sh` / `.ps1` copies into `dist/native/` + binding `native/` dirs | **Yes** |
| Node / Python search order includes staged `native/` then cargo `target/` | **Yes** |
| CI upload of per-OS `guardd-native-*` artifacts (quality job) | **Yes** — ubuntu, macOS, Windows |
| Tag release attaches `guardd-native-<os>.zip` assets | **Yes** (`bundle-push.yml`) |
| `scripts/fetch_guardd_native.sh` / `.ps1` | **Yes** — **release asset first**, CI artifact fallback |
| Node `npm run fetch-native` | **Yes** — invokes repo fetch helper (needs `gh`) |
| Python `model-asset-guard-fetch-native` / `python -m fetch_native` | **Yes** — same helper (editable / repo checkout) |
| npm package with embedded prebuilt binaries for all platforms | **No** — not published |
| PyPI wheel with embedded `libguardd` | **No** — sdist/wheel build exists; publish is manual / not automated |

## Local install (recommended)

```bash
cargo build --release --manifest-path src/rust/guardd/Cargo.toml
bash scripts/stage_guardd_native.sh
# Windows: .\scripts\stage_guardd_native.ps1

# Or fetch a built library (needs GitHub CLI):
# 1) latest GitHub Release asset guardd-native-<os>.zip
# 2) if missing (unreleased commit), CI artifact guardd-native-<os>
bash scripts/fetch_guardd_native.sh
# Windows: .\scripts\fetch_guardd_native.ps1

# Node (from bindings/nodejs)
npm run fetch-native

# Python (repo / editable install)
model-asset-guard-fetch-native
# or: python -m fetch_native

# Python load check
export PYTHONPATH="$(pwd)/bindings/python${PYTHONPATH:+:$PYTHONPATH}"
python -c "from pytorch_guard import ModelAssetGuard; print(ModelAssetGuard().lib._name)"

# Node
cd bindings/nodejs && npm install && npm test
```

Or set `GUARDD_LIB` to an absolute path of the shared library.

### Fetch options

```bash
bash scripts/fetch_guardd_native.sh --tag v0.1.0
bash scripts/fetch_guardd_native.sh --from-release          # no CI fallback
bash scripts/fetch_guardd_native.sh --from-ci               # Actions artifact only
bash scripts/fetch_guardd_native.sh --from-ci --run-id 123  # specific run
```

PowerShell mirrors the same flags as `-Tag`, `-FromRelease`, `-FromCi`, `-RunId`.

## Release assets vs CI artifacts

On version tags (`v*`), `bundle-push.yml` attaches
`guardd-native-<os>.zip` to the GitHub Release. Prefer these for consumers:
they last with the release.

On `push` / `pull_request`, the quality workflow builds a **release** `guardd`
library, stages it, and uploads ephemeral Actions artifacts:

- Artifact name: `guardd-native-<os>` (e.g. `guardd-native-ubuntu-latest`,
  `guardd-native-windows-latest`)
- Contents: `dist/native/` (library + `STAGED.txt`)

`fetch_guardd_native` uses **release zip first**, then CI download for
unreleased commits or missing assets. Neither path embeds binaries into
npm/PyPI packages.

### Windows runners

Quality CI uses `shell: bash` (Git Bash) on Windows so `scripts/ci_preflight.sh`
and staging stay the same scripts. Prefer `stage_guardd_native.ps1` /
`fetch_guardd_native.ps1` locally on Windows PowerShell if bash is unavailable.

## Wheel / npm consumers without a git checkout

Published packages still do **not** ship `libguardd`. Options:

1. Download `guardd-native-<os>.zip` from the matching GitHub Release, extract
   `libguardd.so` / `libguardd.dylib` / `guardd.dll`, and set `GUARDD_LIB` to
   that file (or copy next to the binding’s `native/` if you maintain a checkout).
2. Build from source and run `scripts/stage_guardd_native.*`.

The `fetch-native` entry points require the repo `scripts/` helpers (and `gh`).

## What we do not claim

- No fake “published binaries” in the Node `package.json` / npm registry story
- No multi-arch wheel that silently ships empty `native/` directories
- Hash vocab default is **CHD MPH** (`kind: "chd_mph"`); legacy open-address JSON
  still loads (not claimed as MPH when `kind: "open_address"`)
- Formal completeness is not claimed (`docs/axioms.md`); Lean proves abstract
  CHD placement injectivity + bucket-place merge (`Token/CHD.lean`), not the
  hash/displace search

## Residual (Wave 25)

- Optional one-platform wheel that embeds a CI-tested `libguardd` is deferred
  until release automation builds and tests that platform end-to-end (Wave 25b).
- Until then: fetch or local `cargo build` + stage.
